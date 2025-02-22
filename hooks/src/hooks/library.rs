use crate::hooks;
use hooks_macros::{hook, hooks};
use shared::windows::{module, process};
use winapi::{
    ctypes::c_void,
    shared::{
        minwindef::HMODULE,
        ntdef::{HANDLE, NULL},
    },
    um::{
        libloaderapi::{
            FreeLibrary, FreeLibraryAndExitThread, LoadLibraryA, LoadLibraryExA, LoadLibraryExW,
            LoadLibraryW,
        },
        processthreadsapi::ExitThread,
    },
};

pub(crate) const HOOKS: &[(&str, &str, *const c_void)] = &hooks![
    LoadLibraryA,
    LoadLibraryW,
    LoadLibraryExA,
    LoadLibraryExW,
    FreeLibrary,
    FreeLibraryAndExitThread,
];

#[hook("kernel32.dll")]
unsafe extern "system" fn LoadLibraryA(filename: *const i8) -> HMODULE {
    unsafe { LoadLibraryExA(filename, NULL, 0) }
}

#[hook("kernel32.dll")]
unsafe extern "system" fn LoadLibraryW(filename: *const u16) -> HMODULE {
    unsafe { LoadLibraryExW(filename, NULL, 0) }
}

#[hook("kernel32.dll")]
unsafe extern "system" fn LoadLibraryExA(filename: *const i8, _: HANDLE, flags: u32) -> HMODULE {
    unsafe { load_library(filename, flags, get_self_trampoline()) }
}

#[hook("kernel32.dll")]
unsafe extern "system" fn LoadLibraryExW(filename: *const u16, _: HANDLE, flags: u32) -> HMODULE {
    unsafe { load_library(filename, flags, get_self_trampoline()) }
}

#[hook("kernel32.dll")]
unsafe extern "system" fn FreeLibrary(_: HMODULE) -> i32 {
    // Prevent libraries from being unloaded so that they do not have to be rehooked
    1
}

#[hook("kernel32.dll")]
unsafe extern "system" fn FreeLibraryAndExitThread(module: HMODULE, exit_code: u32) {
    unsafe {
        FreeLibrary(module);
        ExitThread(exit_code);
    }
}

unsafe fn load_library<T>(
    filename: T,
    flags: u32,
    trampoline: unsafe extern "system" fn(T, HANDLE, u32) -> HMODULE,
) -> HMODULE {
    let handle = unsafe { trampoline(filename, NULL, flags) };
    if handle.is_null() {
        return NULL.cast();
    }
    let current_process = process::Process::get_current();
    hooks::apply_to_module(&module::Module::from_raw_handle(&current_process, handle));
    handle
}
