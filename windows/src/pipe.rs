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

pub fn create() -> Result<(Writer, Reader), CreateError> {
    let mut read_handle = std::ptr::null_mut();
    let mut write_handle = std::ptr::null_mut();
    let security_attributes = SECURITY_ATTRIBUTES {
        #[allow(clippy::cast_possible_truncation)]
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: NULL.cast(),
        bInheritHandle: TRUE,
    };
    unsafe {
        if CreatePipe(
            &mut read_handle,
            &mut write_handle,
            std::ptr::addr_of!(security_attributes).cast_mut(),
            0,
        ) == 0
        {
            return Err(CreateError(std::io::Error::last_os_error()));
        }
    }

    Ok((
        Writer {
            handle: write_handle,
        },
        Reader {
            handle: read_handle,
        },
    ))
}

#[derive(Debug)]
pub struct Writer {
    pub(crate) handle: *mut c_void,
}

impl Writer {
    /// # Panics
    /// Panics if `data.len()` exceeds `u32::MAX`.
    pub fn write(&self, data: &[u8]) -> Result<(), WriteError> {
        let mut written_bytes = 0u32;
        unsafe {
            if WriteFile(
                self.handle,
                data.as_ptr().cast(),
                data.len()
                    .try_into()
                    .expect("cannot cast data length to u32"),
                &mut written_bytes,
                NULL.cast(),
            ) == 0
            {
                return Err(WriteError(std::io::Error::last_os_error()));
            }
        }

        Ok(())
    }
}

#[derive(Debug)]
pub struct Reader {
    pub(crate) handle: *mut c_void,
}

impl Reader {
    pub fn read(&self, count: u32) -> Result<Vec<u8>, ReadError> {
        let mut buffer = vec![0u8; count as usize];
        let mut read_bytes = 0u32;
        unsafe {
            if ReadFile(
                self.handle,
                buffer.as_mut_ptr().cast(),
                count,
                &mut read_bytes,
                NULL.cast(),
            ) == 0
            {
                return Err(ReadError(std::io::Error::last_os_error()));
            }
        }

        Ok(buffer)
    }

    pub fn read_pending(&self) -> Result<Vec<u8>, ReadError> {
        let mut count = 0;
        unsafe {
            if PeekNamedPipe(self.handle, NULL, 0, NULL.cast(), &mut count, NULL.cast()) == 0 {
                return Err(ReadError(std::io::Error::last_os_error()));
            }
        }
        if count > 0 {
            self.read(count)
        } else {
            Ok(Vec::new())
        }
    }
}

#[derive(Debug, Error)]
#[error("failed to create pipe")]
pub struct CreateError(#[source] std::io::Error);

#[derive(Debug, Error)]
#[error("failed to write to pipe")]
pub struct WriteError(#[source] std::io::Error);

#[derive(Debug, Error)]
#[error("failed to read from pipe")]
pub struct ReadError(#[source] std::io::Error);
