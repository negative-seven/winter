use crate::{
    get_trampoline,
    state::{self, State, STATE},
};
use winapi::um::winuser::{MSG, WM_CHAR, WM_KEYDOWN, WM_KEYUP};

pub extern "system" fn get_keyboard_state(key_states: *mut u8) {
    let state = STATE.lock().unwrap();
    for i in 0u8..=255u8 {
        unsafe {
            *(key_states.offset(isize::from(i))) = u8::from(state.key_states[usize::from(i)]) << 7;
        }
    }
}

pub extern "system" fn get_key_state(id: u32) -> u16 {
    u16::from(STATE.lock().unwrap().key_states[id as usize]) << 15
}

pub extern "system" fn get_async_key_state(id: u32) -> u16 {
    get_key_state(id)
}

pub extern "system" fn sleep(milliseconds: u32) {
    state::sleep(u64::from(milliseconds) * State::TICKS_PER_SECOND / 1000);
    unsafe {
        let trampoline: extern "system" fn(u32) = std::mem::transmute(get_trampoline("Sleep"));
        trampoline(milliseconds);
    }
}

pub unsafe extern "system" fn peek_message(
    message: *mut MSG,
    arg1: u32,
    arg2: u32,
    arg3: u32,
    arg4: u32,
) -> u32 {
    if let Some(custom_message) = STATE.lock().unwrap().custom_message_queue.pop_front() {
        *message = custom_message.0;
        1
    } else {
        let trampoline: extern "system" fn(*mut MSG, u32, u32, u32, u32) -> u32 =
            std::mem::transmute(get_trampoline("PeekMessageA"));
        let result = trampoline(message, arg1, arg2, arg3, arg4);
        if result != 0 && matches!((*message).message, WM_KEYDOWN | WM_KEYUP | WM_CHAR) {
            (*message).message = 0;
        }
        result
    }
}

#[allow(clippy::cast_possible_truncation)]
pub extern "system" fn get_tick_count() -> u32 {
    let mut state_guard = STATE.lock().unwrap();

    state_guard.busy_wait_count += 1;
    if state_guard.busy_wait_count >= 100 {
        drop(state_guard);
        state::sleep(State::TICKS_PER_SECOND / 60);
        state_guard = STATE.lock().unwrap();
        state_guard.busy_wait_count = 0;
    }

    (state_guard.ticks * 1000 / State::TICKS_PER_SECOND) as u32
}

pub extern "system" fn time_get_time() -> u32 {
    get_tick_count()
}

const SIMULATED_PERFORMANCE_COUNTER_FREQUENCY: u64 = 1 << 32;

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

    let mut state_guard = STATE.lock().unwrap();

    state_guard.busy_wait_count += 1;
    if state_guard.busy_wait_count >= 100 {
        drop(state_guard);
        state::sleep(State::TICKS_PER_SECOND / 60);
        state_guard = STATE.lock().unwrap();
        state_guard.busy_wait_count = 0;
    }

    #[allow(clippy::cast_possible_truncation)]
    unsafe {
        let simulated_performance_counter =
            state_guard.ticks * SIMULATED_PERFORMANCE_COUNTER_FREQUENCY / State::TICKS_PER_SECOND;
        *count = simulated_performance_counter as u32;
        *count.offset(1) = (simulated_performance_counter >> 32) as u32;
    }

    1
}
