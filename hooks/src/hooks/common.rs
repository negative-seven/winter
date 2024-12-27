use minhook::MinHook;
use shared::windows::process;
use std::{collections::BTreeMap, sync::RwLock};
use winapi::ctypes::c_void;

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

pub(crate) fn initialize() {
    let hooks = [
        super::input::HOOKS,
        super::time::HOOKS,
        super::window::HOOKS,
        super::misc::HOOKS,
    ]
    .concat();

    for (module_name, function_name, hook) in hooks {
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
        let _unused_result = hook_function(module_name, function_name, hook);
    }
}
