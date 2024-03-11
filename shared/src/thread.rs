use crate::handle::Handle;
use std::io;
use thiserror::Error;
use tracing::{debug, instrument, Level};
use winapi::{
    shared::minwindef::FALSE,
    um::{
        processthreadsapi::{GetExitCodeThread, GetThreadId, OpenThread, ResumeThread},
        synchapi::WaitForSingleObject,
        winbase::{INFINITE, WAIT_FAILED},
        winnt::THREAD_SUSPEND_RESUME,
    },
};

#[derive(Debug)]
pub struct Thread {
    handle: Handle,
}

impl Thread {
    #[instrument(ret(level = Level::DEBUG), err)]
    pub fn from_id(id: u32) -> Result<Self, FromIdError> {
        let handle = unsafe { OpenThread(THREAD_SUSPEND_RESUME, FALSE, id) };
        if handle.is_null() {
            return Err(FromIdError(io::Error::last_os_error()));
        }

        unsafe { Ok(Self::from_handle(Handle::from_raw(handle))) }
    }

    #[instrument(ret(level = Level::DEBUG))]
    pub unsafe fn from_handle(handle: Handle) -> Self {
        Self { handle }
    }

    #[instrument(ret(level = Level::DEBUG), err)]
    pub fn get_id(&self) -> Result<u32, io::Error> {
        let id = unsafe { GetThreadId(self.handle.as_raw()) };

        if id == 0 {
            return Err(io::Error::last_os_error());
        }

        Ok(id)
    }

    #[instrument(err)]
    pub fn resume(&self) -> Result<(), ResumeError> {
        if unsafe { ResumeThread(self.handle.as_raw()) } == 0xffff_ffff {
            return Err(io::Error::last_os_error().into());
        }

        debug!("success");
        Ok(())
    }

    #[instrument(err)]
    pub fn join(&self) -> Result<u32, JoinError> {
        unsafe {
            if WaitForSingleObject(self.handle.as_raw(), INFINITE) == WAIT_FAILED {
                return Err(io::Error::last_os_error().into());
            }

            let mut exit_code = 0u32;
            if GetExitCodeThread(self.handle.as_raw(), &mut exit_code) == 0 {
                return Err(io::Error::last_os_error().into());
            }

            Ok(exit_code)
        }
    }
}

#[derive(Debug, Error)]
#[error("failed to open thread handle from id")]
pub struct FromIdError(#[from] io::Error);

#[derive(Debug, Error)]
#[error("failed to resume thread")]
pub struct ResumeError(#[from] io::Error);

#[derive(Debug, Error)]
#[error("failed to join thread")]
pub struct JoinError(#[from] io::Error);
