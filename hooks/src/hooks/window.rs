use super::get_trampoline;
use crate::state::{self, STATE};
use hooks_macros::{hook, hooks};
use shared::windows::process;
use winapi::{
    ctypes::c_void,
    shared::{ntdef::NULL, windef::HWND},
    um::winuser::{
        GetMessageA, GetMessageW, PeekMessageA, PeekMessageW, RegisterClassExA, RegisterClassExW,
        MSG, PM_REMOVE, WM_ACTIVATE, WM_ACTIVATEAPP, WM_CHAR, WM_KEYDOWN, WM_KEYUP, WM_KILLFOCUS,
        WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDOWN, WM_MBUTTONUP, WM_MOUSEMOVE, WM_QUIT,
        WM_RBUTTONDOWN, WM_RBUTTONUP, WM_SETFOCUS, WM_XBUTTONDOWN, WM_XBUTTONUP, WNDCLASSEXA,
        WNDCLASSEXW,
    },
};

pub(crate) const HOOKS: &[(&str, &str, *const c_void)] = &hooks![
    RegisterClassExA,
    RegisterClassExW,
    PeekMessageA,
    PeekMessageW,
    GetMessageA,
    GetMessageW,
];

#[hook("user32.dll")]
unsafe extern "system" fn RegisterClassExA(information: *const WNDCLASSEXA) -> u16 {
    unsafe { register_class_ex(information, false) }
}

#[hook("user32.dll")]
unsafe extern "system" fn RegisterClassExW(information: *const WNDCLASSEXW) -> u16 {
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
            let hook_wrapper_address = current_process
                .allocate_memory(
                    hook_wrapper.len(),
                    process::MemoryPermissions {
                        rwe: process::MemoryPermissionsRwe::ReadExecute,
                        is_guard: false,
                    },
                )
                .unwrap()
                .cast();
            current_process
                .write(hook_wrapper_address, &hook_wrapper)
                .unwrap();

            unsafe { std::mem::transmute(hook_wrapper_address) }
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

#[hook("user32.dll")]
unsafe extern "system" fn PeekMessageA(
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

#[hook("user32.dll")]
unsafe extern "system" fn PeekMessageW(
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

#[hook("user32.dll")]
unsafe extern "system" fn GetMessageA(
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

#[hook("user32.dll")]
unsafe extern "system" fn GetMessageW(
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
