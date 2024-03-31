use crate::state::{self, State, STATE};
use minhook::MinHook;
use std::{collections::BTreeMap, sync::RwLock};
use winapi::{
    ctypes::c_void,
    shared::{minwindef::FILETIME, ntdef::NULL, windef::HWND},
    um::{
        profileapi::{QueryPerformanceCounter, QueryPerformanceFrequency},
        synchapi::Sleep,
        sysinfoapi::{
            GetSystemTimeAsFileTime, GetSystemTimePreciseAsFileTime, GetTickCount, GetTickCount64,
        },
        timeapi::timeGetTime,
        winnt::LARGE_INTEGER,
        winsock2::{socket, INVALID_SOCKET},
        winuser::{
            GetAsyncKeyState, GetKeyState, GetKeyboardState, PeekMessageA, PeekMessageW, MSG,
            PM_REMOVE, WM_CHAR, WM_KEYDOWN, WM_KEYUP,
        },
    },
};

static TRAMPOLINES: RwLock<BTreeMap<String, usize>> = RwLock::new(BTreeMap::new());

macro_rules! get_trampoline {
    ($name:expr, $type:ty $(,)?) => {{
        let mut f: $type;
        #[allow(unused_assignments)]
        {
            f = $name; // type check
        }
        #[allow(unused_unsafe)]
        unsafe {
            f = std::mem::transmute(*TRAMPOLINES.read().unwrap().get(stringify!($name)).unwrap())
        };
        f
    }};
}

fn set_trampoline(name: impl AsRef<str>, pointer: *const c_void) {
    TRAMPOLINES
        .write()
        .unwrap()
        .insert(name.as_ref().to_string(), pointer as usize);
}

macro_rules! hook {
    ($module:expr, $original:expr, $new:expr, $type:ty $(,)?) => {
        #[allow(unused_qualifications)]
        {
            #[allow(unused_assignments)]
            #[allow(unused_variables)]
            {
                let mut f: $type;
                f = $original; // type check
                f = $new; // type check
            }

            (
                $module,
                stringify!($original),
                $new as *const winapi::ctypes::c_void,
            )
        }
    };
}

const HOOKS: &[(&str, &str, *const c_void)] = &[
    hook!(
        "user32.dll",
        GetKeyboardState,
        get_keyboard_state,
        unsafe extern "system" fn(*mut u8) -> i32,
    ),
    hook!(
        "user32.dll",
        GetKeyState,
        get_key_state,
        unsafe extern "system" fn(i32) -> i16,
    ),
    hook!(
        "user32.dll",
        GetAsyncKeyState,
        get_async_key_state,
        unsafe extern "system" fn(i32) -> i16,
    ),
    hook!("kernel32.dll", Sleep, sleep, unsafe extern "system" fn(u32)),
    hook!(
        "user32.dll",
        PeekMessageA,
        peek_message_a,
        unsafe extern "system" fn(*mut MSG, HWND, u32, u32, u32) -> i32,
    ),
    hook!(
        "user32.dll",
        PeekMessageW,
        peek_message_w,
        unsafe extern "system" fn(*mut MSG, HWND, u32, u32, u32) -> i32,
    ),
    hook!(
        "kernel32.dll",
        GetTickCount,
        get_tick_count,
        unsafe extern "system" fn() -> u32,
    ),
    hook!(
        "kernel32.dll",
        GetTickCount64,
        get_tick_count_64,
        unsafe extern "system" fn() -> u64,
    ),
    hook!(
        "winmm.dll",
        timeGetTime,
        time_get_time,
        unsafe extern "system" fn() -> u32,
    ),
    hook!(
        "kernel32.dll",
        QueryPerformanceFrequency,
        query_performance_frequency,
        unsafe extern "system" fn(*mut LARGE_INTEGER) -> i32,
    ),
    hook!(
        "kernel32.dll",
        QueryPerformanceCounter,
        query_performance_counter,
        unsafe extern "system" fn(*mut LARGE_INTEGER) -> i32,
    ),
    hook!(
        "kernel32.dll",
        GetSystemTimeAsFileTime,
        get_system_time_as_file_time,
        unsafe extern "system" fn(*mut FILETIME),
    ),
    hook!(
        "kernel32.dll",
        GetSystemTimePreciseAsFileTime,
        get_system_time_precise_as_file_time,
        unsafe extern "system" fn(*mut FILETIME),
    ),
    hook!(
        "ws2_32.dll",
        socket,
        socket_,
        unsafe extern "system" fn(i32, i32, i32) -> usize,
    ),
];

#[allow(clippy::too_many_lines)] // TODO
pub(crate) fn initialize() {
    for (module_name, function_name, hook) in HOOKS {
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
                set_trampoline(function_name, original_function.cast());
            }
            Ok(())
        }
        let _unused_result = hook_function(module_name, function_name, *hook);
    }
}

unsafe extern "system" fn get_keyboard_state(key_states: *mut u8) -> i32 {
    let state = STATE.lock().unwrap();
    for i in 0u8..=255u8 {
        unsafe {
            *(key_states.offset(isize::from(i))) = u8::from(state.get_key_state(i)) << 7;
        }
    }
    1
}

#[allow(clippy::cast_possible_truncation)]
#[allow(clippy::cast_sign_loss)]
unsafe extern "system" fn get_key_state(id: i32) -> i16 {
    i16::from(STATE.lock().unwrap().get_key_state(id as u8)) << 15
}

unsafe extern "system" fn get_async_key_state(id: i32) -> i16 {
    unsafe { get_key_state(id) }
}

extern "system" fn sleep(milliseconds: u32) {
    state::sleep(u64::from(milliseconds) * State::TICKS_PER_SECOND / 1000);
}

unsafe extern "system" fn peek_message_a(
    message: *mut MSG,
    window_filter: HWND,
    minimum_id_filter: u32,
    maximum_id_filter: u32,
    flags: u32,
) -> i32 {
    unsafe {
        peek_message(
            message,
            window_filter,
            minimum_id_filter,
            maximum_id_filter,
            flags,
            false,
        )
    }
}

unsafe extern "system" fn peek_message_w(
    message: *mut MSG,
    window_filter: HWND,
    minimum_id_filter: u32,
    maximum_id_filter: u32,
    flags: u32,
) -> i32 {
    unsafe {
        peek_message(
            message,
            window_filter,
            minimum_id_filter,
            maximum_id_filter,
            flags,
            true,
        )
    }
}

unsafe extern "system" fn peek_message(
    message: *mut MSG,
    window_filter: HWND,
    minimum_id_filter: u32,
    maximum_id_filter: u32,
    flags: u32,
    unicode_strings: bool,
) -> i32 {
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
                unsafe {
                    *message = custom_message.0;
                }
                if flags & PM_REMOVE != 0 {
                    state.custom_message_queue.remove(custom_message_index);
                }
                return 1;
            }
        }
    }

    let trampoline = if unicode_strings {
        get_trampoline!(
            PeekMessageW,
            unsafe extern "system" fn(*mut MSG, HWND, u32, u32, u32) -> i32
        )
    } else {
        get_trampoline!(
            PeekMessageA,
            unsafe extern "system" fn(*mut MSG, HWND, u32, u32, u32) -> i32
        )
    };
    unsafe {
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

unsafe extern "system" fn query_performance_frequency(frequency: *mut LARGE_INTEGER) -> i32 {
    #[allow(clippy::cast_possible_wrap)]
    unsafe {
        *(*frequency).QuadPart_mut() = SIMULATED_PERFORMANCE_COUNTER_FREQUENCY as i64;
    }

    1
}

unsafe extern "system" fn query_performance_counter(count: *mut LARGE_INTEGER) -> i32 {
    #[allow(clippy::cast_possible_wrap)]
    unsafe {
        let simulated_performance_counter = state::get_ticks_with_busy_wait()
            * SIMULATED_PERFORMANCE_COUNTER_FREQUENCY
            / State::TICKS_PER_SECOND;
        *(*count).QuadPart_mut() = simulated_performance_counter as i64;
    }

    1
}

unsafe extern "system" fn get_system_time_as_file_time(file_time: *mut FILETIME) {
    #[allow(clippy::cast_possible_truncation)]
    let one_hundred_nanosecond_intervals = (u128::from(state::get_ticks_with_busy_wait())
        * 10_000_000
        / u128::from(State::TICKS_PER_SECOND)) as u64;

    unsafe {
        (*file_time).dwLowDateTime = (one_hundred_nanosecond_intervals & ((1 << 32) - 1)) as u32;
        (*file_time).dwHighDateTime = (one_hundred_nanosecond_intervals >> 32) as u32;
    }
}

unsafe extern "system" fn get_system_time_precise_as_file_time(file_time: *mut FILETIME) {
    unsafe { get_system_time_as_file_time(file_time) }
}

unsafe extern "system" fn socket_(_address_family: i32, _type: i32, _protocol: i32) -> usize {
    INVALID_SOCKET
}
