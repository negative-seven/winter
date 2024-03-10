use anyhow::Result;
use std::env;
use tracing::info;
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

    let mut runtime = Runtime::new(
        executable_path,
        injected_dll_path,
        Some(|bytes: &_| {
            for line in String::from_utf8_lossy(bytes).lines() {
                info!("stdout: {}", line);
            }
        }),
    )?;
    runtime.resume()?;
    runtime.wait_until_exit()?;

    Ok(())
}
