use crate::handle::{self, Handle};
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
        winbase::{INFINITE, WAIT_FAILED, WAIT_OBJECT_0},
    },
};

#[derive(Debug)]
#[allow(clippy::module_name_repetitions)]
pub struct ManualResetEvent {
    handle: Handle,
}

impl ManualResetEvent {
    pub fn new() -> Result<Self, NewError> {
        let security_attributes = SECURITY_ATTRIBUTES {
            #[allow(clippy::cast_possible_truncation)]
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: NULL,
            bInheritHandle: TRUE,
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
            Ok(Self::from_handle(Handle::from_raw(handle)))
        }
    }

    #[must_use]
    pub unsafe fn from_handle(handle: Handle) -> Self {
        Self { handle }
    }

    pub fn try_clone(&self) -> Result<Self, CloneError> {
        unsafe { Ok(Self::from_handle(self.handle.try_clone()?)) }
    }

    #[must_use]
    pub unsafe fn handle(&self) -> &Handle {
        &self.handle
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

    pub fn wait(&self) -> Result<(), GetError> {
        unsafe {
            match WaitForSingleObject(self.handle.as_raw(), INFINITE) {
                WAIT_OBJECT_0 => Ok(()),
                WAIT_FAILED => Err(io::Error::last_os_error().into()),
                _ => unreachable!(),
            }
        }
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
#[error("failed to set event to signaled state")]
pub struct SetError(#[from] io::Error);

#[derive(Debug, Error)]
#[error("failed to reset event to non-signaled state")]
pub struct ResetError(#[from] io::Error);
