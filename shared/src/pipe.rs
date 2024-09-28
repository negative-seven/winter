use crate::handle::{self, Handle};
use std::io::{Read, Write};
use thiserror::Error;
use winapi::{
    ctypes::c_void,
    shared::{minwindef::TRUE, ntdef::NULL},
    um::{
        fileapi::{ReadFile, WriteFile},
        minwinbase::SECURITY_ATTRIBUTES,
        namedpipeapi::{CreatePipe, PeekNamedPipe},
    },
};

pub fn new() -> Result<(Writer, Reader), NewError> {
    unsafe {
        let mut read_handle = std::ptr::null_mut();
        let mut write_handle = std::ptr::null_mut();
        let security_attributes = SECURITY_ATTRIBUTES {
            #[expect(clippy::cast_possible_truncation)]
            nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: NULL.cast(),
            bInheritHandle: TRUE,
        };
        if CreatePipe(
            &mut read_handle,
            &mut write_handle,
            std::ptr::addr_of!(security_attributes).cast_mut(),
            0,
        ) == 0
        {
            return Err(NewError(std::io::Error::last_os_error()));
        }

        Ok((
            Writer {
                handle: Handle::from_raw(write_handle),
            },
            Reader {
                handle: Handle::from_raw(read_handle),
            },
        ))
    }
}

#[derive(Debug)]
pub struct Writer {
    handle: Handle,
}

impl Writer {
    pub fn try_clone(&self) -> Result<Self, WriterCloneError> {
        Ok(Self {
            handle: self.handle.try_clone()?,
        })
    }

    #[must_use]
    pub unsafe fn new(handle: Handle) -> Self {
        Self { handle }
    }

    #[must_use]
    pub unsafe fn handle(&self) -> &Handle {
        &self.handle
    }

    #[must_use]
    pub unsafe fn leak(self) -> *mut c_void {
        unsafe { self.handle.leak() }
    }
}

impl Write for Writer {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let mut written_count = 0u32;
        unsafe {
            if WriteFile(
                self.handle.as_raw(),
                buf.as_ptr().cast(),
                buf.len()
                    .try_into()
                    .expect("cannot cast data length to u32"),
                &mut written_count,
                NULL.cast(),
            ) == 0
            {
                return Err(std::io::Error::last_os_error());
            }
        }

        Ok(written_count as usize)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[derive(Debug, Error)]
#[error("failed to clone pipe writer")]
pub enum WriterCloneError {
    HandleClone(#[from] handle::CloneError),
}

#[derive(Debug)]
pub struct Reader {
    handle: Handle,
}

impl Reader {
    #[must_use]
    pub unsafe fn new(handle: Handle) -> Self {
        Self { handle }
    }

    #[must_use]
    pub unsafe fn handle(&self) -> &Handle {
        &self.handle
    }

    #[must_use]
    pub unsafe fn leak(self) -> *mut c_void {
        unsafe { self.handle.leak() }
    }
}

impl Read for Reader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let mut pending_count = 0;
        unsafe {
            if PeekNamedPipe(
                self.handle.as_raw(),
                NULL,
                0,
                NULL.cast(),
                &mut pending_count,
                NULL.cast(),
            ) == 0
            {
                return Err(std::io::Error::last_os_error());
            }
        }
        if pending_count > 0 {
            let mut read_count = 0u32;
            unsafe {
                if ReadFile(
                    self.handle.as_raw(),
                    buf.as_mut_ptr().cast(),
                    u32::min(pending_count, buf.len().try_into().unwrap()),
                    &mut read_count,
                    NULL.cast(),
                ) == 0
                {
                    return Err(std::io::Error::last_os_error());
                }
            }

            Ok(read_count as usize)
        } else {
            Ok(0)
        }
    }
}

#[derive(Debug, Error)]
#[error("failed to create pipe")]
pub struct NewError(#[source] std::io::Error);
