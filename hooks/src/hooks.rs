use crate::state::{self, State, WaitableTimer, STATE};
use minhook::MinHook;
use ntapi::ntpsapi::{NtSetInformationThread, ThreadHideFromDebugger, THREADINFOCLASS};
use shared::process;
use std::{
    collections::BTreeMap,
    num::NonZeroU64,
    sync::{Arc, Mutex, RwLock},
};
use winapi::{
    ctypes::c_void,
    shared::{
        minwindef::FILETIME,
        ntdef::{HANDLE, NULL},
        ntstatus::STATUS_SUCCESS,
        windef::HWND,
        winerror::WAIT_TIMEOUT,
    },
    um::{
        handleapi::CloseHandle,
        minwinbase::{REASON_CONTEXT, SECURITY_ATTRIBUTES},
        profileapi::{QueryPerformanceCounter, QueryPerformanceFrequency},
        synchapi::{
            CreateWaitableTimerExW, CreateWaitableTimerW, SetWaitableTimer, SetWaitableTimerEx,
            Sleep, WaitForSingleObject, CREATE_WAITABLE_TIMER_MANUAL_RESET,
        },
        sysinfoapi::{
            GetSystemTimeAsFileTime, GetSystemTimePreciseAsFileTime, GetTickCount, GetTickCount64,
        },
        timeapi::timeGetTime,
        winbase::{CreateWaitableTimerA, CreateWaitableTimerExA, WAIT_OBJECT_0},
        winnt::{LARGE_INTEGER, TIMER_ALL_ACCESS},
        winsock2::{socket, INVALID_SOCKET},
        winuser::{
            GetAsyncKeyState, GetKeyState, GetKeyboardState, GetMessageA, GetMessageW,
            PeekMessageA, PeekMessageW, RegisterClassExA, RegisterClassExW, MSG, PM_REMOVE,
            WM_ACTIVATE, WM_ACTIVATEAPP, WM_CHAR, WM_KEYDOWN, WM_KEYUP, WM_KILLFOCUS,
            WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDOWN, WM_MBUTTONUP, WM_MOUSEMOVE, WM_QUIT,
            WM_RBUTTONDOWN, WM_RBUTTONUP, WM_SETFOCUS, WM_XBUTTONDOWN, WM_XBUTTONUP, WNDCLASSEXA,
            WNDCLASSEXW,
        },
    },
};

pub(crate) static TRAMPOLINES: RwLock<BTreeMap<String, usize>> = RwLock::new(BTreeMap::new());

macro_rules! get_trampoline {
    ($name:expr, $type:ty $(,)?) => {{
        let mut f: $type;
        #[expect(unused_assignments)]
        {
            f = $name; // type check
        }
        unsafe {
            f = std::mem::transmute::<usize, $type>(
                *crate::hooks::TRAMPOLINES
                    .read()
                    .unwrap()
                    .get(stringify!($name))
                    .unwrap(),
            )
        };
        f
    }};
}
pub(crate) use get_trampoline;

fn set_trampoline(name: impl AsRef<str>, pointer: *const c_void) {
    TRAMPOLINES
        .write()
        .unwrap()
        .insert(name.as_ref().to_string(), pointer as usize);
}

macro_rules! hook {
    ($module:expr, $original:expr, $new:expr, $type:ty $(,)?) => {{
        #[expect(unused_assignments)]
        #[expect(unused_variables)]
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
    }};
}

const HOOKS: &[(&str, &str, *const c_void)] = &[
    hook!(
        "kernel32.dll",
        CloseHandle,
        close_handle,
        unsafe extern "system" fn(*mut c_void) -> i32,
    ),
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
        RegisterClassExA,
        register_class_ex_a,
        unsafe extern "system" fn(*const WNDCLASSEXA) -> u16
    ),
    hook!(
        "user32.dll",
        RegisterClassExW,
        register_class_ex_w,
        unsafe extern "system" fn(*const WNDCLASSEXW) -> u16
    ),
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
        "user32.dll",
        GetMessageA,
        get_message_a,
        unsafe extern "system" fn(*mut MSG, HWND, u32, u32) -> i32
    ),
    hook!(
        "user32.dll",
        GetMessageW,
        get_message_w,
        unsafe extern "system" fn(*mut MSG, HWND, u32, u32) -> i32
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
        "kernel32.dll",
        CreateWaitableTimerA,
        create_waitable_timer_a,
        unsafe extern "system" fn(*mut SECURITY_ATTRIBUTES, i32, *const i8) -> *mut c_void,
    ),
    hook!(
        "kernel32.dll",
        CreateWaitableTimerW,
        create_waitable_timer_w,
        unsafe extern "system" fn(*mut SECURITY_ATTRIBUTES, i32, *const u16) -> *mut c_void,
    ),
    hook!(
        "kernel32.dll",
        CreateWaitableTimerExA,
        create_waitable_timer_ex_a,
        unsafe extern "system" fn(*mut SECURITY_ATTRIBUTES, *const i8, u32, u32) -> *mut c_void,
    ),
    hook!(
        "kernel32.dll",
        CreateWaitableTimerExW,
        create_waitable_timer_ex_w,
        unsafe extern "system" fn(*mut SECURITY_ATTRIBUTES, *const u16, u32, u32) -> *mut c_void,
    ),
    hook!(
        "kernel32.dll",
        SetWaitableTimer,
        set_waitable_timer,
        unsafe extern "system" fn(
            *mut c_void,
            *const LARGE_INTEGER,
            i32,
            Option<unsafe extern "system" fn(*mut c_void, u32, u32)>,
            *mut c_void,
            i32,
        ) -> i32,
    ),
    hook!(
        "kernelbase.dll",
        SetWaitableTimerEx,
        set_waitable_timer_ex,
        unsafe extern "system" fn(
            *mut c_void,
            *const LARGE_INTEGER,
            i32,
            Option<unsafe extern "system" fn(*mut c_void, u32, u32)>,
            *mut c_void,
            *mut REASON_CONTEXT,
            u32,
        ) -> i32,
    ),
    hook!(
        "kernel32.dll",
        WaitForSingleObject,
        wait_for_single_object,
        unsafe extern "system" fn(*mut c_void, u32) -> u32,
    ),
    hook!(
        "ws2_32.dll",
        socket,
        socket_,
        unsafe extern "system" fn(i32, i32, i32) -> usize,
    ),
    hook!(
        "ntdll.dll",
        NtSetInformationThread,
        nt_set_information_thread,
        unsafe extern "system" fn(HANDLE, THREADINFOCLASS, *mut c_void, u32) -> i32
    ),
];

pub(crate) fn initialize() {
    for (module_name, function_name, hook) in HOOKS {
        fn hook_function(
            module_name: &str,
            function_name: &str,
            hook: *const c_void,
        ) -> Result<(), Box<dyn std::error::Error>> {
            let process = process::Process::get_current();
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

unsafe extern "system" fn close_handle(_handle: *mut c_void) -> i32 {
    // TODO: temporary solution; leak all handles to ensure that they still exist
    // after loading a state
    1
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

#[expect(clippy::cast_possible_truncation)]
#[expect(clippy::cast_sign_loss)]
unsafe extern "system" fn get_key_state(id: i32) -> i16 {
    i16::from(STATE.lock().unwrap().get_key_state(id as u8)) << 15
}

unsafe extern "system" fn get_async_key_state(id: i32) -> i16 {
    unsafe { get_key_state(id) }
}

unsafe extern "system" fn sleep(milliseconds: u32) {
    state::sleep(u64::from(milliseconds) * State::TICKS_PER_SECOND / 1000);
}

unsafe extern "system" fn register_class_ex_a(information: *const WNDCLASSEXA) -> u16 {
    unsafe { register_class_ex(information, false) }
}

unsafe extern "system" fn register_class_ex_w(information: *const WNDCLASSEXW) -> u16 {
    unsafe { register_class_ex(information.cast::<WNDCLASSEXA>(), true) }
}

// note: WNDCLASSEXA and WNDCLASSEXW are nearly identical types, with the only
// difference being that two *const i8 fields in WNDCLASSEXA are instead *const
// u16 fields in WNDCLASSEXW
unsafe fn register_class_ex(information: *const WNDCLASSEXA, unicode_strings: bool) -> u16 {
    let mut new_information = unsafe { *information };
    new_information.lpfnWndProc = new_information
        .lpfnWndProc
        .map(|original_window_procedure| {
            // a wrapper which prepends the address of the trampoline as the first argument
            #[cfg(target_pointer_width = "64")]
            let hook_wrapper = {
                let mut function = vec![
                    // 0xeb, 0xfe,
                    0x41, 0x51, // push r9
                    0x48, 0x83, 0xec, 0x20, // sub rsp, 0x20
                    0x4d, 0x89, 0xc1, // mov r9, r8
                    0x49, 0x89, 0xd0, // mov r8, rdx
                    0x48, 0x89, 0xca, // mov rdx, rcx
                    0x48, 0xb9, 0, 0, 0, 0, 0, 0, 0, 0, // mov rcx, original_window_procedure
                    0x48, 0xb8, 0, 0, 0, 0, 0, 0, 0, 0, // mov rax, window_procedure
                    0xff, 0xd0, // call rax
                    0x48, 0x83, 0xc4, 0x28, // add rsp, 0x28
                    0xc3,
                ];
                function[17..][..8]
                    .copy_from_slice(&(original_window_procedure as usize).to_le_bytes());
                function[27..][..8].copy_from_slice(&(window_procedure as usize).to_le_bytes());
                function
            };
            #[cfg(target_pointer_width = "32")]
            let hook_wrapper = {
                let mut function = vec![
                    0x58, // pop eax
                    0x68, 0, 0, 0, 0,    // push original_window_procedure
                    0x50, // push eax
                    0xb8, 0, 0, 0, 0, // mov eax, window_procedure
                    0xff, 0xe0, // jmp eax
                ];
                function[2..][..4]
                    .copy_from_slice(&(original_window_procedure as usize).to_le_bytes());
                function[8..][..4].copy_from_slice(&(window_procedure as usize).to_le_bytes());
                function
            };

            let current_process = process::Process::get_current();
            let hook_wrapper_pointer = current_process
                .allocate_memory(
                    hook_wrapper.len(),
                    process::MemoryPermissions {
                        rwe: process::MemoryPermissionsRwe::ReadExecute,
                        is_guard: false,
                    },
                )
                .unwrap();
            current_process
                .write(hook_wrapper_pointer, &hook_wrapper)
                .unwrap();

            unsafe { std::mem::transmute(hook_wrapper_pointer) }
        });

    if unicode_strings {
        let trampoline = get_trampoline!(
            RegisterClassExW,
            unsafe extern "system" fn(*const WNDCLASSEXW) -> u16
        );
        unsafe { trampoline(std::ptr::addr_of!(new_information).cast()) }
    } else {
        let trampoline = get_trampoline!(
            RegisterClassExA,
            unsafe extern "system" fn(*const WNDCLASSEXA) -> u16
        );
        unsafe { trampoline(&new_information) }
    }
}

unsafe extern "system" fn window_procedure(
    trampoline: unsafe extern "system" fn(HWND, u32, usize, isize) -> isize,
    window: HWND,
    message: u32,
    w_parameter: usize,
    l_parameter: isize,
) -> isize {
    if matches!(
        message,
        WM_SETFOCUS | WM_KILLFOCUS | WM_ACTIVATE | WM_ACTIVATEAPP
    ) {
        0
    } else {
        unsafe { trampoline(window, message, w_parameter, l_parameter) }
    }
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

unsafe fn peek_message(
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
        if result != 0
            && matches!(
                (*message).message,
                WM_KEYDOWN
                    | WM_KEYUP
                    | WM_CHAR
                    | WM_MOUSEMOVE
                    | WM_LBUTTONDOWN
                    | WM_LBUTTONUP
                    | WM_RBUTTONDOWN
                    | WM_RBUTTONUP
                    | WM_MBUTTONDOWN
                    | WM_MBUTTONUP
                    | WM_XBUTTONDOWN
                    | WM_XBUTTONUP
            )
        {
            0
        } else {
            result
        }
    }
}

unsafe extern "system" fn get_message_a(
    message: *mut MSG,
    window_filter: HWND,
    minimum_id_filter: u32,
    maximum_id_filter: u32,
) -> i32 {
    unsafe {
        get_message(
            message,
            window_filter,
            minimum_id_filter,
            maximum_id_filter,
            false,
        )
    }
}

unsafe extern "system" fn get_message_w(
    message: *mut MSG,
    window_filter: HWND,
    minimum_id_filter: u32,
    maximum_id_filter: u32,
) -> i32 {
    unsafe {
        get_message(
            message,
            window_filter,
            minimum_id_filter,
            maximum_id_filter,
            true,
        )
    }
}

unsafe fn get_message(
    message: *mut MSG,
    window_filter: HWND,
    minimum_id_filter: u32,
    maximum_id_filter: u32,
    unicode_strings: bool,
) -> i32 {
    let peek_message = if unicode_strings {
        PeekMessageW
    } else {
        PeekMessageA
    };

    loop {
        unsafe {
            if peek_message(
                message,
                window_filter,
                minimum_id_filter,
                maximum_id_filter,
                PM_REMOVE,
            ) != 0
            {
                if (*message).message == WM_QUIT {
                    return 0;
                }
                return 1;
            }
        }

        state::sleep_indefinitely();
    }
}

#[expect(clippy::cast_possible_truncation)]
extern "system" fn get_tick_count() -> u32 {
    (state::get_ticks_with_busy_wait() * 1000 / State::TICKS_PER_SECOND) as u32
}

#[expect(clippy::cast_possible_truncation)]
extern "system" fn get_tick_count_64() -> u64 {
    (u128::from(state::get_ticks_with_busy_wait()) * 1000 / u128::from(State::TICKS_PER_SECOND))
        as u64
}

extern "system" fn time_get_time() -> u32 {
    get_tick_count()
}

const SIMULATED_PERFORMANCE_COUNTER_FREQUENCY: u64 = 1 << 32;

unsafe extern "system" fn query_performance_frequency(frequency: *mut LARGE_INTEGER) -> i32 {
    #[expect(clippy::cast_possible_wrap)]
    unsafe {
        *(*frequency).QuadPart_mut() = SIMULATED_PERFORMANCE_COUNTER_FREQUENCY as i64;
    }

    1
}

unsafe extern "system" fn query_performance_counter(count: *mut LARGE_INTEGER) -> i32 {
    #[expect(clippy::cast_possible_wrap)]
    unsafe {
        let simulated_performance_counter = state::get_ticks_with_busy_wait()
            * SIMULATED_PERFORMANCE_COUNTER_FREQUENCY
            / State::TICKS_PER_SECOND;
        *(*count).QuadPart_mut() = simulated_performance_counter as i64;
    }

    1
}

unsafe extern "system" fn get_system_time_as_file_time(file_time: *mut FILETIME) {
    #[expect(clippy::cast_possible_truncation)]
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

unsafe extern "system" fn create_waitable_timer_a(
    security_attributes: *mut SECURITY_ATTRIBUTES,
    manual_reset: i32,
    timer_name: *const i8,
) -> *mut c_void {
    unsafe {
        create_waitable_timer_ex_a(
            security_attributes,
            timer_name.cast(),
            if manual_reset == 1 {
                CREATE_WAITABLE_TIMER_MANUAL_RESET
            } else {
                0
            },
            TIMER_ALL_ACCESS,
        )
    }
}

unsafe extern "system" fn create_waitable_timer_w(
    security_attributes: *mut SECURITY_ATTRIBUTES,
    manual_reset: i32,
    timer_name: *const u16,
) -> *mut c_void {
    unsafe {
        create_waitable_timer_ex_w(
            security_attributes,
            timer_name.cast(),
            if manual_reset == 1 {
                CREATE_WAITABLE_TIMER_MANUAL_RESET
            } else {
                0
            },
            TIMER_ALL_ACCESS,
        )
    }
}

unsafe extern "system" fn create_waitable_timer_ex_a(
    security_attributes: *mut SECURITY_ATTRIBUTES,
    timer_name: *const i8,
    flags: u32,
    desired_access: u32,
) -> *mut c_void {
    unsafe {
        create_waitable_timer_ex(
            security_attributes,
            timer_name.cast(),
            flags,
            desired_access,
            false,
        )
    }
}

unsafe extern "system" fn create_waitable_timer_ex_w(
    security_attributes: *mut SECURITY_ATTRIBUTES,
    timer_name: *const u16,
    flags: u32,
    desired_access: u32,
) -> *mut c_void {
    unsafe {
        create_waitable_timer_ex(
            security_attributes,
            timer_name.cast(),
            flags,
            desired_access,
            true,
        )
    }
}

unsafe fn create_waitable_timer_ex(
    security_attributes: *mut SECURITY_ATTRIBUTES,
    timer_name: *const c_void,
    flags: u32,
    desired_access: u32,
    unicode_strings: bool,
) -> *mut c_void {
    let result = if unicode_strings {
        let trampoline = get_trampoline!(
            CreateWaitableTimerExW,
            unsafe extern "system" fn(
                *mut SECURITY_ATTRIBUTES,
                *const u16,
                u32,
                u32,
            ) -> *mut c_void
        );
        unsafe {
            trampoline(
                security_attributes,
                timer_name.cast(),
                flags,
                desired_access,
            )
        }
    } else {
        let trampoline = get_trampoline!(
            CreateWaitableTimerExA,
            unsafe extern "system" fn(*mut SECURITY_ATTRIBUTES, *const i8, u32, u32) -> *mut c_void
        );
        unsafe {
            trampoline(
                security_attributes,
                timer_name.cast(),
                flags,
                desired_access,
            )
        }
    };
    if !result.is_null() {
        STATE.lock().unwrap().waitable_timer_handles.insert(
            result as u32,
            Arc::new(Mutex::new(WaitableTimer {
                reset_automatically: flags != CREATE_WAITABLE_TIMER_MANUAL_RESET,
                signaled: false,
                remaining_ticks: 0,
                period_in_ticks: None,
            })),
        );
    }
    result
}

unsafe extern "system" fn set_waitable_timer(
    timer: *mut c_void,
    due_time: *const LARGE_INTEGER,
    period: i32,
    completion_routine: Option<unsafe extern "system" fn(*mut c_void, u32, u32)>,
    completion_routine_argument: *mut c_void,
    resume: i32,
) -> i32 {
    let trampoline = get_trampoline!(
        SetWaitableTimer,
        unsafe extern "system" fn(
            *mut c_void,
            *const LARGE_INTEGER,
            i32,
            Option<unsafe extern "system" fn(*mut c_void, u32, u32)>,
            *mut c_void,
            i32,
        ) -> i32
    );
    let result = unsafe {
        trampoline(
            timer,
            due_time,
            period,
            completion_routine,
            completion_routine_argument,
            resume,
        )
    };
    if result != 0 {
        set_waitable_timer_shared(timer, due_time, period);
    }
    result
}

unsafe extern "system" fn set_waitable_timer_ex(
    timer: *mut c_void,
    due_time: *const LARGE_INTEGER,
    period: i32,
    completion_routine: Option<unsafe extern "system" fn(*mut c_void, u32, u32)>,
    completion_routine_argument: *mut c_void,
    wake_context: *mut REASON_CONTEXT,
    tolerable_delay: u32,
) -> i32 {
    let trampoline = get_trampoline!(
        SetWaitableTimerEx,
        unsafe extern "system" fn(
            *mut c_void,
            *const LARGE_INTEGER,
            i32,
            Option<unsafe extern "system" fn(*mut c_void, u32, u32)>,
            *mut c_void,
            *mut REASON_CONTEXT,
            u32,
        ) -> i32
    );
    let result = unsafe {
        trampoline(
            timer,
            due_time,
            period,
            completion_routine,
            completion_routine_argument,
            wake_context,
            tolerable_delay,
        )
    };
    if result != 0 {
        set_waitable_timer_shared(timer, due_time, period);
    }
    result
}

#[expect(clippy::cast_sign_loss)]
fn set_waitable_timer_shared(timer: *mut c_void, due_time: *const LARGE_INTEGER, period: i32) {
    let state = STATE.lock().unwrap();
    let Some(waitable_timer) = state.waitable_timer_handles.get(&(timer as u32)) else {
        return;
    };
    let mut waitable_timer = waitable_timer.lock().unwrap();
    waitable_timer.signaled = false;
    waitable_timer.period_in_ticks =
        NonZeroU64::new(period as u64 * State::TICKS_PER_SECOND / 1000);

    let due_time = unsafe { *(*due_time).QuadPart() };
    waitable_timer.remaining_ticks = if due_time >= 0 {
        due_time as u64 * State::TICKS_PER_SECOND / 10_000_000 - state.ticks()
    } else {
        -due_time as u64 * State::TICKS_PER_SECOND / 10_000_000
    };
}

unsafe extern "system" fn wait_for_single_object(
    object: *mut c_void,
    timeout_in_milliseconds: u32,
) -> u32 {
    let waitable_timer = STATE
        .lock()
        .unwrap()
        .waitable_timer_handles
        .get(&(object as u32))
        .map(Arc::clone);
    if let Some(waitable_timer) = waitable_timer {
        let sleep_time;
        {
            let waitable_timer = waitable_timer.lock().unwrap();
            let timeout_in_ticks =
                u64::from(timeout_in_milliseconds) * State::TICKS_PER_SECOND / 1000;
            if waitable_timer.signaled {
                sleep_time = 0;
            } else if waitable_timer.running() {
                sleep_time = timeout_in_ticks.min(waitable_timer.remaining_ticks);
            } else {
                sleep_time = timeout_in_ticks;
            }
        }
        state::sleep(sleep_time);
        let mut waitable_timer = waitable_timer.lock().unwrap();
        if waitable_timer.signaled {
            if waitable_timer.reset_automatically {
                waitable_timer.signaled = false;
            }
            WAIT_OBJECT_0
        } else {
            WAIT_TIMEOUT
        }
    } else {
        let trampoline = get_trampoline!(
            WaitForSingleObject,
            unsafe extern "system" fn(*mut c_void, u32) -> u32
        );
        unsafe { trampoline(object, timeout_in_milliseconds) }
    }
}

unsafe extern "system" fn socket_(_address_family: i32, _type: i32, _protocol: i32) -> usize {
    INVALID_SOCKET
}

unsafe extern "system" fn nt_set_information_thread(
    thread: HANDLE,
    information_class: THREADINFOCLASS,
    information: *mut c_void,
    information_length: u32,
) -> i32 {
    if information_class == ThreadHideFromDebugger {
        STATUS_SUCCESS
    } else {
        let trampoline = get_trampoline!(
            NtSetInformationThread,
            unsafe extern "system" fn(HANDLE, THREADINFOCLASS, *mut c_void, u32) -> i32
        );
        unsafe { trampoline(thread, information_class, information, information_length) }
    }
}
