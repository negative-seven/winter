use crate::windows::handle::{self, handle_wrapper};
use std::io;
use thiserror::Error;
use winapi::{
    shared::{
        minwindef::{FALSE, TRUE},
        ntdef::NULL,
        winerror::WAIT_TIMEOUT,
    },
    um::{
        minwinbase::SECURITY_ATTRIBUTES,
        synchapi::{CreateEventA, ResetEvent, SetEvent, WaitForSingleObject},
        winbase::{WAIT_FAILED, WAIT_OBJECT_0},
    },
};

handle_wrapper!(ManualResetEvent);

impl ManualResetEvent {
    pub fn new() -> Result<Self, NewError> {
        let security_attributes = SECURITY_ATTRIBUTES {
            #[expect(clippy::cast_possible_truncation)]
            nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: NULL,
            bInheritHandle: FALSE,
        };

        unsafe {
            let handle = CreateEventA(
                std::ptr::addr_of!(security_attributes).cast_mut(),
                TRUE,
                FALSE,
                NULL.cast(),
            );
            if handle == NULL {
                return Err(io::Error::last_os_error().into());
            }
            Ok(Self::from_raw_handle(handle))
        }
    }

    pub fn get(&self) -> Result<bool, GetError> {
        unsafe {
            match WaitForSingleObject(self.handle.as_raw(), 0) {
                WAIT_OBJECT_0 => Ok(true),
                WAIT_TIMEOUT => Ok(false),
                WAIT_FAILED => Err(io::Error::last_os_error().into()),
                _ => unreachable!(),
            }
        }
    }

    pub async fn wait(&self) -> Result<(), WaitError> {
        self.handle.wait().await?;
        Ok(())
    }

    pub fn set(&mut self) -> Result<(), SetError> {
        unsafe {
            if SetEvent(self.handle.as_raw()) == 0 {
                return Err(io::Error::last_os_error().into());
            }
            Ok(())
        }
    }

    pub fn reset(&mut self) -> Result<(), ResetError> {
        unsafe {
            if ResetEvent(self.handle.as_raw()) == 0 {
                return Err(io::Error::last_os_error().into());
            }
            Ok(())
        }
    }
}

#[derive(Debug, Error)]
#[error("failed to create event")]
pub struct NewError(#[from] io::Error);

#[derive(Debug, Error)]
#[error("failed to clone event")]
pub struct CloneError(#[from] handle::CloneError);

#[derive(Debug, Error)]
#[error("failed to get event state")]
pub struct GetError(#[from] io::Error);

#[derive(Debug, Error)]
#[error("failed to wait for event")]
pub enum WaitError {
    HandleWait(#[from] handle::WaitError),
}

#[derive(Debug, Error)]
#[error("failed to set event to signaled state")]
pub struct SetError(#[from] io::Error);

#[derive(Debug, Error)]
#[error("failed to reset event to non-signaled state")]
pub struct ResetError(#[from] io::Error);
