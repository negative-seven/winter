mod input;
mod library;
mod misc;
mod time;
mod window;

use crate::log;
use minhook::MinHook;
use shared::{
    ipc::message::LogLevel,
    windows::{module, process},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::{OsStr, OsString},
    sync::{LazyLock, Mutex, RwLock},
};
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

static HOOKS: LazyLock<BTreeMap<OsString, Vec<(&str, usize)>>> = LazyLock::new(|| {
    let mut map = BTreeMap::<_, Vec<_>>::new();
    for (module_name, function_name, hook) in [
        library::HOOKS,
        input::HOOKS,
        time::HOOKS,
        window::HOOKS,
        misc::HOOKS,
    ]
    .concat()
    {
        map.entry(OsString::from(module_name))
            .or_default()
            .push((function_name, hook as usize));
    }
    map
});

pub(crate) fn initialize() {
    let process = process::Process::get_current();
    for module in process.get_modules().unwrap() {
        apply_to_module(&module);
    }
}

static HOOKED_MODULE_ADDRESSES: Mutex<BTreeSet<usize>> = Mutex::new(BTreeSet::new());
pub(crate) fn apply_to_module(module: &module::Module) {
    if HOOKED_MODULE_ADDRESSES
        .lock()
        .unwrap()
        .contains(&(module.get_base_address() as usize))
    {
        return;
    }

    let module_name = module.get_name().unwrap().to_ascii_lowercase();
    log!(LogLevel::Debug, "applying hooks to {:?}", module_name);
    for &(function_name, hook) in HOOKS.get(&module_name).unwrap_or(&vec![]) {
        fn hook_function(
            module_name: &OsStr,
            function_name: &str,
            hook: *const c_void,
        ) -> Result<(), Box<dyn std::error::Error>> {
            let process = process::Process::get_current();
            let function_address = process
                .get_module(module_name)?
                .ok_or("module not found")?
                .get_export_address(function_name)?;
            unsafe {
                let original_function =
                    MinHook::create_hook(function_address, hook as *mut std::ffi::c_void).unwrap();
                MinHook::enable_hook(function_address).unwrap();
                set_trampoline(function_name, original_function.cast());
            }
            Ok(())
        }

        let result = hook_function(&module_name, function_name, hook as *const c_void);
        if let Err(error) = result {
            log!(
                LogLevel::Debug,
                "failed to hook: {function_name}; error: {error}"
            );
        }
    }

    HOOKED_MODULE_ADDRESSES
        .lock()
        .unwrap()
        .insert(module.get_base_address() as usize);
}
