use super::{Receiver, Sender};
use crate::input::MouseButton;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug)]
pub struct Initial {
    pub main_thread_id: u32,
    pub initialized_message_sender: Sender<Initialized>,
    pub log_message_sender: Sender<Log>,
    pub idle_message_sender: Sender<Idle>,
    pub message_receiver: Receiver<FromConductor>,
}

impl Initial {
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
pub enum FromConductor {
    Resume,
    AdvanceTime(Duration),
    SetKeyState { id: u8, state: bool },
    SetMousePosition { x: u16, y: u16 },
    SetMouseButtonState { button: MouseButton, state: bool },
    IdleRequest,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Initialized;

#[derive(Debug, Serialize, Deserialize)]
pub struct Log {
    pub level: LogLevel,
    pub message: String,
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
