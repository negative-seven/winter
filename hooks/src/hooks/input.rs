use crate::state::STATE;
use hooks_macros::{hook, hooks};
use winapi::{
    ctypes::c_void,
    um::winuser::{GetAsyncKeyState, GetKeyState, GetKeyboardState},
};

pub(crate) const HOOKS: &[(&str, &str, *const c_void)] =
    &hooks![GetKeyboardState, GetKeyState, GetAsyncKeyState];

#[hook("user32.dll")]
unsafe extern "system" fn GetKeyboardState(key_states: *mut u8) -> i32 {
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
#[hook("user32.dll")]
unsafe extern "system" fn GetKeyState(id: i32) -> i16 {
    i16::from(STATE.lock().unwrap().get_key_state(id as u8)) << 15
}

#[hook("user32.dll")]
unsafe extern "system" fn GetAsyncKeyState(id: i32) -> i16 {
    unsafe { GetKeyState(id) }
}
