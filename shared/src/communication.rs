use crate::{
    event::{self, ManualResetEvent},
    handle::Handle,
    pipe,
};
use protocol::{Parcel, Protocol};
use std::io::{self, Read, Write};
use thiserror::Error;
use tracing::{debug, error, instrument};

#[derive(Debug)]
pub struct RuntimeTransceiver {
    writer: pipe::Writer,
    reader: pipe::Reader,
    writer_event: ManualResetEvent,
    reader_event: ManualResetEvent,
}

impl RuntimeTransceiver {
    #[instrument]
    pub fn send(&mut self, message: &RuntimeMessage) -> Result<(), SendError> {
        debug!("sending runtime message");
        transceiver_send(&mut self.writer, &mut self.writer_event, message)
    }

    #[instrument]
    pub fn receive(&mut self) -> Result<Option<HooksMessage>, ReceiveError> {
        transceiver_receive(&mut self.reader, &mut self.reader_event)
    }

    #[instrument]
    pub fn receive_blocking(&mut self) -> Result<HooksMessage, ReceiveError> {
        transceiver_receive_blocking(&mut self.reader, &mut self.reader_event)
    }
}

#[derive(Debug)]
pub struct HooksTransceiver {
    writer: pipe::Writer,
    reader: pipe::Reader,
    writer_event: ManualResetEvent,
    reader_event: ManualResetEvent,
}

impl HooksTransceiver {
    const SIZE_IN_BYTES: usize = 16;

    #[instrument]
    pub fn send(&mut self, message: &HooksMessage) -> Result<(), SendError> {
        debug!("sending hooks message");
        transceiver_send(&mut self.writer, &mut self.writer_event, message)
    }

    #[instrument]
    pub fn receive(&mut self) -> Result<Option<RuntimeMessage>, ReceiveError> {
        transceiver_receive(&mut self.reader, &mut self.reader_event)
    }

    #[instrument]
    pub fn receive_blocking(&mut self) -> Result<RuntimeMessage, ReceiveError> {
        transceiver_receive_blocking(&mut self.reader, &mut self.reader_event)
    }

    #[must_use]
    #[allow(clippy::missing_panics_doc)]
    pub fn from_bytes(bytes: [u8; Self::SIZE_IN_BYTES]) -> Self {
        let writer_handle = u32::from_ne_bytes(bytes[0..4].try_into().unwrap());
        let reader_handle = u32::from_ne_bytes(bytes[4..8].try_into().unwrap());
        let writer_event_handle = u32::from_ne_bytes(bytes[8..12].try_into().unwrap());
        let reader_event_handle = u32::from_ne_bytes(bytes[12..16].try_into().unwrap());

        unsafe {
            Self {
                writer: pipe::Writer::new(Handle::from_raw(writer_handle as _)),
                reader: pipe::Reader::new(Handle::from_raw(reader_handle as _)),
                writer_event: ManualResetEvent::from_handle(Handle::from_raw(
                    writer_event_handle as _,
                )),
                reader_event: ManualResetEvent::from_handle(Handle::from_raw(
                    reader_event_handle as _,
                )),
            }
        }
    }

    #[must_use]
    #[allow(clippy::missing_panics_doc)]
    pub unsafe fn leak_to_bytes(self) -> [u8; Self::SIZE_IN_BYTES] {
        unsafe {
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
}

fn transceiver_send<W: Write, M: Parcel>(
    write: &mut W,
    event: &mut ManualResetEvent,
    message: &M,
) -> Result<(), SendError> {
    #[allow(clippy::cast_possible_truncation)]
    write.write_all(
        &message
            .raw_bytes(&protocol::Settings::default())
            .map_err(|_| SendError::Protocol)?,
    )?;
    write.flush()?;
    event.set()?;
    Ok(())
}

fn transceiver_receive<R: Read, M: Parcel>(
    read: &mut R,
    event: &mut ManualResetEvent,
) -> Result<Option<M>, ReceiveError> {
    if !event.get()? {
        return Ok(None);
    }
    event.reset()?;
    let mut bytes = Vec::new();
    read.read_to_end(&mut bytes)?;
    M::from_raw_bytes(&bytes, &protocol::Settings::default())
        .map(Some)
        .map_err(|_| ReceiveError::Protocol)
}

fn transceiver_receive_blocking<R: Read, M: Parcel>(
    read: &mut R,
    event: &mut ManualResetEvent,
) -> Result<M, ReceiveError> {
    event.wait()?;
    event.reset()?;
    let mut bytes = Vec::new();
    read.read_to_end(&mut bytes)?;
    M::from_raw_bytes(&bytes, &protocol::Settings::default()).map_err(|_| ReceiveError::Protocol)
}

pub fn new_transceiver_pair(
) -> Result<(RuntimeTransceiver, HooksTransceiver), NewTransceiverPairError> {
    let (pipe_0_writer, pipe_0_reader) = pipe::new()?;
    let (pipe_1_writer, pipe_1_reader) = pipe::new()?;
    let pipe_0_event = ManualResetEvent::new()?;
    let pipe_1_event = ManualResetEvent::new()?;
    Ok((
        RuntimeTransceiver {
            writer: pipe_0_writer,
            reader: pipe_1_reader,
            writer_event: pipe_0_event.try_clone()?,
            reader_event: pipe_1_event.try_clone()?,
        },
        HooksTransceiver {
            writer: pipe_1_writer,
            reader: pipe_0_reader,
            writer_event: pipe_1_event,
            reader_event: pipe_0_event,
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
