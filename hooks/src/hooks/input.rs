use super::common::hook;
use crate::state::STATE;
use winapi::{
    ctypes::c_void,
    um::winuser::{GetAsyncKeyState, GetKeyState, GetKeyboardState},
};

pub(crate) const HOOKS: &[(&str, &str, *const c_void)] = &[
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
];

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
