use crate::{get_trampoline, BUSY_WAIT_COUNT, SIMULATED_PERFORMANCE_COUNTER_FREQUENCY, TICKS, TICKS_PER_SECOND};
use winapi::um::winuser::{MSG, WM_CHAR, WM_KEYDOWN, WM_KEYUP};

pub extern "system" fn get_keyboard_state(key_states: *mut bool) {
    for i in 0..256 {
        unsafe {
            *(key_states.offset(i)) = false;
        }
    }
}

pub extern "system" fn get_key_state(_: u32) -> u16 {
    0
}

pub extern "system" fn get_async_key_state(_: u32) -> u16 {
    0
}

pub extern "system" fn sleep(milliseconds: u32) {
    *TICKS.write() += u64::from(milliseconds) * TICKS_PER_SECOND / 1000;

    unsafe {
        let trampoline: extern "system" fn(u32) =
            std::mem::transmute(get_trampoline("Sleep"));
        trampoline(milliseconds);
    }
}

pub extern "system" fn peek_message(
    message_pointer: *mut MSG,
    arg1: u32,
    arg2: u32,
    arg3: u32,
    arg4: u32,
) -> u32 {
    unsafe {
        let trampoline: extern "system" fn(*mut MSG, u32, u32, u32, u32) -> u32 =
            std::mem::transmute(get_trampoline("PeekMessageA"));
    
        let result = trampoline(message_pointer, arg1, arg2, arg3, arg4);
        if result != 0 && matches!((*message_pointer).message, WM_KEYDOWN | WM_KEYUP | WM_CHAR) {
            (*message_pointer).message = 0;
        }
        result
    }
}

pub extern "system" fn query_performance_frequency(frequency: *mut u32) -> u32 {
    // due to pointer alignment issues, frequency must be split into two u32 chunks

    #[allow(clippy::cast_possible_truncation)]
    unsafe {
        *frequency = SIMULATED_PERFORMANCE_COUNTER_FREQUENCY as u32;
        *frequency.offset(1) = (SIMULATED_PERFORMANCE_COUNTER_FREQUENCY >> 32) as u32;
    }

    1
}

pub extern "system" fn query_performance_counter(count: *mut u32) -> u32 {
    // due to pointer alignment issues, count must be split into two u32 chunks

    let mut busy_wait_count = BUSY_WAIT_COUNT.write();
    *busy_wait_count += 1;
    if *busy_wait_count >= 100 {
        *TICKS.write() += TICKS_PER_SECOND / 60;
        *busy_wait_count = 0;
    }

    #[allow(clippy::cast_possible_truncation)]
    unsafe {
        let simulated_performance_counter =
            *TICKS.read() * SIMULATED_PERFORMANCE_COUNTER_FREQUENCY / TICKS_PER_SECOND;
        *count = simulated_performance_counter as u32;
        *count.offset(1) = (simulated_performance_counter >> 32) as u32;
    }

    1
}
