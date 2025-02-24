use super::process::Process;
use std::{
    ffi::{c_void, OsString},
    io,
    mem::MaybeUninit,
    os::windows::ffi::OsStringExt,
};
use thiserror::Error;
use winapi::{shared::minwindef::HMODULE, um::psapi::GetModuleBaseNameW};

pub struct Module<'p> {
    process: &'p Process,
    handle: HMODULE,
}

impl<'p> Module<'p> {
    pub fn from_raw_handle(process: &'p Process, handle: HMODULE) -> Self {
        Self { process, handle }
    }

    pub fn get_name(&self) -> Result<OsString, GetNameError> {
        unsafe {
            let mut name = vec![MaybeUninit::<u16>::uninit(); 256];
            let mut len;
            loop {
                len = GetModuleBaseNameW(
                    self.process.raw_handle(),
                    self.handle,
                    name.as_mut_ptr().cast(),
                    name.len().try_into().unwrap(),
                );
                if len == 0 {
                    return Err(io::Error::last_os_error().into());
                }
                if len < name.len().try_into().unwrap() {
                    break;
                }
                name.resize(name.len() * 2, MaybeUninit::uninit());
            }
            Ok(OsStringExt::from_wide(
                &*(std::ptr::from_ref(&name[..len as usize]) as *const [u16]),
            ))
        }
    }

    #[must_use]
    pub fn get_base_address(&self) -> *mut c_void {
        // https://learn.microsoft.com/en-us/windows/win32/api/psapi/ns-psapi-moduleinfo
        // "The load address of a module is the same as the HMODULE value."
        self.handle.cast()
    }
}

#[derive(Debug, Error)]
#[error("failed to get name of module")]
pub struct GetNameError(#[from] io::Error);
