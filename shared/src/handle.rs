use crate::process;
use std::io;
use thiserror::Error;
use winapi::{
    ctypes::c_void,
    shared::{minwindef::TRUE, ntdef::NULL},
    um::{
        handleapi::{CloseHandle, DuplicateHandle},
        winnt::DUPLICATE_SAME_ACCESS,
    },
};

#[derive(Debug)]
pub struct Handle(*mut c_void);

impl Handle {
    pub unsafe fn from_raw(raw_handle: *mut c_void) -> Self {
        Self(raw_handle)
    }

    #[must_use]
    pub unsafe fn as_raw(&self) -> *mut c_void {
        self.0
    }

    #[must_use]
    pub unsafe fn leak(self) -> *mut c_void {
        let raw = self.0;
        std::mem::forget(self);
        raw
    }

    pub fn try_clone(&self) -> Result<Self, CloneError> {
        unsafe {
            let current_process_handle = process::Process::get_current().handle().as_raw();
            let mut duplicated_handle = NULL;
            if DuplicateHandle(
                current_process_handle,
                self.0,
                current_process_handle,
                &mut duplicated_handle,
                0,
                TRUE,
                DUPLICATE_SAME_ACCESS,
            ) == 0
            {
                return Err(io::Error::last_os_error().into());
            }
            Ok(Self::from_raw(duplicated_handle))
        }
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            if CloseHandle(self.0) == 0 {
                let last_os_error = io::Error::last_os_error();
                panic!("failed to drop handle {:?}: {}", self.0, last_os_error,);
            }
        }
    }
}

unsafe impl Send for Handle {}

#[derive(Debug, Error)]
#[error("failed to clone handle")]
pub struct CloneError(#[from] io::Error);
