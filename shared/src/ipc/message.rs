use super::{Receiver, Sender};
use crate::input::MouseButton;
use serde::{Deserialize, Serialize};
use std::{io::Read, time::Duration};
use thiserror::Error;

pub trait Message: Sized {
    unsafe fn serialize(self) -> Result<Vec<u8>, SerializeError>;

    unsafe fn deserialize_from(reader: impl Read) -> Result<Self, DeserializeError>;

    unsafe fn deserialize(bytes: &[u8]) -> Result<Self, DeserializeError> {
        Ok(unsafe { Self::deserialize_from(std::io::Cursor::new(bytes))? })
    }
}

#[derive(Debug)]
pub struct Initial {
    pub main_thread_id: u32,
    pub initialized_message_sender: Sender<Initialized>,
    pub log_message_sender: Sender<Log>,
    pub idle_message_sender: Sender<Idle>,
    pub message_receiver: Receiver<FromConductor>,
}

impl Message for Initial {
    unsafe fn serialize(self) -> Result<Vec<u8>, SerializeError> {
        let bytes = [
            &self.main_thread_id.to_ne_bytes() as &[u8],
            &self.initialized_message_sender.serialize_to_bytes(),
            &self.log_message_sender.serialize_to_bytes(),
            &self.idle_message_sender.serialize_to_bytes(),
            &self.message_receiver.serialize_to_bytes(),
        ]
        .concat();

        unsafe {
            self.initialized_message_sender.pipe.leak_handle();
            self.initialized_message_sender.send_event.leak_handle();
            self.initialized_message_sender
                .acknowledge_event
                .leak_handle();
            self.log_message_sender.pipe.leak_handle();
            self.log_message_sender.send_event.leak_handle();
            self.log_message_sender.acknowledge_event.leak_handle();
            self.idle_message_sender.pipe.leak_handle();
            self.idle_message_sender.send_event.leak_handle();
            self.idle_message_sender.acknowledge_event.leak_handle();
            self.message_receiver.pipe.leak_handle();
            self.message_receiver.send_event.leak_handle();
            self.message_receiver.acknowledge_event.leak_handle();
        }

        Ok(bytes)
    }

    unsafe fn deserialize_from(mut reader: impl Read) -> Result<Self, DeserializeError> {
        fn read<const N: usize>(mut reader: impl Read) -> Result<[u8; N], DeserializeError> {
            let mut array = [0; N];
            reader.read_exact(&mut array)?;
            Ok(array)
        }

        let serialized_main_thread_id = read::<4>(&mut reader)?;
        let serialized_initialized_message_sender = read::<12>(&mut reader)?;
        let serialized_log_message_sender = read::<12>(&mut reader)?;
        let serialized_idle_message_sender = read::<12>(&mut reader)?;
        let serialized_message_receiver = read::<12>(&mut reader)?;
        unsafe {
            Ok(Self {
                main_thread_id: u32::from_ne_bytes(serialized_main_thread_id),
                initialized_message_sender: Sender::deserialize_from_bytes(
                    serialized_initialized_message_sender,
                ),
                log_message_sender: Sender::deserialize_from_bytes(serialized_log_message_sender),
                idle_message_sender: Sender::deserialize_from_bytes(serialized_idle_message_sender),
                message_receiver: Receiver::deserialize_from_bytes(serialized_message_receiver),
            })
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub enum FromConductor {
    Resume,
    AdvanceTime(Duration),
    SetKeyState { id: u8, state: bool },
    SetMousePosition { x: u16, y: u16 },
    SetMouseButtonState { button: MouseButton, state: bool },
    IdleRequest,
}

impl Message for FromConductor {
    unsafe fn serialize(self) -> Result<Vec<u8>, SerializeError> {
        Ok(bincode::serialize(&self)?)
    }

    unsafe fn deserialize_from(reader: impl Read) -> Result<Self, DeserializeError> {
        Ok(bincode::deserialize_from(reader)?)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Initialized;

impl Message for Initialized {
    unsafe fn serialize(self) -> Result<Vec<u8>, SerializeError> {
        Ok(bincode::serialize(&self)?)
    }

    unsafe fn deserialize_from(reader: impl Read) -> Result<Self, DeserializeError> {
        Ok(bincode::deserialize_from(reader)?)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Log {
    pub level: LogLevel,
    pub message: String,
}

impl Message for Log {
    unsafe fn serialize(self) -> Result<Vec<u8>, SerializeError> {
        Ok(bincode::serialize(&self)?)
    }

    unsafe fn deserialize_from(reader: impl Read) -> Result<Self, DeserializeError> {
        Ok(bincode::deserialize_from(reader)?)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warning,
    Error,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Idle;

impl Message for Idle {
    unsafe fn serialize(self) -> Result<Vec<u8>, SerializeError> {
        Ok(bincode::serialize(&self)?)
    }

    unsafe fn deserialize_from(reader: impl Read) -> Result<Self, DeserializeError> {
        Ok(bincode::deserialize_from(reader)?)
    }
}

#[derive(Debug, Error)]
#[error("failed to serialize message")]
pub enum SerializeError {
    Bincode(#[from] Box<bincode::ErrorKind>),
}

#[derive(Debug, Error)]
#[error("failed to deserialize message")]
pub enum DeserializeError {
    Bincode(#[from] Box<bincode::ErrorKind>),
    Io(#[from] std::io::Error),
}
