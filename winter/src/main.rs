use anyhow::Result;
use std::{env, io::Read};
use winter::Runtime;

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let mut arguments = env::args().skip(1);
    let executable_path = arguments.next().expect("missing argument: executable path");
    let injected_dll_path = arguments
        .next()
        .expect("missing argument: injected dll path");

    let mut runtime = Runtime::new(executable_path, injected_dll_path)?;
    runtime.resume()?;
    runtime.wait_until_exit()?;

    let mut stdout = String::new();
    runtime.stdout_mut().read_to_string(&mut stdout)?;
    println!("{stdout}");

    Ok(())
}
