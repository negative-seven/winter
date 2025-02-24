use super::module::{self, Module};
use crate::windows::{
    handle::{self, handle_wrapper, Handle},
    pipe,
    thread::Thread,
};
use std::{
    ffi::{c_void, CString, NulError, OsStr},
    io,
    mem::MaybeUninit,
    os::windows::ffi::OsStrExt,
    path::Path,
};
use thiserror::Error;
use winapi::{
    shared::{
        minwindef::{FALSE, HMODULE, TRUE},
        ntdef::NULL,
    },
    um::{
        handleapi::INVALID_HANDLE_VALUE,
        jobapi2::{AssignProcessToJobObject, SetInformationJobObject},
        memoryapi::{
            ReadProcessMemory, VirtualAllocEx, VirtualFreeEx, VirtualProtectEx, VirtualQueryEx,
            WriteProcessMemory,
        },
        processthreadsapi::{
            CreateProcessW, CreateRemoteThread, GetCurrentProcess, GetExitCodeProcess,
            GetProcessId, OpenProcess, PROCESS_INFORMATION, STARTUPINFOW,
        },
        psapi::{EnumProcessModulesEx, LIST_MODULES_ALL},
        tlhelp32::{
            CreateToolhelp32Snapshot, Thread32First, Thread32Next, TH32CS_SNAPTHREAD, THREADENTRY32,
        },
        winbase::{CreateJobObjectA, CREATE_SUSPENDED, STARTF_USESTDHANDLES},
        winnt::{
            JobObjectExtendedLimitInformation, IMAGE_FILE_MACHINE_AMD64, IMAGE_FILE_MACHINE_I386,
            IMAGE_FILE_MACHINE_IA64, IMAGE_FILE_MACHINE_UNKNOWN,
            JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE, MEM_COMMIT,
            MEM_FREE, MEM_RELEASE, MEM_RESERVE, PROCESS_ALL_ACCESS,
        },
        wow64apiset::IsWow64Process2,
    },
};

handle_wrapper!(Process);

impl Process {
    #[must_use]
    pub fn get_current() -> Self {
        unsafe { Self::from_raw_handle(GetCurrentProcess()) }
    }

    pub fn from_id(id: u32) -> Result<Self, io::Error> {
        let handle = unsafe { OpenProcess(PROCESS_ALL_ACCESS, FALSE, id) };
        if handle.is_null() {
            return Err(io::Error::last_os_error());
        }
        unsafe { Ok(Self::from_raw_handle(handle)) }
    }

    pub fn create(
        executable_path: impl AsRef<Path>,
        command_line_string: impl AsRef<OsStr>,
        suspended: bool,
        stdin_redirect: Option<pipe::Reader>,
        stdout_redirect: Option<pipe::Writer>,
        stderr_redirect: Option<pipe::Writer>,
    ) -> Result<Self, CreateError> {
        let executable_path_raw = executable_path
            .as_ref()
            .as_os_str()
            .encode_wide()
            .chain([0])
            .collect::<Vec<_>>();
        let executable_directory_path_raw = executable_path
            .as_ref()
            .parent()
            .unwrap()
            .as_os_str()
            .encode_wide()
            .chain([0])
            .collect::<Vec<_>>();
        let mut command_line_string_raw = command_line_string
            .as_ref()
            .encode_wide()
            .chain([0])
            .collect::<Vec<_>>();

        let mut startup_info = STARTUPINFOW {
            #[expect(clippy::cast_possible_truncation)]
            cb: size_of::<STARTUPINFOW>() as u32,
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
            hStdInput: stdin_redirect
                .map_or_else(|| NULL.cast(), |reader| unsafe { reader.leak_handle() }),
            hStdOutput: stdout_redirect
                .map_or_else(|| NULL.cast(), |writer| unsafe { writer.leak_handle() }),
            hStdError: stderr_redirect
                .map_or_else(|| NULL.cast(), |writer| unsafe { writer.leak_handle() }),
        };
        let mut process_information = PROCESS_INFORMATION {
            hProcess: NULL.cast(),
            hThread: NULL.cast(),
            dwProcessId: 0,
            dwThreadId: 0,
        };

        unsafe {
            if CreateProcessW(
                executable_path_raw.as_ptr(),
                command_line_string_raw.as_mut_ptr(),
                NULL.cast(),
                NULL.cast(),
                TRUE,
                if suspended { CREATE_SUSPENDED } else { 0 },
                NULL.cast(),
                executable_directory_path_raw.as_ptr(),
                &mut startup_info,
                &mut process_information,
            ) == 0
            {
                return Err(io::Error::last_os_error().into());
            }

            // ensure these variables are dropped after the call to CreateProcessW
            drop(executable_path_raw);
            drop(executable_directory_path_raw);

            // ensure the handle gets cleaned up correctly
            Thread::from_raw_handle(process_information.hThread);

            Ok(Process::from_raw_handle(process_information.hProcess))
        }
    }

    pub fn is_64_bit(&self) -> Result<bool, CheckIs64BitError> {
        let mut process_wow64_machine = 0;
        let mut system_machine = 0;
        unsafe {
            IsWow64Process2(
                self.handle.as_raw(),
                &mut process_wow64_machine,
                &mut system_machine,
            );
        }

        let machine = if process_wow64_machine == IMAGE_FILE_MACHINE_UNKNOWN {
            system_machine
        } else {
            process_wow64_machine
        };

        Ok(match machine {
            IMAGE_FILE_MACHINE_I386 => false,
            IMAGE_FILE_MACHINE_AMD64 | IMAGE_FILE_MACHINE_IA64 => true,
            _ => return Err(UnknownMachineError(machine).into()),
        })
    }

    pub fn kill_on_current_process_exit(&self) -> Result<(), KillOnCurrentProcessExitError> {
        unsafe {
            let job = CreateJobObjectA(NULL.cast(), NULL.cast());
            if job == NULL {
                return Err(io::Error::last_os_error().into());
            }
            let job = Handle::from_raw(job);

            let information = {
                let mut information: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
                information.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
                information
            };

            #[expect(clippy::cast_possible_truncation)]
            if SetInformationJobObject(
                job.as_raw(),
                JobObjectExtendedLimitInformation,
                std::ptr::addr_of!(information).cast_mut().cast(),
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            ) == 0
            {
                return Err(io::Error::last_os_error().into());
            }

            if AssignProcessToJobObject(job.as_raw(), self.handle.as_raw()) == 0 {
                return Err(io::Error::last_os_error().into());
            }

            // purposefully leak handle so that it gets closed on process exit
            let _ = job.leak();
        }

        Ok(())
    }

    pub async fn join(&self) -> Result<u32, JoinError> {
        self.handle.wait().await?;
        let mut exit_code = 0;
        unsafe {
            GetExitCodeProcess(self.handle.as_raw(), &mut exit_code);
        }
        Ok(exit_code)
    }

    pub fn get_id(&self) -> Result<u32, GetIdError> {
        let process_id = unsafe { GetProcessId(self.handle.as_raw()) };
        if process_id == 0 {
            Err(io::Error::last_os_error())?
        } else {
            Ok(process_id)
        }
    }

    /// Returns an iterator over the IDs of all the threads that belong to the
    /// process.
    ///
    /// Note that this method can be quite slow, as it internally fetches all
    /// threads from all processes.
    pub fn iter_thread_ids(&self) -> Result<ThreadIdIterator, IterThreadIdsError> {
        Ok(ThreadIdIterator::new(self.get_id()?)?)
    }

    pub fn get_modules(&self) -> Result<Vec<Module>, GetModulesError> {
        unsafe {
            let mut modules = Vec::<MaybeUninit<HMODULE>>::new();
            let mut items_needed = 0;
            loop {
                if EnumProcessModulesEx(
                    self.raw_handle(),
                    modules.as_mut_ptr().cast(),
                    (modules.len() * size_of::<HMODULE>()).try_into().unwrap(),
                    &mut items_needed,
                    LIST_MODULES_ALL,
                ) == 0
                {
                    return Err(io::Error::last_os_error().into());
                }
                items_needed /= u32::try_from(size_of::<HMODULE>()).unwrap();

                if modules.len() >= items_needed as usize {
                    break;
                }

                modules.resize(items_needed as usize, MaybeUninit::uninit());
            }

            Ok(modules
                .iter()
                .take(items_needed as usize)
                .map(|m| Module::from_raw_handle(self, m.assume_init()))
                .collect())
        }
    }

    pub fn get_module(&self, name: &OsStr) -> Result<Option<Module>, GetModulesError> {
        Ok(self
            .get_modules()?
            .into_iter()
            .find(|m| m.get_name().is_ok_and(|n| n.eq_ignore_ascii_case(name))))
    }

    pub fn allocate_memory(
        &self,
        size: usize,
        permissions: MemoryPermissions,
    ) -> Result<*mut c_void, AllocateMemoryError> {
        let pointer = unsafe {
            VirtualAllocEx(
                self.handle.as_raw(),
                NULL,
                size,
                MEM_COMMIT | MEM_RESERVE,
                permissions.to_winapi_constant(),
            )
        };
        if pointer.is_null() {
            return Err(io::Error::last_os_error().into());
        }

        Ok(pointer.cast())
    }

    pub fn allocate_memory_at(
        &self,
        address: *mut c_void,
        size: usize,
        permissions: MemoryPermissions,
    ) -> Result<*mut c_void, AllocateMemoryError> {
        let pointer = unsafe {
            VirtualAllocEx(
                self.handle.as_raw(),
                address.cast(),
                size,
                MEM_COMMIT | MEM_RESERVE,
                permissions.to_winapi_constant(),
            )
        };
        if pointer.is_null() {
            return Err(io::Error::last_os_error().into());
        }

        Ok(pointer.cast())
    }

    pub fn free_memory(&self, address: *mut c_void) -> Result<(), FreeMemoryError> {
        unsafe {
            if VirtualFreeEx(self.handle.as_raw(), address.cast(), 0, MEM_RELEASE) == 0 {
                return Err(io::Error::last_os_error().into());
            }
        }
        Ok(())
    }

    pub fn set_memory_permissions(
        &self,
        address: *mut c_void,
        size: usize,
        permissions: MemoryPermissions,
    ) -> Result<MemoryPermissions, SetMemoryPermissionsError> {
        let mut previous_constant = 0;
        unsafe {
            if VirtualProtectEx(
                self.handle.as_raw(),
                address.cast(),
                size,
                permissions.to_winapi_constant(),
                std::ptr::addr_of_mut!(previous_constant),
            ) == 0
            {
                return Err(io::Error::last_os_error().into());
            }
        }
        Ok(MemoryPermissions::from_winapi_constant(previous_constant))
    }

    pub unsafe fn read<T: Copy>(&self, address: *const T) -> Result<T, ReadMemoryError> {
        use std::alloc::{alloc, dealloc, Layout};

        unsafe {
            let data = alloc(Layout::array::<T>(1).unwrap());
            if ReadProcessMemory(
                self.handle.as_raw(),
                address.cast(),
                data.cast(),
                size_of::<T>(),
                NULL.cast(),
            ) == 0
            {
                dealloc(data, Layout::array::<T>(1).unwrap());
                return Err(io::Error::last_os_error().into());
            }
            let result = *data.cast();
            dealloc(data, Layout::array::<T>(1).unwrap());

            Ok(result)
        }
    }

    pub fn read_to_vec(&self, address: *const u8, size: usize) -> Result<Vec<u8>, ReadMemoryError> {
        let mut data = vec![0; size];
        unsafe {
            if ReadProcessMemory(
                self.handle.as_raw(),
                address.cast(),
                data.as_mut_ptr().cast(),
                size,
                NULL.cast(),
            ) == 0
            {
                return Err(io::Error::last_os_error().into());
            }
        }
        Ok(data)
    }

    pub fn read_u8(&self, address: *const u8) -> Result<u8, ReadMemoryError> {
        Ok(self.read_to_vec(address, 1)?[0])
    }

    #[expect(clippy::missing_panics_doc)]
    pub fn read_u16(&self, address: *const u16) -> Result<u16, ReadMemoryError> {
        Ok(u16::from_le_bytes(
            <[u8; 2]>::try_from(self.read_to_vec(address.cast(), 2)?).unwrap(),
        ))
    }

    #[expect(clippy::missing_panics_doc)]
    pub fn read_u32(&self, address: *const u32) -> Result<u32, ReadMemoryError> {
        Ok(u32::from_le_bytes(
            <[u8; 4]>::try_from(self.read_to_vec(address.cast(), 4)?).unwrap(),
        ))
    }

    #[expect(clippy::not_unsafe_ptr_arg_deref)]
    pub fn read_nul_terminated_string(
        &self,
        address: *const u8,
    ) -> Result<String, ReadMemoryError> {
        let mut string = String::new();
        for index in 0.. {
            let next_byte = self.read_u8(unsafe { address.add(index) })?;
            if next_byte == 0 {
                break;
            }
            string.push(next_byte as char);
        }
        Ok(string)
    }

    pub fn write(&self, address: *mut u8, data: &[u8]) -> Result<(), WriteMemoryError> {
        unsafe {
            if WriteProcessMemory(
                self.handle.as_raw(),
                address.cast(),
                data.as_ptr().cast(),
                data.len(),
                NULL.cast(),
            ) == 0
            {
                return Err(io::Error::last_os_error().into());
            }
        }

        Ok(())
    }

    pub unsafe fn create_thread(
        &self,
        start_address: *mut c_void,
        suspended: bool,
        parameter: Option<*mut c_void>,
    ) -> Result<Thread, CreateThreadError> {
        let thread_handle = unsafe {
            CreateRemoteThread(
                self.handle.as_raw(),
                NULL.cast(),
                0,
                Some(std::mem::transmute::<
                    *mut c_void,
                    unsafe extern "system" fn(*mut winapi::ctypes::c_void) -> u32,
                >(start_address)),
                parameter.map_or(NULL, <*mut _>::cast),
                if suspended { CREATE_SUSPENDED } else { 0 },
                NULL.cast(),
            )
        };

        if thread_handle == NULL {
            return Err(io::Error::last_os_error().into());
        }

        Ok(unsafe { Thread::from_raw_handle(thread_handle) })
    }

    pub fn get_memory_region(
        &self,
        address: *mut c_void,
    ) -> Result<MemoryRegion, GetMemoryRegionError> {
        unsafe {
            let mut winapi_region = MaybeUninit::zeroed().assume_init();
            if VirtualQueryEx(
                self.handle.as_raw(),
                address.cast(),
                &mut winapi_region,
                size_of_val(&winapi_region),
            ) == 0
            {
                return Err(io::Error::last_os_error().into());
            }
            Ok(if winapi_region.State == MEM_FREE {
                MemoryRegion::Free(FreeMemoryRegion {
                    address: winapi_region.BaseAddress.cast(),
                    size: winapi_region.RegionSize,
                })
            } else {
                MemoryRegion::Reserved(ReservedMemoryRegion {
                    address: winapi_region.BaseAddress.cast(),
                    size: winapi_region.RegionSize,
                    is_committed: winapi_region.State == MEM_COMMIT,
                    allocation_address: winapi_region.AllocationBase.cast(),
                    permissions: MemoryPermissions::from_winapi_constant(winapi_region.Protect),
                })
            })
        }
    }

    pub async fn inject_dll(&self, library_path: &str) -> Result<(), InjectDllError> {
        let library_path_c_string =
            CString::new(library_path).map_err(LibraryPathContainsNulError)?;

        let no_op_function_pointer = self.allocate_memory(
            1,
            MemoryPermissions {
                rwe: MemoryPermissionsRwe::ReadExecute,
                is_guard: false,
            },
        )?;
        self.write(no_op_function_pointer.cast(), &[0xc3])?; // opcode c3 is ret in both x86 and x64

        unsafe {
            self.create_thread(no_op_function_pointer, false, None)?
                .join()
                .await?;
        }

        let injected_dll_path_pointer = self
            .allocate_memory(
                library_path_c_string.to_bytes_with_nul().len(),
                MemoryPermissions {
                    rwe: MemoryPermissionsRwe::ReadWrite,
                    is_guard: false,
                },
            )?
            .cast();
        self.write(
            injected_dll_path_pointer,
            library_path_c_string.as_bytes_with_nul(),
        )?;

        let kernel32_module = self
            .get_module(OsStr::new("kernel32.dll"))?
            .expect("kernel32.dll module not found");
        let load_library_a_pointer = kernel32_module.get_export_address("LoadLibraryA")?;
        let get_last_error_pointer = kernel32_module.get_export_address("GetLastError")?;
        let load_dll_function = {
            if self.is_64_bit()? {
                let mut function = vec![
                    // special care must be taken to preserve the initial value of rsp and to
                    // reserve 32 bytes of shadow store for LoadLibraryA, all while ensuring the
                    // stack is aligned to a multiple of 16 bytes when calling LoadLibraryA
                    0x48, 0x89, 0xe0, // mov rax, rsp
                    0x48, 0x83, 0xe4, 0xf0, // and rsp, 0xfffffffffffffff0 (aligns stack)
                    0x50, // push rax (misaligns stack)
                    0x48, 0x83, 0xec, 0x28, // sub rsp, 0x28 (realigns stack)
                    //
                    0x48, 0xb9, 0, 0, 0, 0, 0, 0, 0, 0, // mov rcx, injected_dll_path_pointer
                    0x48, 0xb8, 0, 0, 0, 0, 0, 0, 0, 0, // mov rax, load_library_a_pointer
                    0xff, 0xd0, // call rax
                    0x48, 0x85, 0xc0, // test rax, rax
                    0x48, 0xc7, 0xc0, 0x00, 0x00, 0x00, 0x00, // mov rax, 0 (preserves ZF)
                    0x75, 0x0c, // jne return
                    0x48, 0xb8, 0, 0, 0, 0, 0, 0, 0, 0, // mov rax, get_last_error_pointer
                    0xff, 0xd0, // call rax
                    // return:
                    0x48, 0x83, 0xc4, 0x28, // add rsp, 0x28
                    0x5c, // pop rsp
                    0xc3, // ret
                ];
                function[14..][..8]
                    .copy_from_slice(&(injected_dll_path_pointer as usize).to_le_bytes());
                function[24..][..8]
                    .copy_from_slice(&(load_library_a_pointer as usize).to_le_bytes());
                function[48..][..8]
                    .copy_from_slice(&(get_last_error_pointer as usize).to_le_bytes());
                function
            } else {
                let mut function = vec![
                    0x68, 0, 0, 0, 0, // push injected_dll_path_pointer
                    0xb8, 0, 0, 0, 0, // mov eax, load_library_a_pointer
                    0xff, 0xd0, // call eax
                    0x85, 0xc0, // test eax, eax
                    0xb8, 0x00, 0x00, 0x00, 0x00, // mov eax, 0 (preserves ZF)
                    0x75, 0x07, // jne return
                    0xb8, 0, 0, 0, 0, // mov eax, get_last_error_pointer
                    0xff, 0xd0, // call eax
                    // return:
                    0xc3, // ret
                ];
                function[1..][..4]
                    .copy_from_slice(&(injected_dll_path_pointer as usize).to_le_bytes()[..4]);
                function[6..][..4]
                    .copy_from_slice(&(load_library_a_pointer as usize).to_le_bytes()[..4]);
                function[22..][..4]
                    .copy_from_slice(&(get_last_error_pointer as usize).to_le_bytes()[..4]);
                function
            }
        };
        let load_dll_function_pointer = self.allocate_memory(
            load_dll_function.len(),
            MemoryPermissions {
                rwe: MemoryPermissionsRwe::ReadExecute,
                is_guard: false,
            },
        )?;
        self.write(load_dll_function_pointer.cast(), &load_dll_function)?;

        unsafe {
            match self
                .create_thread(load_dll_function_pointer, false, None)?
                .join()
                .await?
            {
                0 => Ok(()),
                error_code => Err(LoadLibraryThreadError { error_code }.into()),
            }
        }
    }
}

pub enum MemoryRegion {
    Free(FreeMemoryRegion),
    Reserved(ReservedMemoryRegion),
}

impl MemoryRegion {
    #[must_use]
    pub fn address(&self) -> *mut c_void {
        match self {
            MemoryRegion::Free(free_memory_region) => free_memory_region.address,
            MemoryRegion::Reserved(reserved_memory_region) => reserved_memory_region.address,
        }
    }

    #[must_use]
    pub fn size(&self) -> usize {
        match self {
            MemoryRegion::Free(free_memory_region) => free_memory_region.size,
            MemoryRegion::Reserved(reserved_memory_region) => reserved_memory_region.size,
        }
    }
}

pub struct FreeMemoryRegion {
    address: *mut c_void,
    size: usize,
}

impl FreeMemoryRegion {
    #[must_use]
    pub fn address(&self) -> *mut c_void {
        self.address
    }

    #[must_use]
    pub fn size(&self) -> usize {
        self.size
    }
}

pub struct ReservedMemoryRegion {
    address: *mut c_void,
    size: usize,
    is_committed: bool,
    allocation_address: *mut c_void,
    permissions: MemoryPermissions,
}

impl ReservedMemoryRegion {
    #[must_use]
    pub fn address(&self) -> *mut c_void {
        self.address
    }

    #[must_use]
    pub fn size(&self) -> usize {
        self.size
    }

    #[must_use]
    pub fn is_committed(&self) -> bool {
        self.is_committed
    }

    #[must_use]
    pub fn allocation_address(&self) -> *mut c_void {
        self.allocation_address
    }

    #[must_use]
    pub fn permissions(&self) -> MemoryPermissions {
        self.permissions
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MemoryPermissions {
    pub rwe: MemoryPermissionsRwe,
    pub is_guard: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryPermissionsRwe {
    Unknown = 0x0,
    None = 0x1,
    Read = 0x2,
    ReadWrite = 0x4,
    ReadWriteCow = 0x8,
    Execute = 0x10,
    ReadExecute = 0x20,
    ReadWriteExecute = 0x40,
}

impl MemoryPermissions {
    #[must_use]
    pub fn from_winapi_constant(constant: u32) -> Self {
        let guard = constant & 0x100 != 0;
        let rwe = match constant & 0xff {
            0x0 => MemoryPermissionsRwe::Unknown,
            0x1 => MemoryPermissionsRwe::None,
            0x2 => MemoryPermissionsRwe::Read,
            0x4 => MemoryPermissionsRwe::ReadWrite,
            0x8 => MemoryPermissionsRwe::ReadWriteCow,
            0x10 => MemoryPermissionsRwe::Execute,
            0x20 => MemoryPermissionsRwe::ReadExecute,
            0x40 => MemoryPermissionsRwe::ReadWriteExecute,
            _ => unimplemented!("memory permissions constant: {constant:#x}"),
        };
        Self {
            rwe,
            is_guard: guard,
        }
    }

    #[must_use]
    pub fn to_winapi_constant(&self) -> u32 {
        let rwe = match self.rwe {
            MemoryPermissionsRwe::Unknown => 0x0,
            MemoryPermissionsRwe::None => 0x1,
            MemoryPermissionsRwe::Read => 0x2,
            MemoryPermissionsRwe::ReadWrite => 0x4,
            MemoryPermissionsRwe::ReadWriteCow => 0x8,
            MemoryPermissionsRwe::Execute => 0x10,
            MemoryPermissionsRwe::ReadExecute => 0x20,
            MemoryPermissionsRwe::ReadWriteExecute => 0x40,
        };
        (if self.is_guard { 0x100 } else { 0 }) | rwe
    }
}

pub struct ThreadIdIterator {
    process_id: u32,
    snapshot_handle: Handle,
    called_thread_32_first: bool,
}

impl ThreadIdIterator {
    pub(in crate::windows::process) fn new(
        process_id: u32,
    ) -> Result<Self, NewThreadIdIteratorError> {
        let snapshot_handle = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) };
        if snapshot_handle == INVALID_HANDLE_VALUE {
            return Err(io::Error::last_os_error().into());
        }

        Ok(ThreadIdIterator {
            process_id,
            snapshot_handle: unsafe { Handle::from_raw(snapshot_handle) },
            called_thread_32_first: false,
        })
    }
}

impl Iterator for ThreadIdIterator {
    type Item = u32;

    fn next(&mut self) -> Option<Self::Item> {
        // https://devblogs.microsoft.com/oldnewthing/20060223-14/?p=32173

        let mut entry = THREADENTRY32 {
            #[expect(clippy::cast_possible_truncation)]
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
                    Thread32Next(self.snapshot_handle.as_raw(), &mut entry)
                } else {
                    self.called_thread_32_first = true;
                    Thread32First(self.snapshot_handle.as_raw(), &mut entry)
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

#[derive(Debug, Error)]
#[error("failed to create process")]
pub enum CreateError {
    PathContainsNul(#[from] NulError),
    Os(#[from] io::Error),
}

#[derive(Debug, Error)]
#[error("failed to determine whether process is 64-bit")]
pub enum CheckIs64BitError {
    UnknownMachine(#[from] UnknownMachineError),
    Os(#[from] io::Error),
}

#[derive(Debug, Error)]
#[error("unknown machine with id: 0x{:x}", .0)]
pub struct UnknownMachineError(u16);

#[derive(Debug, Error)]
#[error("failed to set process to be killed on current process exit")]
pub struct KillOnCurrentProcessExitError(#[from] io::Error);

#[derive(Debug, Error)]
#[error("error occurred while joining process")]
pub struct JoinError(#[from] handle::WaitError);

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
#[error("failed to get process modules")]
pub struct GetModulesError(#[from] io::Error);

#[derive(Debug, Error)]
#[error("failed to allocate memory")]
pub struct AllocateMemoryError(#[from] io::Error);

#[derive(Debug, Error)]
#[error("failed to free memory")]
pub struct FreeMemoryError(#[from] io::Error);

#[derive(Debug, Error)]
#[error("failed to set memory permissions")]
pub struct SetMemoryPermissionsError(#[from] io::Error);

#[derive(Debug, Error)]
#[error("failed to read from memory")]
pub struct ReadMemoryError(#[from] io::Error);

#[derive(Debug, Error)]
#[error("failed to write to memory")]
pub struct WriteMemoryError(#[from] io::Error);

#[derive(Debug, Error)]
#[error("failed to create thread")]
pub struct CreateThreadError(#[from] io::Error);

#[derive(Debug, Error)]
#[error("failed to get memory region metadata")]
pub struct GetMemoryRegionError(#[from] io::Error);

#[derive(Debug, Error)]
#[error("failed to inject dll")]
pub enum InjectDllError {
    LibraryPathContainsNul(#[from] LibraryPathContainsNulError),
    GetModules(#[from] GetModulesError),
    ModuleGetExportAddress(#[from] module::GetExportAddressError),
    AllocateMemory(#[from] AllocateMemoryError),
    ReadMemory(#[from] ReadMemoryError),
    WriteMemory(#[from] WriteMemoryError),
    CreateThread(#[from] CreateThreadError),
    JoinThread(#[from] crate::windows::thread::JoinError),
    LoadLibraryThread(#[from] LoadLibraryThreadError),
    CheckIs64Bit(#[from] CheckIs64BitError),
}

#[derive(Debug, Error)]
#[error("library loading thread returned with error code 0x{error_code:x}")]
pub struct LoadLibraryThreadError {
    error_code: u32,
}

#[derive(Debug, Error)]
#[error("library path contains nul")]
pub struct LibraryPathContainsNulError(#[from] NulError);

#[derive(Debug, Error)]
#[error("failed to create thread id iterator")]
pub struct NewThreadIdIteratorError(#[from] io::Error);
