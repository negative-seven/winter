use std::mem::MaybeUninit;

use winapi::um::sysinfoapi::{GetNativeSystemInfo, SYSTEM_INFO};

#[must_use]
pub fn get_info() -> SYSTEM_INFO {
    unsafe {
        let mut system_info = MaybeUninit::zeroed().assume_init();
        GetNativeSystemInfo(&mut system_info);
        system_info
    }
}
