use anyhow::Result;
use clap::Parser;
use std::time::{Duration, Instant};
use tokio::time::sleep;
use tracing::info;
use winter::Conductor;

#[derive(clap::Parser)]
struct Arguments {
    executable_path: String,
    movie_path: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let arguments = Arguments::parse();

    let mut conductor = Conductor::new(
        arguments.executable_path,
        Some(|bytes: &_| {
            for line in String::from_utf8_lossy(bytes).lines() {
                info!("stdout: {}", line);
            }
        }),
    )
    .await?;
    conductor.resume().await?;

    let mut sleep_target = Instant::now();
    if let Some(movie_path) = arguments.movie_path {
        for line in std::fs::read_to_string(movie_path)?.lines() {
            let mut tokens = line.split_ascii_whitespace();
            match tokens.next().unwrap().to_lowercase().as_str() {
                "key" => {
                    let key_id = tokens.next().unwrap().parse::<u8>()?;
                    let key_state = tokens.next().unwrap().parse::<u8>()? != 0;
                    conductor.set_key_state(key_id, key_state).await?;
                }
                "wait" => {
                    wait(
                        &mut conductor,
                        Duration::from_secs_f64(tokens.next().unwrap().parse::<f64>()?),
                        &mut sleep_target,
                    )
                    .await?;
                }
                _ => unimplemented!(),
            }
        }
    }

    loop {
        wait(
            &mut conductor,
            Duration::from_secs_f64(1.0 / 60.0),
            &mut sleep_target,
        )
        .await?;
    }
}

async fn wait(
    conductor: &mut Conductor,
    duration: Duration,
    sleep_target: &mut Instant,
) -> Result<()> {
    let now = Instant::now();
    conductor.advance_time(duration).await?;
    *sleep_target += duration;
    *sleep_target = (*sleep_target).max(now.checked_sub(duration * 4).unwrap_or(now));
    sleep(*sleep_target - now).await;
    conductor.wait_until_inactive().await?;
    Ok(())
}
