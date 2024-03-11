use crate::process::Process;
use std::io;
use thiserror::Error;
use winapi::{
    ctypes::c_void,
    shared::{
        minwindef::{FALSE, TRUE},
        ntdef::NULL,
        winerror::WAIT_TIMEOUT,
    },
    um::{
        handleapi::DuplicateHandle,
        minwinbase::SECURITY_ATTRIBUTES,
        synchapi::{CreateEventA, ResetEvent, SetEvent, WaitForSingleObject},
        winbase::{INFINITE, WAIT_FAILED, WAIT_OBJECT_0},
        winnt::DUPLICATE_SAME_ACCESS,
    },
};

#[derive(Debug)]
#[allow(clippy::module_name_repetitions)]
pub struct ManualResetEvent {
    handle: *mut c_void,
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
            Ok(Self::from_handle(handle))
        }
    }

    pub unsafe fn from_handle(handle: *mut c_void) -> Self {
        Self { handle }
    }

    pub fn try_clone(&self) -> Result<Self, CloneError> {
        unsafe {
            let current_process_handle = Process::get_current().handle();
            let mut duplicated_handle = NULL;
            if DuplicateHandle(
                current_process_handle,
                self.handle,
                current_process_handle,
                &mut duplicated_handle,
                0,
                TRUE,
                DUPLICATE_SAME_ACCESS,
            ) == 0
            {
                return Err(io::Error::last_os_error().into());
            }
            Ok(Self::from_handle(duplicated_handle))
        }
    }

    #[must_use]
    pub unsafe fn handle(&self) -> *mut c_void {
        self.handle
    }

    pub fn get(&self) -> Result<bool, GetError> {
        unsafe {
            match WaitForSingleObject(self.handle, 0) {
                WAIT_OBJECT_0 => Ok(true),
                WAIT_TIMEOUT => Ok(false),
                WAIT_FAILED => Err(io::Error::last_os_error().into()),
                _ => unreachable!(),
            }
        }
    }

    pub fn wait(&self) -> Result<(), GetError> {
        unsafe {
            match WaitForSingleObject(self.handle, INFINITE) {
                WAIT_OBJECT_0 => Ok(()),
                WAIT_FAILED => Err(io::Error::last_os_error().into()),
                _ => unreachable!(),
            }
        }
    }

    pub fn set(&mut self) -> Result<(), SetError> {
        unsafe {
            if SetEvent(self.handle) == 0 {
                return Err(io::Error::last_os_error().into());
            }
            Ok(())
        }
    }

    pub fn reset(&mut self) -> Result<(), ResetError> {
        unsafe {
            if ResetEvent(self.handle) == 0 {
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
pub struct CloneError(#[from] io::Error);

#[derive(Debug, Error)]
#[error("failed to get event state")]
pub struct GetError(#[from] io::Error);

#[derive(Debug, Error)]
#[error("failed to set event to signaled state")]
pub struct SetError(#[from] io::Error);

#[derive(Debug, Error)]
#[error("failed to reset event to non-signaled state")]
pub struct ResetError(#[from] io::Error);
