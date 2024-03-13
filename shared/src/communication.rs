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
    sync::Mutex,
    time::Duration,
};
use thiserror::Error;
use tracing::{debug, error, instrument};

#[derive(Debug)]
pub struct Transceiver<S, R>
where
    S: Serialize + for<'de> Deserialize<'de> + Debug,
    R: Serialize + for<'de> Deserialize<'de> + Debug,
{
    writer: Mutex<TransceiverWriter>,
    reader: Mutex<TransceiverReader>,
    _phantom_data: PhantomData<(S, R)>, // circumvents "parameter is never used" errors
}

impl<S, R> Transceiver<S, R>
where
    S: Serialize + for<'de> Deserialize<'de> + Debug,
    R: Serialize + for<'de> Deserialize<'de> + Debug,
{
    #[instrument]
    pub fn send(&self, message: &S) -> Result<(), SendError> {
        debug!("sending message");
        let mut writer = self.writer.lock().unwrap();
        #[allow(clippy::cast_possible_truncation)]
        writer.pipe.write_all(&bincode::serialize(&message)?)?;
        writer.pipe.flush()?;
        writer.send_event.set()?;
        writer.acknowledge_event.wait()?;
        writer.acknowledge_event.reset()?;
        Ok(())
    }

    #[instrument]
    pub fn receive(&self) -> Result<Option<R>, ReceiveError> {
        let mut reader = self.reader.lock().unwrap();
        if !reader.send_event.get()? {
            return Ok(None);
        }
        reader.send_event.reset()?;
        let received = bincode::deserialize_from(&mut reader.pipe)?;
        reader.acknowledge_event.set()?;
        Ok(Some(received))
    }

    #[instrument]
    pub fn receive_blocking(&self) -> Result<R, ReceiveError> {
        let mut reader = self.reader.lock().unwrap();
        reader.send_event.wait()?;
        reader.send_event.reset()?;
        let received = bincode::deserialize_from(&mut reader.pipe)?;
        reader.acknowledge_event.set()?;
        Ok(received)
    }

    #[must_use]
    #[allow(clippy::missing_panics_doc)]
    pub unsafe fn from_bytes(bytes: [u8; 24]) -> Self {
        let mut handles = bytes
            .chunks(4)
            .map(|chunk| Handle::from_raw(u32::from_ne_bytes(chunk.try_into().unwrap()) as _));

        Self {
            writer: Mutex::new(TransceiverWriter {
                pipe: pipe::Writer::new(handles.next().unwrap()),
                send_event: ManualResetEvent::from_handle(handles.next().unwrap()),
                acknowledge_event: ManualResetEvent::from_handle(handles.next().unwrap()),
            }),
            reader: Mutex::new(TransceiverReader {
                pipe: pipe::Reader::new(handles.next().unwrap()),
                send_event: ManualResetEvent::from_handle(handles.next().unwrap()),
                acknowledge_event: ManualResetEvent::from_handle(handles.next().unwrap()),
            }),
            _phantom_data: PhantomData,
        }
    }

    #[must_use]
    #[allow(clippy::missing_panics_doc)]
    pub unsafe fn leak_to_bytes(self) -> [u8; 24] {
        let writer = self.writer.into_inner().unwrap();
        let reader = self.reader.into_inner().unwrap();
        let bytes = [
            writer.pipe.handle().as_raw() as u32,
            writer.send_event.handle().as_raw() as u32,
            writer.acknowledge_event.handle().as_raw() as u32,
            reader.pipe.handle().as_raw() as u32,
            reader.send_event.handle().as_raw() as u32,
            reader.acknowledge_event.handle().as_raw() as u32,
        ]
        .iter()
        .flat_map(|h| h.to_ne_bytes())
        .collect::<Vec<_>>()
        .try_into()
        .unwrap();
        std::mem::forget(writer);
        std::mem::forget(reader);
        bytes
    }
}

#[derive(Debug)]
struct TransceiverWriter {
    pipe: pipe::Writer,
    send_event: ManualResetEvent,
    acknowledge_event: ManualResetEvent,
}

#[derive(Debug)]
struct TransceiverReader {
    pipe: pipe::Reader,
    send_event: ManualResetEvent,
    acknowledge_event: ManualResetEvent,
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
            writer: Mutex::new(TransceiverWriter {
                pipe: pipe_0_writer,
                send_event: pipe_0_send_event.try_clone()?,
                acknowledge_event: pipe_0_acknowledge_event.try_clone()?,
            }),
            reader: Mutex::new(TransceiverReader {
                pipe: pipe_1_reader,
                send_event: pipe_1_send_event.try_clone()?,
                acknowledge_event: pipe_1_acknowledge_event.try_clone()?,
            }),
            _phantom_data: PhantomData,
        },
        Transceiver::<P1, P0> {
            writer: Mutex::new(TransceiverWriter {
                pipe: pipe_1_writer,
                send_event: pipe_1_send_event,
                acknowledge_event: pipe_1_acknowledge_event,
            }),
            reader: Mutex::new(TransceiverReader {
                pipe: pipe_0_reader,
                send_event: pipe_0_send_event,
                acknowledge_event: pipe_0_acknowledge_event,
            }),
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
#[repr(u8)]
#[non_exhaustive]
pub enum RuntimeMessage {
    AdvanceTime(Duration),
}

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
