use crate::pipe;
use protocol::{Parcel, Protocol};
use std::io::{self, Read, Write};
use thiserror::Error;
use tracing::{debug, error, instrument};

#[derive(Debug)]
pub struct RuntimeTransceiver {
    writer: pipe::Writer,
    reader: pipe::Reader,
}

impl RuntimeTransceiver {
    #[must_use]
    pub fn new(writer: pipe::Writer, reader: pipe::Reader) -> Self {
        Self { writer, reader }
    }

    #[instrument]
    pub fn send(&mut self, message: &RuntimeMessage) -> Result<(), WriteError> {
        debug!("sending runtime message");
        transceiver_send(&mut self.writer, message)
    }

    #[instrument]
    pub fn receive(&mut self) -> Result<Option<HooksMessage>, ReadError> {
        transceiver_receive(&mut self.reader)
    }
}

#[derive(Debug)]
pub struct HooksTransceiver {
    writer: pipe::Writer,
    reader: pipe::Reader,
}

impl HooksTransceiver {
    #[must_use]
    pub fn new(writer: pipe::Writer, reader: pipe::Reader) -> Self {
        Self { writer, reader }
    }

    #[instrument]
    pub fn send(&mut self, message: &HooksMessage) -> Result<(), WriteError> {
        debug!("sending hooks message");
        transceiver_send(&mut self.writer, message)
    }

    #[instrument]
    pub fn receive(&mut self) -> Result<Option<RuntimeMessage>, ReadError> {
        transceiver_receive(&mut self.reader)
    }

    #[must_use]
    #[allow(clippy::missing_panics_doc)]
    pub fn from_bytes(bytes: [u8; 8]) -> Self {
        let writer_handle = u32::from_ne_bytes(bytes[0..4].try_into().unwrap());
        let reader_handle = u32::from_ne_bytes(bytes[4..8].try_into().unwrap());

        unsafe {
            Self::new(
                pipe::Writer::new(writer_handle as _),
                pipe::Reader::new(reader_handle as _),
            )
        }
    }

    #[must_use]
    pub fn to_bytes(&self) -> [u8; 8] {
        let mut bytes: [u8; 8] = Default::default();
        bytes[0..4].copy_from_slice(&(self.writer.handle as u32).to_ne_bytes());
        bytes[4..8].copy_from_slice(&(self.reader.handle as u32).to_ne_bytes());
        bytes
    }
}

fn transceiver_send<W: Write, M: Parcel>(write: &mut W, message: &M) -> Result<(), WriteError> {
    let bytes = message
        .raw_bytes(&protocol::Settings::default())
        .map_err(|_| WriteError::Protocol)?;
    #[allow(clippy::cast_possible_truncation)]
    write.write_all(&(bytes.len() as u32).to_ne_bytes())?;
    write.write_all(&bytes)?;
    Ok(())
}

fn transceiver_receive<R: Read, M: Parcel>(read: &mut R) -> Result<Option<M>, ReadError> {
    let size = {
        let mut bytes = [0; std::mem::size_of::<u32>()];
        if !read_exact_or_nothing(read, &mut bytes)? {
            return Ok(None);
        }
        u32::from_ne_bytes(bytes) as usize
    };
    let mut bytes = vec![0; size];
    if !read_exact_or_nothing(read, &mut bytes)? {
        return Ok(None);
    }
    M::from_raw_bytes(&bytes, &protocol::Settings::default())
        .map(Some)
        .map_err(|_| ReadError::Protocol)
}

fn read_exact_or_nothing<R: Read>(read: &mut R, bytes: &mut [u8]) -> Result<bool, io::Error> {
    let mut count = 0;
    loop {
        let partial_count = read.read(&mut bytes[count..])?;
        if partial_count == 0 {
            break;
        }
        count += partial_count;
    }
    if count == 0 {
        Ok(false)
    } else if count < bytes.len() {
        Err(io::Error::from(io::ErrorKind::UnexpectedEof))
    } else if count == bytes.len() {
        Ok(true)
    } else {
        unreachable!();
    }
}

pub fn new_transceiver_pair(
) -> Result<(RuntimeTransceiver, HooksTransceiver), NewTransceiverPairError> {
    let (pipe_0_writer, pipe_0_reader) = pipe::new()?;
    let (pipe_1_writer, pipe_1_reader) = pipe::new()?;

    Ok((
        RuntimeTransceiver::new(pipe_0_writer, pipe_1_reader),
        HooksTransceiver::new(pipe_1_writer, pipe_0_reader),
    ))
}

#[derive(Debug, Error)]
#[error("failed to create transceiver pair")]
pub struct NewTransceiverPairError(#[from] pipe::NewError);

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
#[error("error during transceiver write")]
pub enum WriteError {
    Protocol,
    Io(#[from] io::Error),
}

// protocol::Error isn't stored inside as it does not implement Sync
#[derive(Debug, Error)]
#[error("error during transceiver read")]
pub enum ReadError {
    Protocol,
    Io(#[from] io::Error),
}
