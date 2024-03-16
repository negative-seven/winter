use crate::{
    event::{self, ManualResetEvent},
    handle::Handle,
    pipe,
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

    pub fn send(&mut self, message: &S) -> Result<(), SendError> {
        #[allow(clippy::cast_possible_truncation)]
        self.pipe.write_all(&bincode::serialize(&message)?)?;
        self.pipe.flush()?;
        self.send_event.set()?;
        self.acknowledge_event.wait()?;
        self.acknowledge_event.reset()?;
        Ok(())
    }

    #[must_use]
    #[allow(clippy::missing_panics_doc)]
    pub unsafe fn from_bytes(bytes: [u8; 12]) -> Self {
        let mut handles = bytes
            .chunks(4)
            .map(|chunk| Handle::from_raw(u32::from_ne_bytes(chunk.try_into().unwrap()) as _));

        Self {
            pipe: pipe::Writer::new(handles.next().unwrap()),
            send_event: ManualResetEvent::from_handle(handles.next().unwrap()),
            acknowledge_event: ManualResetEvent::from_handle(handles.next().unwrap()),
            _phantom_data: PhantomData,
        }
    }

    #[must_use]
    #[allow(clippy::missing_panics_doc)]
    pub unsafe fn leak_to_bytes(self) -> [u8; 12] {
        let bytes = [
            self.pipe.handle().as_raw() as u32,
            self.send_event.handle().as_raw() as u32,
            self.acknowledge_event.handle().as_raw() as u32,
        ]
        .iter()
        .flat_map(|h| h.to_ne_bytes())
        .collect::<Vec<_>>()
        .try_into()
        .unwrap();
        std::mem::forget(self);
        bytes
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
    pub fn receive(&mut self) -> Result<Option<R>, ReceiveError> {
        if !self.send_event.get()? {
            return Ok(None);
        }
        self.send_event.reset()?;
        let received = bincode::deserialize_from(&mut self.pipe)?;
        self.acknowledge_event.set()?;
        Ok(Some(received))
    }

    pub fn receive_blocking(&mut self) -> Result<R, ReceiveError> {
        self.send_event.wait()?;
        self.send_event.reset()?;
        let received = bincode::deserialize_from(&mut self.pipe)?;
        self.acknowledge_event.set()?;
        Ok(received)
    }

    #[must_use]
    #[allow(clippy::missing_panics_doc)]
    pub unsafe fn from_bytes(bytes: [u8; 12]) -> Self {
        let mut handles = bytes
            .chunks(4)
            .map(|chunk| Handle::from_raw(u32::from_ne_bytes(chunk.try_into().unwrap()) as _));

        Self {
            pipe: pipe::Reader::new(handles.next().unwrap()),
            send_event: ManualResetEvent::from_handle(handles.next().unwrap()),
            acknowledge_event: ManualResetEvent::from_handle(handles.next().unwrap()),
            _phantom_data: PhantomData,
        }
    }

    #[must_use]
    #[allow(clippy::missing_panics_doc)]
    pub unsafe fn leak_to_bytes(self) -> [u8; 12] {
        let bytes = [
            self.pipe.handle().as_raw() as u32,
            self.send_event.handle().as_raw() as u32,
            self.acknowledge_event.handle().as_raw() as u32,
        ]
        .iter()
        .flat_map(|h| h.to_ne_bytes())
        .collect::<Vec<_>>()
        .try_into()
        .unwrap();
        std::mem::forget(self);
        bytes
    }
}

#[allow(clippy::type_complexity)]
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
    CloneEvent(#[from] event::CloneError),
}

#[derive(Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub enum RuntimeMessage {
    AdvanceTime(Duration),
    SetKeyState { id: u8, state: bool },
    IdleRequest,
}

#[derive(Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub enum HooksMessage {
    HooksInitialized,
    Idle,
    Stop,
    Log { level: LogLevel, message: String },
}

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
    EventGet(#[from] event::GetError),
    Bincode(#[from] bincode::Error),
    EventSet(#[from] event::SetError),
    EventReset(#[from] event::ResetError),
    Io(#[from] io::Error),
}

#[derive(Debug, Error)]
#[error("failed to receive message")]
pub enum ReceiveError {
    Bincode(#[from] bincode::Error),
    EventGet(#[from] event::GetError),
    EventSet(#[from] event::SetError),
    EventReset(#[from] event::ResetError),
}

#[derive(Debug, Error)]
#[error("failed to clone sender")]
pub enum SenderCloneError {
    PipeWriterClone(#[from] pipe::WriterCloneError),
    EventClone(#[from] event::CloneError),
}
