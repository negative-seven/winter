#![allow(clippy::missing_panics_doc)]

mod hooks;
mod state;

use hooks::{
    get_async_key_state, get_key_state, get_keyboard_state, get_tick_count, peek_message,
    query_performance_counter, query_performance_frequency, sleep, time_get_time,
};
use minhook::MinHook;
use shared::{
    communication::{HooksMessage, RuntimeMessage, Transceiver},
    process,
};
use static_init::dynamic;
use std::{collections::HashMap, error::Error, ffi::c_void, mem::MaybeUninit};

#[allow(clippy::ignored_unit_patterns)] // lint triggered inside macro
#[dynamic]
static mut TRAMPOLINES: HashMap<String, usize> = HashMap::new();

unsafe fn get_trampoline(function_name: impl AsRef<str>) -> *const c_void {
    *TRAMPOLINES.read().get(function_name.as_ref()).unwrap() as *const c_void
}

static mut TRANSCEIVER: MaybeUninit<Transceiver<HooksMessage, RuntimeMessage>> =
    MaybeUninit::uninit();

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
    unsafe {
        TRANSCEIVER.write(Transceiver::<HooksMessage, RuntimeMessage>::from_bytes(
            process::Process::get_current()
                .read_to_vec(serialized_transceiver_pointer, 24)
                .unwrap()
                .try_into()
                .unwrap(),
        ));
    }

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

    let transceiver = unsafe { TRANSCEIVER.assume_init_ref() };
    transceiver.send(&HooksMessage::HooksInitialized).unwrap();
    loop {
        match transceiver.receive_blocking().unwrap() {
            #[allow(clippy::cast_possible_truncation)]
            RuntimeMessage::AdvanceTime(duration) => {
                state::STATE.lock().unwrap().pending_ticks +=
                    (duration.as_nanos() * u128::from(state::State::TICKS_PER_SECOND)
                        / std::time::Duration::from_secs(1).as_nanos()) as u64;
                state::TICKS_PENDING_EVENT.write().set().unwrap();
            }
            message => unimplemented!("handle message {message:?}"),
        }
    }
}
