use std::env;

use anyhow::Result;
use windows::{Process, Thread};

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let mut arguments = env::args().skip(1);
    let executable_path = arguments.next().expect("missing argument: executable path");
    let injected_dll_path = arguments
        .next()
        .expect("missing argument: injected dll path");

    let process = Process::create(&executable_path, true)?;
    process.inject_dll(&injected_dll_path)?;

    let initialize_function = process.get_export_address("hooks.dll", "initialize")?;
    process.create_thread(initialize_function, true, None)?;

    for thread in process
        .iter_thread_ids()?
        .map(Thread::from_id)
        .collect::<Result<Vec<_>, _>>()?
    {
        thread.resume()?;
    }

    Ok(())
}
