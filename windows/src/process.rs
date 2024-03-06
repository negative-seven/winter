use crate::{pipe, thread::Thread};
use std::{
    ffi::{CStr, CString, NulError},
    io,
    mem::{size_of, transmute},
};
use thiserror::Error;
use tracing::{debug, instrument, Level};
use winapi::{
    ctypes::c_void,
    shared::{minwindef::TRUE, ntdef::NULL},
    um::{
        handleapi::{CloseHandle, INVALID_HANDLE_VALUE},
        memoryapi::{ReadProcessMemory, VirtualAllocEx, WriteProcessMemory},
        processthreadsapi::{
            CreateProcessA, CreateRemoteThread, GetCurrentProcess, GetProcessId,
            PROCESS_INFORMATION, STARTUPINFOA,
        },
        tlhelp32::{
            CreateToolhelp32Snapshot, Module32First, Module32Next, Thread32First, Thread32Next,
            MODULEENTRY32, TH32CS_SNAPMODULE, TH32CS_SNAPMODULE32, TH32CS_SNAPTHREAD,
            THREADENTRY32,
        },
        winbase::{CREATE_SUSPENDED, STARTF_USESTDHANDLES},
        winnt::{
            IMAGE_DIRECTORY_ENTRY_EXPORT, IMAGE_DOS_HEADER, IMAGE_EXPORT_DIRECTORY,
            IMAGE_FILE_HEADER, IMAGE_OPTIONAL_HEADER32, IMAGE_OPTIONAL_HEADER64, MEM_COMMIT,
            PAGE_EXECUTE_READ, PAGE_READWRITE,
        },
    },
};

#[derive(Debug)]
pub struct Process {
    handle: *mut c_void,
    is_pseudo_handle: bool,
}

impl Process {
    #[must_use]
    pub fn get_current() -> Self {
        Self {
            handle: unsafe { GetCurrentProcess() },
            is_pseudo_handle: true,
        }
    }

    #[instrument(ret, err)]
    pub fn create(
        executable_path: &str,
        suspended: bool,
        stdin_redirect: Option<&pipe::Reader>,
        stdout_redirect: Option<&pipe::Writer>,
        stderr_redirect: Option<&pipe::Writer>,
    ) -> Result<Self, CreateError> {
        let executable_path_c_string = CString::new(executable_path)?;

        #[allow(clippy::cast_possible_truncation)]
        let mut startup_info = STARTUPINFOA {
            cb: size_of::<STARTUPINFOA>() as u32,
            lpReserved: NULL.cast(),
            lpDesktop: NULL.cast(),
            lpTitle: NULL.cast(),
            dwX: 0,
            dwY: 0,
            dwXSize: 0,
            dwYSize: 0,
            dwXCountChars: 0,
            dwYCountChars: 0,
            dwFillAttribute: 0,
            dwFlags: STARTF_USESTDHANDLES,
            wShowWindow: 0,
            cbReserved2: 0,
            lpReserved2: NULL.cast(),
            hStdInput: stdin_redirect.map_or_else(|| NULL.cast(), |reader| reader.handle),
            hStdOutput: stdout_redirect.map_or_else(|| NULL.cast(), |writer| writer.handle),
            hStdError: stderr_redirect.map_or_else(|| NULL.cast(), |writer| writer.handle),
        };
        let mut process_information = PROCESS_INFORMATION {
            hProcess: NULL.cast(),
            hThread: NULL.cast(),
            dwProcessId: 0,
            dwThreadId: 0,
        };

        unsafe {
            if CreateProcessA(
                executable_path_c_string.as_ptr().cast(),
                NULL.cast(),
                NULL.cast(),
                NULL.cast(),
                TRUE,
                if suspended { CREATE_SUSPENDED } else { 0 },
                NULL.cast(),
                NULL.cast(),
                &mut startup_info,
                &mut process_information,
            ) == 0
            {
                return Err(io::Error::last_os_error().into());
            }

            CloseHandle(process_information.hThread);
        }

        let process = Process {
            handle: process_information.hProcess,
            is_pseudo_handle: false,
        };

        Ok(process)
    }

    pub fn get_id(&self) -> Result<u32, GetIdError> {
        let process_id = unsafe { GetProcessId(self.handle) };
        if process_id == 0 {
            Err(io::Error::last_os_error())?
        } else {
            Ok(process_id)
        }
    }

    pub fn iter_thread_ids(&self) -> Result<ThreadIdIterator, IterThreadIdsError> {
        Ok(ThreadIdIterator::new(self.get_id()?)?)
    }

    #[instrument(ret(level = Level::DEBUG), err)]
    pub fn allocate_read_write_memory(&self, size: usize) -> Result<usize, io::Error> {
        let pointer =
            unsafe { VirtualAllocEx(self.handle, NULL, size, MEM_COMMIT, PAGE_READWRITE) } as usize;
        if pointer == 0 {
            return Err(io::Error::last_os_error());
        }

        Ok(pointer)
    }

    #[instrument(ret(level = Level::DEBUG), err)]
    pub fn allocate_read_execute_memory(&self, size: usize) -> Result<usize, io::Error> {
        let pointer =
            unsafe { VirtualAllocEx(self.handle, NULL, size, MEM_COMMIT, PAGE_EXECUTE_READ) }
                as usize;
        if pointer == 0 {
            return Err(io::Error::last_os_error());
        }

        Ok(pointer)
    }

    #[instrument(
        err,
        skip(address),
        fields(address = %format!("0x{:x}", address))
    )]
    pub unsafe fn read<T: Copy>(&self, address: usize) -> Result<T, io::Error> {
        use std::alloc::{alloc, dealloc, Layout};

        let data = alloc(Layout::array::<T>(1).unwrap());
        if ReadProcessMemory(
            self.handle,
            address as *mut c_void,
            data.cast(),
            std::mem::size_of::<T>(),
            NULL.cast(),
        ) == 0
        {
            dealloc(data, Layout::array::<T>(1).unwrap());
            return Err(io::Error::last_os_error());
        }
        let result = *data.cast();
        dealloc(data, Layout::array::<T>(1).unwrap());
        Ok(result)
    }

    #[instrument(
        err,
        skip(address),
        fields(address = %format!("0x{:x}", address))
    )]
    #[allow(dead_code)]
    pub fn read_to_vec(&self, address: usize, size: usize) -> Result<Vec<u8>, io::Error> {
        let mut data = vec![0; size];
        unsafe {
            if ReadProcessMemory(
                self.handle,
                address as *mut c_void,
                data.as_mut_ptr().cast(),
                size,
                NULL.cast(),
            ) == 0
            {
                return Err(io::Error::last_os_error());
            }
        }
        Ok(data)
    }

    pub fn read_u8(&self, address: usize) -> Result<u8, io::Error> {
        Ok(self.read_to_vec(address, 1)?[0])
    }

    #[allow(clippy::missing_panics_doc)]
    pub fn read_u16(&self, address: usize) -> Result<u16, io::Error> {
        Ok(u16::from_le_bytes(
            <[u8; 2]>::try_from(self.read_to_vec(address, 2)?).unwrap(),
        ))
    }

    #[allow(clippy::missing_panics_doc)]
    pub fn read_u32(&self, address: usize) -> Result<u32, io::Error> {
        Ok(u32::from_le_bytes(
            <[u8; 4]>::try_from(self.read_to_vec(address, 4)?).unwrap(),
        ))
    }

    pub fn read_nul_terminated_string(&self, address: usize) -> Result<String, io::Error> {
        let mut string = String::new();
        for index in 0.. {
            let next_byte = self.read_u8(address + index)?;
            if next_byte == 0 {
                break;
            }
            string.push(next_byte as char);
        }
        Ok(string)
    }

    #[instrument(
        ret(level = Level::DEBUG),
        err,
        skip(address, data),
        fields(address = %format!("0x{:x}", address), data_len = data.len())
    )]
    #[allow(dead_code)]
    pub fn write(&self, address: usize, data: &[u8]) -> Result<(), io::Error> {
        unsafe {
            if WriteProcessMemory(
                self.handle,
                address as *mut c_void,
                data.as_ptr().cast(),
                data.len(),
                NULL.cast(),
            ) == 0
            {
                return Err(io::Error::last_os_error());
            }
        }

        Ok(())
    }

    #[instrument(
        ret,
        err,
        skip(start_address),
        fields(address = %format!("0x{:x}", start_address))
    )]
    pub fn create_thread(
        &self,
        start_address: usize,
        suspended: bool,
        parameter: Option<*mut c_void>,
    ) -> Result<Thread, io::Error> {
        let thread_handle = unsafe {
            CreateRemoteThread(
                self.handle,
                NULL.cast(),
                0,
                Some(transmute(start_address)),
                parameter.unwrap_or(NULL),
                if suspended { CREATE_SUSPENDED } else { 0 },
                NULL.cast(),
            )
        };

        if thread_handle == NULL {
            return Err(io::Error::last_os_error());
        }

        Ok(unsafe { Thread::from_handle(thread_handle) })
    }

    #[instrument(
        ret(level = Level::DEBUG),
        err,
        skip(name),
        fields(name = name.as_ref().to_string())
    )]
    pub fn get_module_address(
        &self,
        name: impl AsRef<str>,
    ) -> Result<usize, GetModuleAddressError> {
        let target_module_name = name.as_ref();
        unsafe {
            for entry in ModuleEntry32Iterator::new(self.get_id()?)? {
                let module_name = CStr::from_ptr(entry.szModule.as_ptr()).to_str();
                if let Ok(module_name) = module_name {
                    // if the module name is not valid utf-8, it will not match
                    if module_name.to_lowercase() == target_module_name.to_lowercase() {
                        return Ok(entry.modBaseAddr as usize);
                    }
                };
            }
        }
        Err(ModuleNotFoundError.into())
    }

    #[instrument(
        ret(level = Level::DEBUG),
        err,
        skip(module_name, export_name),
        fields(
            module_name = module_name.as_ref().to_string(),
            export_name = export_name.as_ref().to_string(),
        )
    )]
    pub fn get_export_address(
        &self,
        module_name: impl AsRef<str>,
        export_name: impl AsRef<str>,
    ) -> Result<usize, GetExportAddressError> {
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
                if self.data_directory_entry_count() < IMAGE_DIRECTORY_ENTRY_EXPORT.into() {
                    None
                } else {
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
                }
            }
        }

        let module_name = module_name.as_ref();
        let export_name = export_name.as_ref();

        debug!("get module address in target process");
        let module_address = self.get_module_address(module_name)?;

        debug!("read dos header from 0x{module_address:x} and verify magic");
        let dos_header = unsafe { self.read::<IMAGE_DOS_HEADER>(module_address) }?;
        if dos_header.e_magic != 0x5a4d {
            return Err(InvalidModuleHeadersError.into());
        }

        #[allow(clippy::cast_sign_loss)]
        let pe_header_address = module_address + dos_header.e_lfanew as usize;
        debug!("verify signature of pe header at 0x{pe_header_address:x}");
        if self.read_to_vec(pe_header_address, 4)? != [0x50, 0x45, 0x0, 0x0] {
            return Err(InvalidModuleHeadersError.into());
        }

        #[allow(clippy::cast_sign_loss)]
        let optional_header_address =
            pe_header_address + 4 + std::mem::size_of::<IMAGE_FILE_HEADER>();
        debug!("read optional header from 0x{optional_header_address:x} and verify magic",);
        let optional_header_magic = self.read_to_vec(optional_header_address, 2)?;
        let optional_header = match (optional_header_magic[0], optional_header_magic[1]) {
            (0xb, 0x1) => OptionalHeader::Header32(unsafe {
                self.read::<IMAGE_OPTIONAL_HEADER32>(optional_header_address)
            }?),
            (0xb, 0x2) => OptionalHeader::Header64(unsafe {
                self.read::<IMAGE_OPTIONAL_HEADER64>(optional_header_address)
            }?),
            _ => return Err(InvalidModuleHeadersError.into()),
        };

        debug!("get export directory table");
        let export_directory_table_address = module_address
            + optional_header
                .export_table_address()
                .ok_or(InvalidModuleHeadersError)? as usize;
        let export_directory_table =
            unsafe { self.read::<IMAGE_EXPORT_DIRECTORY>(export_directory_table_address) }?;

        debug!("attempt to find export with matching name");
        for index in 0..export_directory_table.NumberOfNames as usize {
            let export_name_pointer = module_address
                + self.read_u32(
                    module_address + export_directory_table.AddressOfNames as usize + index * 4,
                )? as usize;
            let export_name_at_index = self.read_nul_terminated_string(export_name_pointer)?;
            if export_name_at_index.to_lowercase() == export_name.to_lowercase() {
                let export_ordinal = self.read_u16(
                    module_address
                        + export_directory_table.AddressOfNameOrdinals as usize
                        + index * 2,
                )? as usize;
                let export_offset = self.read_u32(
                    module_address
                        + export_directory_table.AddressOfFunctions as usize
                        + export_ordinal * 4,
                )? as usize;
                return Ok(module_address + export_offset);
            }
        }
        return Err(ExportNotFoundError.into());
    }

    #[instrument]
    pub fn inject_dll(&self, library_path: &str) -> Result<(), InjectDllError> {
        let library_path_c_string =
            CString::new(library_path).map_err(LibraryPathContainsNulError)?;

        debug!("write no-op function");
        let no_op_function_pointer = self.allocate_read_execute_memory(1)?;
        self.write(no_op_function_pointer, &[0xc3])?; // opcode c3 is ret in both x86 and x64

        debug!("run dummy thread to provoke loading of kernel32.dll");
        self.create_thread(no_op_function_pointer, false, None)?
            .join()?;

        debug!("write injected dll path");
        let injected_dll_path_pointer =
            self.allocate_read_write_memory(library_path_c_string.to_bytes_with_nul().len())?;
        self.write(
            injected_dll_path_pointer,
            library_path_c_string.as_bytes_with_nul(),
        )?;

        debug!("write dll loading function");
        let load_library_a_pointer = self.get_export_address("kernel32.dll", "LoadLibraryA")?;
        let get_last_error_pointer = self.get_export_address("kernel32.dll", "GetLastError")?;
        let mut load_dll_function = [
            0x68, 0xcc, 0xcc, 0xcc, 0xcc, // push injected_dll_path_pointer
            0xb8, 0xcc, 0xcc, 0xcc, 0xcc, // mov eax, load_library_a_pointer
            0xff, 0xd0, // call eax
            0x85, 0xc0, // test eax, eax
            0xb8, 0x00, 0x00, 0x00, 0x00, // mov eax, 0 (preserves ZF)
            0x75, 0x07, // jne return
            0xb8, 0xcc, 0xcc, 0xcc, 0xcc, // mov eax, get_last_error_pointer
            0xff, 0xd0, // call eax
            // return:
            0xc3, // ret
        ];
        load_dll_function[1..5].copy_from_slice(&injected_dll_path_pointer.to_le_bytes()[..4]);
        load_dll_function[6..10].copy_from_slice(&load_library_a_pointer.to_le_bytes()[..4]);
        load_dll_function[22..26].copy_from_slice(&get_last_error_pointer.to_le_bytes()[..4]);
        let load_dll_function_pointer = self.allocate_read_execute_memory(256)?;
        self.write(load_dll_function_pointer, &load_dll_function)?;

        debug!("run dll loading thread");
        match self
            .create_thread(load_dll_function_pointer, false, None)?
            .join()?
        {
            0 => Ok(()),
            error_code => return Err(LoadLibraryThreadError { error_code }.into()),
        }
    }
}

impl Drop for Process {
    fn drop(&mut self) {
        if !self.is_pseudo_handle {
            unsafe { CloseHandle(self.handle) };
        }
    }
}

pub struct ThreadIdIterator {
    process_id: u32,
    snapshot_handle: *mut c_void,
    called_thread_32_first: bool,
}

impl ThreadIdIterator {
    pub(in crate::process) fn new(process_id: u32) -> Result<Self, NewThreadIdIteratorError> {
        let snapshot_handle = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) };
        if snapshot_handle == INVALID_HANDLE_VALUE {
            return Err(io::Error::last_os_error().into());
        }

        Ok(ThreadIdIterator {
            process_id,
            snapshot_handle,
            called_thread_32_first: false,
        })
    }
}

impl Iterator for ThreadIdIterator {
    type Item = u32;

    fn next(&mut self) -> Option<Self::Item> {
        // https://devblogs.microsoft.com/oldnewthing/20060223-14/?p=32173

        let mut entry = THREADENTRY32 {
            #[allow(clippy::cast_possible_truncation)]
            dwSize: size_of::<THREADENTRY32>() as u32,
            cntUsage: 0,
            th32ThreadID: 0,
            th32OwnerProcessID: 0,
            tpBasePri: 0,
            tpDeltaPri: 0,
            dwFlags: 0,
        };

        loop {
            let next_thread_exists = unsafe {
                if self.called_thread_32_first {
                    Thread32Next(self.snapshot_handle, &mut entry)
                } else {
                    self.called_thread_32_first = true;
                    Thread32First(self.snapshot_handle, &mut entry)
                }
            } != 0;

            if !next_thread_exists {
                return None;
            }

            if entry.dwSize >= 16 && entry.th32OwnerProcessID == self.process_id {
                // if self.te.dwSize >= FIELD_OFFSET(THREADENTRY32, th32OwnerProcessID) +
                // sizeof(te.th32OwnerProcessID))
                return Some(entry.th32ThreadID);
            }

            // continue loop
        }
    }
}

impl Drop for ThreadIdIterator {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.snapshot_handle) };
    }
}

#[derive(Debug)]
struct ModuleEntry32Iterator {
    snapshot_handle: *mut c_void,
    called_module_32_first: bool,
}

impl ModuleEntry32Iterator {
    #[instrument(ret(level = Level::DEBUG), err)]
    pub(in crate::process) fn new(process_id: u32) -> Result<Self, NewModuleEntry32IteratorError> {
        let snapshot_handle = unsafe {
            CreateToolhelp32Snapshot(TH32CS_SNAPMODULE | TH32CS_SNAPMODULE32, process_id)
        };

        if snapshot_handle == INVALID_HANDLE_VALUE {
            return Err(io::Error::last_os_error().into());
        }

        Ok(ModuleEntry32Iterator {
            snapshot_handle,
            called_module_32_first: false,
        })
    }
}

impl Iterator for ModuleEntry32Iterator {
    type Item = MODULEENTRY32;

    fn next(&mut self) -> Option<Self::Item> {
        let mut me32 = MODULEENTRY32 {
            #[allow(clippy::cast_possible_truncation)]
            dwSize: size_of::<MODULEENTRY32>() as u32,
            th32ModuleID: 0,
            th32ProcessID: 0,
            GlblcntUsage: 0,
            ProccntUsage: 0,
            modBaseAddr: NULL.cast(),
            modBaseSize: 0,
            hModule: NULL.cast(),
            szModule: [0; 256],
            szExePath: [0; 260],
        };

        let next_thread_exists = unsafe {
            if self.called_module_32_first {
                Module32Next(self.snapshot_handle, &mut me32)
            } else {
                self.called_module_32_first = true;
                Module32First(self.snapshot_handle, &mut me32)
            }
        } != 0;

        if next_thread_exists {
            Some(me32)
        } else {
            None
        }
    }
}

impl Drop for ModuleEntry32Iterator {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.snapshot_handle) };
    }
}

#[derive(Debug, Error)]
#[error("failed to create process")]
pub enum CreateError {
    PathContainsNul(#[from] NulError),
    Os(#[from] io::Error),
}

#[derive(Debug, Error)]
#[error("failed to get process id")]
pub struct GetIdError(#[from] io::Error);

#[derive(Debug, Error)]
#[error("failed to create thread id iterator")]
pub enum IterThreadIdsError {
    GetId(#[from] GetIdError),
    NewThreadIdIterator(#[from] NewThreadIdIteratorError),
}

#[derive(Debug, Error)]
#[error("failed to get module address")]
pub enum GetModuleAddressError {
    GetId(#[from] GetIdError),
    NewModuleEntry32Iterator(#[from] NewModuleEntry32IteratorError),
    ModuleNotFound(#[from] ModuleNotFoundError),
    Os(#[from] io::Error),
}

#[derive(Debug, Error)]
#[error("module not found")]
pub struct ModuleNotFoundError;

#[derive(Debug, Error)]
#[error("failed to get export address")]
pub enum GetExportAddressError {
    GetModuleAddress(#[from] GetModuleAddressError),
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

#[derive(Debug, Error)]
#[error("failed to inject dll")]
pub enum InjectDllError {
    LibraryPathContainsNul(#[from] LibraryPathContainsNulError),
    GetExportAddress(#[from] GetExportAddressError),
    JoinThread(#[from] crate::thread::JoinError),
    LoadLibraryThread(#[from] LoadLibraryThreadError),
    Os(#[from] io::Error),
}

#[derive(Debug, Error)]
#[error("library loading thread returned with error code 0x{error_code:x}")]
pub struct LoadLibraryThreadError {
    error_code: u32,
}

#[derive(Debug, Error)]
#[error("library path contains nul")]
pub struct LibraryPathContainsNulError(#[source] NulError);

#[derive(Debug, Error)]
#[error("failed to create thread id iterator")]
pub struct NewThreadIdIteratorError(#[from] io::Error);

#[derive(Debug, Error)]
#[error("failed to create MODULEENTRY32 iterator")]
pub struct NewModuleEntry32IteratorError(#[from] io::Error);
