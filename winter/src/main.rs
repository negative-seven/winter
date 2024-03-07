use anyhow::Result;
use std::env;
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

    let runtime = Runtime::new(executable_path, injected_dll_path)?;
    runtime.resume()?;

    Ok(())
}
