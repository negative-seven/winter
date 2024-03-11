#![allow(clippy::missing_panics_doc)]

mod hooks;

use hooks::{
    get_async_key_state, get_key_state, get_keyboard_state, get_tick_count, peek_message,
    query_performance_counter, query_performance_frequency, sleep, time_get_time,
};
use minhook::MinHook;
use shared::{
    communication::{HooksMessage, HooksTransceiver},
    process,
};
use static_init::dynamic;
use std::{collections::HashMap, error::Error, ffi::c_void};

#[allow(clippy::ignored_unit_patterns)] // lint triggered inside macro
#[dynamic]
static mut TRAMPOLINES: HashMap<String, usize> = HashMap::new();

unsafe fn get_trampoline(function_name: impl AsRef<str>) -> *const c_void {
    *TRAMPOLINES.read().get(function_name.as_ref()).unwrap() as *const c_void
}

const SIMULATED_PERFORMANCE_COUNTER_FREQUENCY: u64 = 1 << 32;

#[allow(clippy::ignored_unit_patterns)] // lint triggered inside macro
#[dynamic]
static mut TICKS: u64 = 0;

const TICKS_PER_SECOND: u64 = 3000;

#[allow(clippy::ignored_unit_patterns)] // lint triggered inside macro
#[dynamic]
static mut BUSY_WAIT_COUNT: u64 = 0;

fn hook_function(
    module_name: &str,
    function_name: &str,
    hook: *const c_void,
) -> Result<(), Box<dyn Error>> {
    let process = process::Process::get_current();
    let function_address = process.get_export_address(module_name, function_name)?;
    unsafe {
        #[allow(clippy::ptr_cast_constness)] // the pointer being mutable seems to be pointless
        let original_function =
            MinHook::create_hook(function_address as *mut c_void, hook as *mut c_void).unwrap();
        MinHook::enable_hook(function_address as *mut c_void).unwrap();
        TRAMPOLINES
            .write()
            .insert(function_name.to_string(), original_function as usize);
    }
    Ok(())
}

#[no_mangle]
pub extern "stdcall" fn initialize(serialized_transceiver_pointer: usize) {
    let mut transceiver = HooksTransceiver::from_bytes(
        process::Process::get_current()
            .read_to_vec(serialized_transceiver_pointer, 16)
            .unwrap()
            .try_into()
            .unwrap(),
    );

    for (module_name, function_name, hook) in [
        (
            "user32.dll",
            "GetKeyboardState",
            get_keyboard_state as *const c_void,
        ),
        ("user32.dll", "GetKeyState", get_key_state as *const c_void),
        (
            "user32.dll",
            "GetAsyncKeyState",
            get_async_key_state as *const c_void,
        ),
        ("kernel32.dll", "Sleep", sleep as *const c_void),
        ("user32.dll", "PeekMessageA", peek_message as *const c_void),
        (
            "kernel32.dll",
            "GetTickCount",
            get_tick_count as *const c_void,
        ),
        ("winmm.dll", "timeGetTime", time_get_time as *const c_void),
        (
            "kernel32.dll",
            "QueryPerformanceFrequency",
            query_performance_frequency as *const c_void,
        ),
        (
            "kernel32.dll",
            "QueryPerformanceCounter",
            query_performance_counter as *const c_void,
        ),
    ] {
        let _ = hook_function(module_name, function_name, hook);
    }

    transceiver.send(&HooksMessage::HooksInitialized).unwrap();
}
