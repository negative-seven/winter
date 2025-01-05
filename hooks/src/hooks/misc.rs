use crate::state::{self, State, STATE};
use hooks_macros::{hook, hooks};
use ntapi::ntpsapi::{NtSetInformationThread, ThreadHideFromDebugger, THREADINFOCLASS};
use std::sync::Arc;
use winapi::{
    ctypes::c_void,
    shared::{ntdef::HANDLE, ntstatus::STATUS_SUCCESS, winerror::WAIT_TIMEOUT},
    um::{
        handleapi::CloseHandle,
        synchapi::WaitForSingleObject,
        winbase::WAIT_OBJECT_0,
        winsock2::{socket, INVALID_SOCKET},
    },
};

pub(crate) const HOOKS: &[(&str, &str, *const c_void)] = &hooks![
    CloseHandle,
    WaitForSingleObject,
    socket,
    NtSetInformationThread
];

#[hook("kernel32.dll")]
unsafe extern "system" fn CloseHandle(_handle: *mut c_void) -> i32 {
    // TODO: temporary solution; leak all handles to ensure that they still exist
    // after loading a state
    1
}

#[hook("kernel32.dll")]
unsafe extern "system" fn WaitForSingleObject(
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
        unsafe { get_self_trampoline()(object, timeout_in_milliseconds) }
    }
}

#[hook("ws2_32.dll")]
unsafe extern "system" fn socket(_address_family: i32, _type: i32, _protocol: i32) -> usize {
    INVALID_SOCKET
}

#[hook("ntdll.dll")]
unsafe extern "system" fn NtSetInformationThread(
    thread: HANDLE,
    information_class: THREADINFOCLASS,
    information: *mut c_void,
    information_length: u32,
) -> i32 {
    if information_class == ThreadHideFromDebugger {
        STATUS_SUCCESS
    } else {
        unsafe { get_self_trampoline()(thread, information_class, information, information_length) }
    }
}
