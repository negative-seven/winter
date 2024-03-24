use crate::state::{self, State, STATE};
use minhook::MinHook;
use std::{collections::BTreeMap, sync::RwLock};
use winapi::{
    ctypes::c_void,
    shared::{ntdef::NULL, windef::HWND},
    um::winuser::{MSG, PM_REMOVE, WM_CHAR, WM_KEYDOWN, WM_KEYUP},
};

static TRAMPOLINES: RwLock<BTreeMap<String, usize>> = RwLock::new(BTreeMap::new());

fn get_trampoline(function_name: impl AsRef<str>) -> *const c_void {
    *TRAMPOLINES
        .read()
        .unwrap()
        .get(function_name.as_ref())
        .unwrap() as *const c_void
}

fn set_trampoline(name: impl AsRef<str>, pointer: usize) {
    TRAMPOLINES
        .write()
        .unwrap()
        .insert(name.as_ref().to_string(), pointer);
}

pub fn initialize() {
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
        (
            "kernel32.dll",
            "GetTickCount64",
            get_tick_count_64 as *const c_void,
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
        fn hook_function(
            module_name: &str,
            function_name: &str,
            hook: *const c_void,
        ) -> Result<(), Box<dyn std::error::Error>> {
            let process = shared::process::Process::get_current();
            let function_address = process.get_export_address(module_name, function_name)?;
            unsafe {
                let original_function = MinHook::create_hook(
                    function_address as *mut std::ffi::c_void,
                    hook as *mut std::ffi::c_void,
                )
                .unwrap();
                MinHook::enable_hook(function_address as *mut std::ffi::c_void).unwrap();
                set_trampoline(function_name, original_function as usize);
            }
            Ok(())
        }
        let _ = hook_function(module_name, function_name, hook);
    }
}

extern "system" fn get_keyboard_state(key_states: *mut u8) {
    let state = STATE.lock().unwrap();
    for i in 0u8..=255u8 {
        unsafe {
            *(key_states.offset(isize::from(i))) = u8::from(state.get_key_state(i)) << 7;
        }
    }
}

#[allow(clippy::cast_possible_truncation)]
extern "system" fn get_key_state(id: u32) -> u16 {
    u16::from(STATE.lock().unwrap().get_key_state(id as u8)) << 15
}

extern "system" fn get_async_key_state(id: u32) -> u16 {
    get_key_state(id)
}

extern "system" fn sleep(milliseconds: u32) {
    state::sleep(u64::from(milliseconds) * State::TICKS_PER_SECOND / 1000);
    unsafe {
        let trampoline: extern "system" fn(u32) = std::mem::transmute(get_trampoline("Sleep"));
        trampoline(milliseconds);
    }
}

unsafe extern "system" fn peek_message(
    message: *mut MSG,
    window_filter: HWND,
    minimum_id_filter: u32,
    maximum_id_filter: u32,
    flags: u32,
) -> u32 {
    {
        let mut state = STATE.lock().unwrap();
        if !state.custom_message_queue.is_empty() {
            let id_filter = if minimum_id_filter == 0 && maximum_id_filter == 0 {
                u32::MIN..=u32::MAX
            } else {
                minimum_id_filter..=maximum_id_filter
            };

            for (custom_message_index, custom_message) in
                state.custom_message_queue.iter().enumerate()
            {
                if window_filter != NULL.cast() && custom_message.0.hwnd != window_filter {
                    continue;
                }
                if !id_filter.contains(&custom_message.0.message) {
                    continue;
                }
                *message = custom_message.0;
                if flags & PM_REMOVE != 0 {
                    state.custom_message_queue.remove(custom_message_index);
                }
                return 1;
            }
        }
    }

    let trampoline: extern "system" fn(*mut MSG, HWND, u32, u32, u32) -> u32 =
        std::mem::transmute(get_trampoline("PeekMessageA"));
    let result = trampoline(
        message,
        window_filter,
        minimum_id_filter,
        maximum_id_filter,
        flags,
    );
    if result != 0 && matches!((*message).message, WM_KEYDOWN | WM_KEYUP | WM_CHAR) {
        (*message).message = 0;
    }
    result
}

#[allow(clippy::cast_possible_truncation)]
extern "system" fn get_tick_count() -> u32 {
    (state::get_ticks_with_busy_wait() * 1000 / State::TICKS_PER_SECOND) as u32
}

#[allow(clippy::cast_possible_truncation)]
extern "system" fn get_tick_count_64() -> u64 {
    (u128::from(state::get_ticks_with_busy_wait()) * 1000 / u128::from(State::TICKS_PER_SECOND))
        as u64
}

extern "system" fn time_get_time() -> u32 {
    get_tick_count()
}

const SIMULATED_PERFORMANCE_COUNTER_FREQUENCY: u64 = 1 << 32;

extern "system" fn query_performance_frequency(frequency: *mut u32) -> u32 {
    // due to pointer alignment issues, frequency must be split into two u32 chunks

    #[allow(clippy::cast_possible_truncation)]
    unsafe {
        *frequency = SIMULATED_PERFORMANCE_COUNTER_FREQUENCY as u32;
        *frequency.offset(1) = (SIMULATED_PERFORMANCE_COUNTER_FREQUENCY >> 32) as u32;
    }

    1
}

extern "system" fn query_performance_counter(count: *mut u32) -> u32 {
    // due to pointer alignment issues, count must be split into two u32 chunks

    #[allow(clippy::cast_possible_truncation)]
    unsafe {
        let simulated_performance_counter = state::get_ticks_with_busy_wait()
            * SIMULATED_PERFORMANCE_COUNTER_FREQUENCY
            / State::TICKS_PER_SECOND;
        *count = simulated_performance_counter as u32;
        *count.offset(1) = (simulated_performance_counter >> 32) as u32;
    }

    1
}
