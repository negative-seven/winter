use std::io;
use thiserror::Error;
use tracing::{debug, instrument, Level};
use winapi::{
    ctypes::c_void,
    shared::minwindef::FALSE,
    um::{
        handleapi::CloseHandle,
        processthreadsapi::{GetThreadId, OpenThread, ResumeThread},
        winnt::THREAD_SUSPEND_RESUME,
    },
};

#[derive(Debug)]
pub struct Thread {
    handle: *mut c_void,
}

impl Thread {
    #[instrument(ret(level = Level::DEBUG), err)]
    pub fn from_id(id: u32) -> Result<Self, FromIdError> {
        let handle = unsafe { OpenThread(THREAD_SUSPEND_RESUME, FALSE, id) };
        if handle.is_null() {
            return Err(FromIdError(io::Error::last_os_error()));
        }

        unsafe { Ok(Self::from_handle(handle)) }
    }

    #[instrument(ret(level = Level::DEBUG))]
    pub unsafe fn from_handle(handle: *mut c_void) -> Self {
        Self { handle }
    }

    #[instrument(ret(level = Level::DEBUG), err)]
    pub fn get_id(&self) -> Result<u32, io::Error> {
        let id = unsafe { GetThreadId(self.handle) };

        if id == 0 {
            return Err(io::Error::last_os_error());
        }

        Ok(id)
    }

    #[instrument(err)]
    pub fn resume(&self) -> Result<(), ResumeError> {
        if unsafe { ResumeThread(self.handle) } == 0xffff_ffff {
            return Err(io::Error::last_os_error().into());
        }

        debug!("success");
        Ok(())
    }
}

impl Drop for Thread {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.handle) };
    }
}

#[derive(Debug, Error)]
#[error("failed to open thread handle from id")]
pub struct FromIdError(#[from] io::Error);

#[derive(Debug, Error)]
#[error("failed to resume thread")]
pub struct ResumeError(#[from] io::Error);
