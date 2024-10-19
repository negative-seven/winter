use super::common::get_trampoline;
use crate::state::{self, State, WaitableTimer, STATE};
use hooks_macros::{hook, hooks};
use std::{
    num::NonZeroU64,
    sync::{Arc, Mutex},
};
use winapi::{
    ctypes::c_void,
    shared::minwindef::FILETIME,
    um::{
        minwinbase::{REASON_CONTEXT, SECURITY_ATTRIBUTES},
        profileapi::{QueryPerformanceCounter, QueryPerformanceFrequency},
        synchapi::{
            CreateWaitableTimerExW, CreateWaitableTimerW, SetWaitableTimer, SetWaitableTimerEx,
            Sleep, CREATE_WAITABLE_TIMER_MANUAL_RESET,
        },
        sysinfoapi::{
            GetSystemTimeAsFileTime, GetSystemTimePreciseAsFileTime, GetTickCount, GetTickCount64,
        },
        timeapi::timeGetTime,
        winbase::{CreateWaitableTimerA, CreateWaitableTimerExA},
        winnt::{LARGE_INTEGER, TIMER_ALL_ACCESS},
    },
};

pub(crate) const HOOKS: &[(&str, &str, *const c_void)] = &hooks![
    Sleep,
    GetTickCount,
    GetTickCount64,
    timeGetTime,
    QueryPerformanceFrequency,
    QueryPerformanceCounter,
    GetSystemTimeAsFileTime,
    GetSystemTimePreciseAsFileTime,
    CreateWaitableTimerA,
    CreateWaitableTimerW,
    CreateWaitableTimerExA,
    CreateWaitableTimerExW,
    SetWaitableTimer,
    SetWaitableTimerEx,
];

#[hook("kernel32.dll")]
unsafe extern "system" fn Sleep(milliseconds: u32) {
    state::sleep(u64::from(milliseconds) * State::TICKS_PER_SECOND / 1000);
}

#[expect(clippy::cast_possible_truncation)]
#[hook("kernel32.dll")]
unsafe extern "system" fn GetTickCount() -> u32 {
    (state::get_ticks_with_busy_wait() * 1000 / State::TICKS_PER_SECOND) as u32
}

#[expect(clippy::cast_possible_truncation)]
#[hook("kernel32.dll")]
unsafe extern "system" fn GetTickCount64() -> u64 {
    (u128::from(state::get_ticks_with_busy_wait()) * 1000 / u128::from(State::TICKS_PER_SECOND))
        as u64
}

#[hook("winmm.dll")]
unsafe extern "system" fn timeGetTime() -> u32 {
    unsafe { GetTickCount() }
}

const SIMULATED_PERFORMANCE_COUNTER_FREQUENCY: u64 = 1 << 32;

#[hook("kernel32.dll")]
unsafe extern "system" fn QueryPerformanceFrequency(frequency: *mut LARGE_INTEGER) -> i32 {
    #[expect(clippy::cast_possible_wrap)]
    unsafe {
        *(*frequency).QuadPart_mut() = SIMULATED_PERFORMANCE_COUNTER_FREQUENCY as i64;
    }

    1
}

#[hook("kernel32.dll")]
unsafe extern "system" fn QueryPerformanceCounter(count: *mut LARGE_INTEGER) -> i32 {
    #[expect(clippy::cast_possible_wrap)]
    unsafe {
        let simulated_performance_counter = state::get_ticks_with_busy_wait()
            * SIMULATED_PERFORMANCE_COUNTER_FREQUENCY
            / State::TICKS_PER_SECOND;
        *(*count).QuadPart_mut() = simulated_performance_counter as i64;
    }

    1
}

#[hook("kernel32.dll")]
unsafe extern "system" fn GetSystemTimeAsFileTime(file_time: *mut FILETIME) {
    #[expect(clippy::cast_possible_truncation)]
    let one_hundred_nanosecond_intervals = (u128::from(state::get_ticks_with_busy_wait())
        * 10_000_000
        / u128::from(State::TICKS_PER_SECOND)) as u64;

    unsafe {
        (*file_time).dwLowDateTime = (one_hundred_nanosecond_intervals & ((1 << 32) - 1)) as u32;
        (*file_time).dwHighDateTime = (one_hundred_nanosecond_intervals >> 32) as u32;
    }
}

#[hook("kernel32.dll")]
unsafe extern "system" fn GetSystemTimePreciseAsFileTime(file_time: *mut FILETIME) {
    unsafe { GetSystemTimeAsFileTime(file_time) }
}

#[hook("kernel32.dll")]
unsafe extern "system" fn CreateWaitableTimerA(
    security_attributes: *mut SECURITY_ATTRIBUTES,
    manual_reset: i32,
    timer_name: *const i8,
) -> *mut c_void {
    unsafe {
        CreateWaitableTimerExA(
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

#[hook("kernel32.dll")]
unsafe extern "system" fn CreateWaitableTimerW(
    security_attributes: *mut SECURITY_ATTRIBUTES,
    manual_reset: i32,
    timer_name: *const u16,
) -> *mut c_void {
    unsafe {
        CreateWaitableTimerExW(
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

#[hook("kernel32.dll")]
unsafe extern "system" fn CreateWaitableTimerExA(
    security_attributes: *mut SECURITY_ATTRIBUTES,
    timer_name: *const i8,
    flags: u32,
    desired_access: u32,
) -> *mut c_void {
    unsafe {
        create_waitable_timer(
            security_attributes,
            timer_name.cast(),
            flags,
            desired_access,
            false,
        )
    }
}

#[hook("kernel32.dll")]
unsafe extern "system" fn CreateWaitableTimerExW(
    security_attributes: *mut SECURITY_ATTRIBUTES,
    timer_name: *const u16,
    flags: u32,
    desired_access: u32,
) -> *mut c_void {
    unsafe {
        create_waitable_timer(
            security_attributes,
            timer_name.cast(),
            flags,
            desired_access,
            true,
        )
    }
}

unsafe fn create_waitable_timer(
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

#[hook("kernel32.dll")]
unsafe extern "system" fn SetWaitableTimer(
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

#[hook("kernelbase.dll")]
unsafe extern "system" fn SetWaitableTimerEx(
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
