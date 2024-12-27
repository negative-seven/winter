pub mod message;

use crate::windows::{
    event::{self, ManualResetEvent},
    handle, pipe, process,
};
use serde::{Deserialize, Serialize};
use std::{
    fmt::Debug,
    io::{self, Write},
    marker::PhantomData,
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

    pub async fn send(&mut self, message: S) -> Result<(), SendError> {
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

pub fn new_sender_and_receiver<T>(
    sender_process: &process::Process,
    receiver_process: &process::Process,
) -> Result<(Sender<T>, Receiver<T>), NewSenderAndReceiverError>
where
    T: Serialize + for<'de> Deserialize<'de> + Debug + Debug,
{
    let (pipe_writer, pipe_reader) = pipe::new()?;
    let send_event = ManualResetEvent::new()?;
    let acknowledge_event = ManualResetEvent::new()?;
    Ok((
        Sender {
            pipe: pipe_writer.try_clone_for_process(sender_process)?,
            send_event: send_event.try_clone_for_process(sender_process)?,
            acknowledge_event: acknowledge_event.try_clone_for_process(sender_process)?,
            _phantom_data: PhantomData,
        },
        Receiver {
            pipe: pipe_reader.try_clone_for_process(receiver_process)?,
            send_event: send_event.try_clone_for_process(receiver_process)?,
            acknowledge_event: acknowledge_event.try_clone_for_process(receiver_process)?,
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
