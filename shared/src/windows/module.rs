use super::process::{self, Process};
use std::{
    ffi::{c_void, OsString},
    io,
    mem::MaybeUninit,
    os::windows::ffi::OsStringExt,
};
use thiserror::Error;
use winapi::{
    shared::minwindef::HMODULE,
    um::{
        psapi::GetModuleBaseNameW,
        winnt::{
            IMAGE_DIRECTORY_ENTRY_EXPORT, IMAGE_DOS_HEADER, IMAGE_EXPORT_DIRECTORY,
            IMAGE_FILE_HEADER, IMAGE_OPTIONAL_HEADER32, IMAGE_OPTIONAL_HEADER64,
        },
    },
};

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

    #[expect(clippy::too_many_lines)] // TODO
    pub fn get_export_address(
        &self,
        export_name: &str,
    ) -> Result<*mut c_void, GetExportAddressError> {
        enum OptionalHeader {
            Header32(IMAGE_OPTIONAL_HEADER32),
            Header64(IMAGE_OPTIONAL_HEADER64),
        }
        impl OptionalHeader {
            fn data_directory_entry_count(&self) -> u32 {
                match self {
                    Self::Header32(header) => header.NumberOfRvaAndSizes,
                    Self::Header64(header) => header.NumberOfRvaAndSizes,
                }
            }

            fn export_table_address(&self) -> Option<u32> {
                if u32::from(IMAGE_DIRECTORY_ENTRY_EXPORT) < self.data_directory_entry_count() {
                    Some(
                        match self {
                            Self::Header32(header) => {
                                header.DataDirectory[IMAGE_DIRECTORY_ENTRY_EXPORT as usize]
                            }
                            Self::Header64(header) => {
                                header.DataDirectory[IMAGE_DIRECTORY_ENTRY_EXPORT as usize]
                            }
                        }
                        .VirtualAddress,
                    )
                } else {
                    None
                }
            }
        }

        unsafe {
            let dos_header_address = self.get_base_address().cast::<IMAGE_DOS_HEADER>();
            let dos_header = self.process.read(dos_header_address)?;
            if dos_header.e_magic != 0x5a4d {
                return Err(InvalidModuleHeadersError.into());
            }

            #[expect(clippy::cast_sign_loss)]
            let pe_header_address = self
                .get_base_address()
                .byte_add(dos_header.e_lfanew as usize);
            if self.process.read_to_vec(pe_header_address.cast(), 4)? != [0x50, 0x45, 0x0, 0x0] {
                return Err(InvalidModuleHeadersError.into());
            }

            let optional_header_address =
                pe_header_address.byte_add(4 + size_of::<IMAGE_FILE_HEADER>());
            let optional_header_magic = self
                .process
                .read_to_vec(optional_header_address.cast(), 2)?;
            let optional_header = match (optional_header_magic[0], optional_header_magic[1]) {
                (0xb, 0x1) => OptionalHeader::Header32(
                    self.process
                        .read::<IMAGE_OPTIONAL_HEADER32>(optional_header_address.cast())?,
                ),
                (0xb, 0x2) => OptionalHeader::Header64(
                    self.process
                        .read::<IMAGE_OPTIONAL_HEADER64>(optional_header_address.cast())?,
                ),
                _ => return Err(InvalidModuleHeadersError.into()),
            };

            let export_directory_table_address = self
                .get_base_address()
                .byte_add(
                    optional_header
                        .export_table_address()
                        .ok_or(InvalidModuleHeadersError)? as usize,
                )
                .cast::<IMAGE_EXPORT_DIRECTORY>();
            let export_directory_table = self.process.read(export_directory_table_address)?;

            for index in 0..export_directory_table.NumberOfNames as usize {
                let export_name_pointer = self
                    .get_base_address()
                    .byte_add(
                        self.process.read_u32(
                            self.get_base_address()
                                .byte_add(
                                    export_directory_table.AddressOfNames as usize + index * 4,
                                )
                                .cast(),
                        )? as usize,
                    )
                    .cast();
                let export_name_at_index = self
                    .process
                    .read_nul_terminated_string(export_name_pointer)?;
                if export_name_at_index.to_lowercase() == export_name.to_lowercase() {
                    let export_ordinal = self.process.read_u16(
                        self.get_base_address()
                            .byte_add(
                                export_directory_table.AddressOfNameOrdinals as usize + index * 2,
                            )
                            .cast(),
                    )? as usize;
                    let export_offset = self.process.read_u32(
                        self.get_base_address()
                            .byte_add(
                                export_directory_table.AddressOfFunctions as usize
                                    + export_ordinal * 4,
                            )
                            .cast(),
                    )? as usize;
                    return Ok((self.get_base_address().byte_add(export_offset)).cast());
                }
            }
            Err(ExportNotFoundError.into())
        }
    }
}

#[derive(Debug, Error)]
#[error("failed to get name of module")]
pub struct GetNameError(#[from] io::Error);

#[derive(Debug, Error)]
#[error("failed to get export address")]
pub enum GetExportAddressError {
    ReadMemory(#[from] process::ReadMemoryError),
    InvalidModuleHeaders(#[from] InvalidModuleHeadersError),
    ExportNotFound(#[from] ExportNotFoundError),
    Os(#[from] io::Error),
}

#[derive(Debug, Error)]
#[error("invalid headers in module")]
pub struct InvalidModuleHeadersError;

#[derive(Debug, Error)]
#[error("export not found in module")]
pub struct ExportNotFoundError;
