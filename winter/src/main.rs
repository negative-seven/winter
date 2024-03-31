use anyhow::Result;
use std::{
    env,
    thread::sleep,
    time::{Duration, Instant},
};
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

    let mut wait = {
        let mut sleep_target = Instant::now();
        move |conductor: &mut Conductor, duration| -> Result<(), anyhow::Error> {
            let now = Instant::now();
            conductor.advance_time(duration)?;
            sleep_target += duration;
            sleep_target = sleep_target.max(now.checked_sub(duration * 4).unwrap_or(now));
            sleep(sleep_target - now);
            conductor.wait_until_idle()?;
            Ok(())
        }
    };
    if let Some(movie_path) = movie_path {
        for line in std::fs::read_to_string(movie_path)?.lines() {
            let mut tokens = line.split_ascii_whitespace();
            match tokens.next().unwrap().to_lowercase().as_str() {
                "key" => {
                    let key_id = tokens.next().unwrap().parse::<u8>()?;
                    let key_state = tokens.next().unwrap().parse::<u8>()? != 0;
                    conductor.set_key_state(key_id, key_state)?;
                }
                "wait" => {
                    wait(
                        &mut conductor,
                        Duration::from_secs_f64(tokens.next().unwrap().parse::<f64>()?),
                    )?;
                }
                _ => unimplemented!(),
            }
        }
    } else {
        loop {
            wait(&mut conductor, Duration::from_secs_f64(1.0 / 60.0))?;
        }
    }

    Ok(())
}
