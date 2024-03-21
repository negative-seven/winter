use anyhow::Result;
use std::{env, time::Duration};
use tracing::info;
use winter::Conductor;

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let mut arguments = env::args().skip(1);
    let executable_path = arguments.next().expect("missing argument: executable path");
    let movie_path = arguments.next();

    let mut conductor = Conductor::new(
        executable_path,
        Some(|bytes: &_| {
            for line in String::from_utf8_lossy(bytes).lines() {
                info!("stdout: {}", line);
            }
        }),
    )?;
    conductor.resume()?;
    if let Some(movie_path) = movie_path {
        for line in std::fs::read_to_string(movie_path)?.lines() {
            let mut tokens = line.split_ascii_whitespace();
            match tokens.next().unwrap() {
                "Key" => {
                    let key_id = tokens.next().unwrap().parse::<u8>()?;
                    let key_state = tokens.next().unwrap().parse::<u8>()? != 0;
                    conductor.set_key_state(key_id, key_state)?;
                }
                "Wait" => {
                    let secs = tokens.next().unwrap().parse::<f64>()?;
                    conductor.advance_time(Duration::from_secs_f64(secs))?;
                    conductor.wait_until_idle()?;
                }
                _ => unimplemented!(),
            }
        }
    } else {
        loop {
            conductor.advance_time(Duration::from_secs_f64(1.0 / 60.0))?;
            conductor.wait_until_idle()?;
        }
    }

    Ok(())
}
