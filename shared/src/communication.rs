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
};
use thiserror::Error;
use tracing::{debug, error, instrument};

#[derive(Debug)]
pub struct Transceiver<S, R>
where
    S: Serialize + for<'de> Deserialize<'de> + Debug,
    R: Serialize + for<'de> Deserialize<'de> + Debug,
{
    writer: pipe::Writer,
    reader: pipe::Reader,
    writer_send_event: ManualResetEvent,
    writer_acknowledge_event: ManualResetEvent,
    reader_send_event: ManualResetEvent,
    reader_acknowledge_event: ManualResetEvent,
    _phantom_data: PhantomData<(S, R)>, // circumvents "parameter is never used" errors
}

impl<S, R> Transceiver<S, R>
where
    S: Serialize + for<'de> Deserialize<'de> + Debug,
    R: Serialize + for<'de> Deserialize<'de> + Debug,
{
    #[instrument]
    pub fn send(&mut self, message: &S) -> Result<(), SendError> {
        debug!("sending hooks message");
        #[allow(clippy::cast_possible_truncation)]
        self.writer.write_all(&bincode::serialize(&message)?)?;
        self.writer.flush()?;
        self.writer_send_event.set()?;
        self.writer_acknowledge_event.wait()?;
        self.writer_acknowledge_event.reset()?;
        Ok(())
    }

    #[instrument]
    pub fn receive(&mut self) -> Result<Option<R>, ReceiveError> {
        if !self.reader_send_event.get()? {
            return Ok(None);
        }
        self.reader_send_event.reset()?;
        let received = bincode::deserialize_from(&mut self.reader)?;
        self.reader_acknowledge_event.set()?;
        Ok(Some(received))
    }

    #[instrument]
    pub fn receive_blocking(&mut self) -> Result<R, ReceiveError> {
        self.reader_send_event.wait()?;
        self.reader_send_event.reset()?;
        let received = bincode::deserialize_from(&mut self.reader)?;
        self.reader_acknowledge_event.set()?;
        Ok(received)
    }

    #[must_use]
    #[allow(clippy::missing_panics_doc)]
    pub unsafe fn from_bytes(bytes: [u8; 24]) -> Self {
        let mut handles = bytes
            .chunks(4)
            .map(|chunk| Handle::from_raw(u32::from_ne_bytes(chunk.try_into().unwrap()) as _));

        Self {
            writer: pipe::Writer::new(handles.next().unwrap()),
            reader: pipe::Reader::new(handles.next().unwrap()),
            writer_send_event: ManualResetEvent::from_handle(handles.next().unwrap()),
            writer_acknowledge_event: ManualResetEvent::from_handle(handles.next().unwrap()),
            reader_send_event: ManualResetEvent::from_handle(handles.next().unwrap()),
            reader_acknowledge_event: ManualResetEvent::from_handle(handles.next().unwrap()),
            _phantom_data: PhantomData,
        }
    }

    #[must_use]
    #[allow(clippy::missing_panics_doc)]
    pub unsafe fn leak_to_bytes(self) -> [u8; 24] {
        let bytes = [
            self.writer.handle().as_raw() as u32,
            self.reader.handle().as_raw() as u32,
            self.writer_send_event.handle().as_raw() as u32,
            self.writer_acknowledge_event.handle().as_raw() as u32,
            self.reader_send_event.handle().as_raw() as u32,
            self.reader_acknowledge_event.handle().as_raw() as u32,
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
pub fn new_transceiver_pair<P0, P1>(
) -> Result<(Transceiver<P0, P1>, Transceiver<P1, P0>), NewTransceiverPairError>
where
    P0: Serialize + for<'de> Deserialize<'de> + Debug,
    P1: Serialize + for<'de> Deserialize<'de> + Debug,
{
    let (pipe_0_writer, pipe_0_reader) = pipe::new()?;
    let (pipe_1_writer, pipe_1_reader) = pipe::new()?;
    let pipe_0_send_event = ManualResetEvent::new()?;
    let pipe_0_acknowledge_event = ManualResetEvent::new()?;
    let pipe_1_send_event = ManualResetEvent::new()?;
    let pipe_1_acknowledge_event = ManualResetEvent::new()?;
    Ok((
        Transceiver::<P0, P1> {
            writer: pipe_0_writer,
            reader: pipe_1_reader,
            writer_send_event: pipe_0_send_event.try_clone()?,
            writer_acknowledge_event: pipe_0_acknowledge_event.try_clone()?,
            reader_send_event: pipe_1_send_event.try_clone()?,
            reader_acknowledge_event: pipe_1_acknowledge_event.try_clone()?,
            _phantom_data: PhantomData,
        },
        Transceiver::<P1, P0> {
            writer: pipe_1_writer,
            reader: pipe_0_reader,
            writer_send_event: pipe_1_send_event,
            writer_acknowledge_event: pipe_1_acknowledge_event,
            reader_send_event: pipe_0_send_event,
            reader_acknowledge_event: pipe_0_acknowledge_event,
            _phantom_data: PhantomData,
        },
    ))
}

#[derive(Debug, Error)]
#[error("failed to create transceiver pair")]
pub enum NewTransceiverPairError {
    NewPipe(#[from] pipe::NewError),
    NewEvent(#[from] event::NewError),
    CloneEvent(#[from] event::CloneError),
}

#[derive(Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub enum RuntimeMessage {}

#[derive(Debug, Serialize, Deserialize)]
#[repr(u8)]
#[non_exhaustive]
pub enum HooksMessage {
    HooksInitialized,
}

// protocol::Error isn't stored inside as it does not implement Sync
#[derive(Debug, Error)]
#[error("error during transceiver send")]
pub enum SendError {
    EventGet(#[from] event::GetError),
    Bincode(#[from] bincode::Error),
    EventSet(#[from] event::SetError),
    EventReset(#[from] event::ResetError),
    Io(#[from] io::Error),
}

// protocol::Error isn't stored inside as it does not implement Sync
#[derive(Debug, Error)]
#[error("error during transceiver receive")]
pub enum ReceiveError {
    Bincode(#[from] bincode::Error),
    EventGet(#[from] event::GetError),
    EventSet(#[from] event::SetError),
    EventReset(#[from] event::ResetError),
}
