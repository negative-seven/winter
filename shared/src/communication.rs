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
    writer_event: ManualResetEvent,
    reader_event: ManualResetEvent,
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
        self.writer_event.set()?;
        Ok(())
    }

    #[instrument]
    pub fn receive(&mut self) -> Result<Option<R>, ReceiveError> {
        if !self.reader_event.get()? {
            return Ok(None);
        }
        self.reader_event.reset()?;
        Ok(bincode::deserialize_from(&mut self.reader)?)
    }

    #[instrument]
    pub fn receive_blocking(&mut self) -> Result<R, ReceiveError> {
        self.reader_event.wait()?;
        self.reader_event.reset()?;
        Ok(bincode::deserialize_from(&mut self.reader)?)
    }

    #[must_use]
    #[allow(clippy::missing_panics_doc)]
    pub unsafe fn from_bytes(bytes: [u8; 16]) -> Self {
        let writer_handle = u32::from_ne_bytes(bytes[0..4].try_into().unwrap());
        let reader_handle = u32::from_ne_bytes(bytes[4..8].try_into().unwrap());
        let writer_event_handle = u32::from_ne_bytes(bytes[8..12].try_into().unwrap());
        let reader_event_handle = u32::from_ne_bytes(bytes[12..16].try_into().unwrap());

        Self {
            writer: pipe::Writer::new(Handle::from_raw(writer_handle as _)),
            reader: pipe::Reader::new(Handle::from_raw(reader_handle as _)),
            writer_event: ManualResetEvent::from_handle(Handle::from_raw(writer_event_handle as _)),
            reader_event: ManualResetEvent::from_handle(Handle::from_raw(reader_event_handle as _)),
            _phantom_data: PhantomData,
        }
    }

    #[must_use]
    #[allow(clippy::missing_panics_doc)]
    pub unsafe fn leak_to_bytes(self) -> [u8; 16] {
        let bytes = [
            self.writer.handle().as_raw() as u32,
            self.reader.handle().as_raw() as u32,
            self.writer_event.handle().as_raw() as u32,
            self.reader_event.handle().as_raw() as u32,
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
    let pipe_0_event = ManualResetEvent::new()?;
    let pipe_1_event = ManualResetEvent::new()?;
    Ok((
        Transceiver::<P0, P1> {
            writer: pipe_0_writer,
            reader: pipe_1_reader,
            writer_event: pipe_0_event.try_clone()?,
            reader_event: pipe_1_event.try_clone()?,
            _phantom_data: PhantomData,
        },
        Transceiver::<P1, P0> {
            writer: pipe_1_writer,
            reader: pipe_0_reader,
            writer_event: pipe_1_event,
            reader_event: pipe_0_event,
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
    Bincode(#[from] bincode::Error),
    EventSet(#[from] event::SetError),
    Io(#[from] io::Error),
}

// protocol::Error isn't stored inside as it does not implement Sync
#[derive(Debug, Error)]
#[error("error during transceiver receive")]
pub enum ReceiveError {
    Bincode(#[from] bincode::Error),
    EventGet(#[from] event::GetError),
    EventReset(#[from] event::ResetError),
}
