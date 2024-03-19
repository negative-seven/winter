use anyhow::Result;
use std::{env, time::Duration};
use tracing::info;
use winter::Runtime;

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let mut arguments = env::args().skip(1);
    let executable_path = arguments.next().expect("missing argument: executable path");
    let movie_path = arguments.next();

    let mut runtime = Runtime::new(
        executable_path,
        Some(|bytes: &_| {
            for line in String::from_utf8_lossy(bytes).lines() {
                info!("stdout: {}", line);
            }
        }),
    )?;
    runtime.resume()?;
    if let Some(movie_path) = movie_path {
        for line in std::fs::read_to_string(movie_path)?.lines() {
            let mut tokens = line.split_ascii_whitespace();
            match tokens.next().unwrap() {
                "Key" => {
                    let key_id = tokens.next().unwrap().parse::<u8>()?;
                    let key_state = tokens.next().unwrap().parse::<u8>()? != 0;
                    runtime.set_key_state(key_id, key_state)?;
                }
                "Wait" => {
                    let secs = tokens.next().unwrap().parse::<f64>()?;
                    runtime.advance_time(Duration::from_secs_f64(secs))?;
                    // std::thread::sleep(Duration::from_secs_f64(0.1));
                    runtime.wait_until_idle()?;
                }
                _ => unimplemented!(),
            }
        }
    } else {
        loop {
            runtime.advance_time(Duration::from_secs_f64(1.0 / 60.0))?;
            runtime.wait_until_idle()?;
        }
    }

    Ok(())
}
