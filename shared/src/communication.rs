use crate::{
    event::{self, ManualResetEvent},
    handle, pipe,
};
use serde::{Deserialize, Serialize};
use std::{
    fmt::Debug,
    io::{self, Write},
    marker::PhantomData,
    time::Duration,
};
use thiserror::Error;

#[derive(Debug)]
pub struct Sender<S>
where
    S: Serialize + Debug,
{
    pipe: pipe::Writer,
    send_event: ManualResetEvent,
    acknowledge_event: ManualResetEvent,
    _phantom_data: PhantomData<S>, // circumvents "parameter is never used" error
}

impl<S: Serialize + Debug> Sender<S> {
    pub fn try_clone(&self) -> Result<Self, SenderCloneError> {
        Ok(Self {
            pipe: self.pipe.try_clone()?,
            send_event: self.send_event.try_clone()?,
            acknowledge_event: self.acknowledge_event.try_clone()?,
            _phantom_data: PhantomData,
        })
    }

    pub async fn send(&mut self, message: &S) -> Result<(), SendError> {
        self.pipe.write_all(&bincode::serialize(&message)?)?;
        self.pipe.flush()?;
        self.send_event.set()?;
        self.acknowledge_event.wait().await?;
        self.acknowledge_event.reset()?;
        Ok(())
    }

    #[must_use]
    #[expect(clippy::missing_panics_doc)]
    pub fn serialize_to_bytes(&self) -> [u8; 12] {
        let bytes = unsafe {
            [
                self.pipe.raw_handle() as u32,
                self.send_event.raw_handle() as u32,
                self.acknowledge_event.raw_handle() as u32,
            ]
        }
        .iter()
        .flat_map(|h| h.to_ne_bytes())
        .collect::<Vec<_>>()
        .try_into()
        .unwrap();
        bytes
    }

    #[must_use]
    #[expect(clippy::missing_panics_doc)]
    pub unsafe fn deserialize_from_bytes(bytes: [u8; 12]) -> Self {
        unsafe {
            let mut handles = bytes
                .chunks(4)
                .map(|chunk| u32::from_ne_bytes(chunk.try_into().unwrap()) as _);

            Self {
                pipe: pipe::Writer::from_raw_handle(handles.next().unwrap()),
                send_event: ManualResetEvent::from_raw_handle(handles.next().unwrap()),
                acknowledge_event: ManualResetEvent::from_raw_handle(handles.next().unwrap()),
                _phantom_data: PhantomData,
            }
        }
    }
}

#[derive(Debug)]
pub struct Receiver<R>
where
    R: for<'de> Deserialize<'de> + Debug,
{
    pipe: pipe::Reader,
    send_event: ManualResetEvent,
    acknowledge_event: ManualResetEvent,
    _phantom_data: PhantomData<R>, // circumvents "parameter is never used" error
}

impl<R: for<'de> Deserialize<'de> + Debug> Receiver<R> {
    pub fn peek(&mut self) -> Result<Option<R>, ReceiveError> {
        if !self.send_event.get()? {
            return Ok(None);
        }
        self.send_event.reset()?;
        let received = bincode::deserialize_from(&mut self.pipe)?;
        self.acknowledge_event.set()?;
        Ok(Some(received))
    }

    pub async fn receive(&mut self) -> Result<R, ReceiveError> {
        self.send_event.wait().await?;
        self.send_event.reset()?;
        let received = bincode::deserialize_from(&mut self.pipe)?;
        self.acknowledge_event.set()?;
        Ok(received)
    }

    #[must_use]
    #[expect(clippy::missing_panics_doc)]
    pub fn serialize_to_bytes(&self) -> [u8; 12] {
        let bytes = unsafe {
            [
                self.pipe.raw_handle() as u32,
                self.send_event.raw_handle() as u32,
                self.acknowledge_event.raw_handle() as u32,
            ]
        }
        .iter()
        .flat_map(|h| h.to_ne_bytes())
        .collect::<Vec<_>>()
        .try_into()
        .unwrap();
        bytes
    }

    #[must_use]
    #[expect(clippy::missing_panics_doc)]
    pub unsafe fn deserialize_from_bytes(bytes: [u8; 12]) -> Self {
        unsafe {
            let mut handles = bytes
                .chunks(4)
                .map(|chunk| u32::from_ne_bytes(chunk.try_into().unwrap()) as _);

            Self {
                pipe: pipe::Reader::from_raw_handle(handles.next().unwrap()),
                send_event: ManualResetEvent::from_raw_handle(handles.next().unwrap()),
                acknowledge_event: ManualResetEvent::from_raw_handle(handles.next().unwrap()),
                _phantom_data: PhantomData,
            }
        }
    }
}

pub fn new_sender_and_receiver<T>() -> Result<(Sender<T>, Receiver<T>), NewSenderAndReceiverError>
where
    T: Serialize + for<'de> Deserialize<'de> + Debug + Debug,
{
    let (pipe_writer, pipe_reader) = pipe::new()?;
    let send_event = ManualResetEvent::new()?;
    let acknowledge_event = ManualResetEvent::new()?;
    Ok((
        Sender {
            pipe: pipe_writer,
            send_event: send_event.try_clone()?,
            acknowledge_event: acknowledge_event.try_clone()?,
            _phantom_data: PhantomData,
        },
        Receiver {
            pipe: pipe_reader,
            send_event: send_event.try_clone()?,
            acknowledge_event: acknowledge_event.try_clone()?,
            _phantom_data: PhantomData,
        },
    ))
}

#[derive(Debug, Error)]
#[error("failed to create sender/receiver pair")]
pub enum NewSenderAndReceiverError {
    NewPipe(#[from] pipe::NewError),
    NewEvent(#[from] event::NewError),
    HandleClone(#[from] handle::CloneError),
}

#[derive(Debug)]
pub struct ConductorInitialMessage {
    pub main_thread_id: u32,
    pub initialized_message_sender: Sender<InitializedMessage>,
    pub log_message_sender: Sender<LogMessage>,
    pub idle_message_sender: Sender<IdleMessage>,
    pub message_receiver: Receiver<ConductorMessage>,
}

impl ConductorInitialMessage {
    #[must_use]
    pub fn serialize_to_bytes(&self) -> Vec<u8> {
        let mut bytes = vec![];
        bytes.extend(self.main_thread_id.to_ne_bytes());
        bytes.extend(self.initialized_message_sender.serialize_to_bytes());
        bytes.extend(self.log_message_sender.serialize_to_bytes());
        bytes.extend(self.idle_message_sender.serialize_to_bytes());
        bytes.extend(self.message_receiver.serialize_to_bytes());
        bytes
    }

    /// # Panics
    /// Panics if `bytes` does not have the expected length.
    #[must_use]
    pub unsafe fn deserialize_from_bytes(bytes: &[u8; 4 + 12 + 12 + 12 + 12]) -> Self {
        let (serialized_main_thread_id, bytes) = bytes.split_at(4);
        let (serialized_initialized_message_sender, bytes) = bytes.split_at(12);
        let (serialized_log_message_sender, bytes) = bytes.split_at(12);
        let (serialized_idle_message_sender, bytes) = bytes.split_at(12);
        let (serialized_message_receiver, bytes) = bytes.split_at(12);
        assert!(bytes.is_empty());
        unsafe {
            Self {
                main_thread_id: u32::from_ne_bytes(serialized_main_thread_id.try_into().unwrap()),
                initialized_message_sender: Sender::deserialize_from_bytes(
                    serialized_initialized_message_sender.try_into().unwrap(),
                ),
                log_message_sender: Sender::deserialize_from_bytes(
                    serialized_log_message_sender.try_into().unwrap(),
                ),
                idle_message_sender: Sender::deserialize_from_bytes(
                    serialized_idle_message_sender.try_into().unwrap(),
                ),
                message_receiver: Receiver::deserialize_from_bytes(
                    serialized_message_receiver.try_into().unwrap(),
                ),
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ConductorMessage {
    Resume,
    AdvanceTime(Duration),
    SetKeyState { id: u8, state: bool },
    SetMousePosition { x: u16, y: u16 },
    SetMouseButtonState { button: MouseButton, state: bool },
    IdleRequest,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    X1,
    X2,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InitializedMessage;

#[derive(Debug, Serialize, Deserialize)]
pub struct LogMessage {
    pub level: LogLevel,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct IdleMessage;

#[derive(Debug, Serialize, Deserialize)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warning,
    Error,
}

#[derive(Debug, Error)]
#[error("failed to send message")]
pub enum SendError {
    EventWait(#[from] event::WaitError),
    Bincode(#[from] bincode::Error),
    EventSet(#[from] event::SetError),
    EventReset(#[from] event::ResetError),
    Os(#[from] io::Error),
}

#[derive(Debug, Error)]
#[error("failed to receive message")]
pub enum ReceiveError {
    Bincode(#[from] bincode::Error),
    EventGet(#[from] event::GetError),
    EventWait(#[from] event::WaitError),
    EventSet(#[from] event::SetError),
    EventReset(#[from] event::ResetError),
}

#[derive(Debug, Error)]
#[error("failed to clone sender")]
pub enum SenderCloneError {
    HandleClone(#[from] handle::CloneError),
}
