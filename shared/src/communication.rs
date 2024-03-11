use crate::{
    event::{self, ManualResetEvent},
    handle::Handle,
    pipe,
};
use protocol::{Parcel, Protocol};
use std::{
    fmt::Debug,
    io::{self, Read, Write},
    marker::PhantomData,
};
use thiserror::Error;
use tracing::{debug, error, instrument};

#[derive(Debug)]
pub struct Transceiver<S: Parcel + Debug, R: Parcel + Debug> {
    writer: pipe::Writer,
    reader: pipe::Reader,
    writer_event: ManualResetEvent,
    reader_event: ManualResetEvent,
    _phantom_data: PhantomData<(S, R)>, // circumvents "parameter is never used" errors
}

impl<S: Parcel + Debug, R: Parcel + Debug> Transceiver<S, R> {
    #[instrument]
    pub fn send(&mut self, message: &S) -> Result<(), SendError> {
        debug!("sending hooks message");
        #[allow(clippy::cast_possible_truncation)]
        self.writer.write_all(
            &message
                .raw_bytes(&protocol::Settings::default())
                .map_err(|_| SendError::Protocol)?,
        )?;
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
        let mut bytes = Vec::new();
        self.reader.read_to_end(&mut bytes)?;
        R::from_raw_bytes(&bytes, &protocol::Settings::default())
            .map(Some)
            .map_err(|_| ReceiveError::Protocol)
    }

    #[instrument]
    pub fn receive_blocking(&mut self) -> Result<R, ReceiveError> {
        self.reader_event.wait()?;
        self.reader_event.reset()?;
        let mut bytes = Vec::new();
        self.reader.read_to_end(&mut bytes)?;
        R::from_raw_bytes(&bytes, &protocol::Settings::default())
            .map_err(|_| ReceiveError::Protocol)
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
pub fn new_transceiver_pair<P0: Parcel + Debug, P1: Parcel + Debug>(
) -> Result<(Transceiver<P0, P1>, Transceiver<P1, P0>), NewTransceiverPairError> {
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

#[derive(Debug, Protocol)]
#[protocol(discriminant = "integer")]
#[non_exhaustive]
pub enum RuntimeMessage {}

#[derive(Debug, Protocol)]
#[protocol(discriminant = "integer")]
#[repr(u8)]
#[non_exhaustive]
pub enum HooksMessage {
    HooksInitialized,
}

// protocol::Error isn't stored inside as it does not implement Sync
#[derive(Debug, Error)]
#[error("error during transceiver send")]
pub enum SendError {
    EventSet(#[from] event::SetError),
    Io(#[from] io::Error),
    Protocol,
}

// protocol::Error isn't stored inside as it does not implement Sync
#[derive(Debug, Error)]
#[error("error during transceiver receive")]
pub enum ReceiveError {
    EventGet(#[from] event::GetError),
    EventReset(#[from] event::ResetError),
    Io(#[from] io::Error),
    Protocol,
}
