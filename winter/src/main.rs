use anyhow::Result;
use clap::Parser;
use std::{
    ffi::OsString,
    path::PathBuf,
    time::{Duration, Instant},
};
use tokio::time::sleep;
use tracing::info;
use winter::Conductor;

#[derive(clap::Parser)]
struct Arguments {
    /// The executable to be run under Winter.
    #[arg(name("executable"))]
    executable_path: PathBuf,

    /// The command-line string to be passed to the spawned child process.
    ///
    /// The command-line string of a process is a single UTF-16 string passed to
    /// the process upon creation. In many programs it is split into an array of
    /// arguments by means of or similarly to the CommandLineToArgvW Windows API
    /// call. Refer to the documentation for CommandLineToArgvW for more details
    /// on typical parsing of arguments. Note that in such cases, providing this
    /// argument may necessitate wrapping the command-line string in quotes and
    /// escaping any inner quotes, potentially adding a layer on top of escape
    /// sequences already present in the string.
    ///
    /// If this argument is given, it will not be automatically prepended with
    /// the program name before being passed to the child process. For programs
    /// that expect this behavior, the program name must be provided explicitly.
    ///
    /// If this argument is omitted, the command-line string will default to the
    /// executable path wrapped in quotes.
    #[arg(name("command_line_string"), short('a'), long)]
    #[allow(clippy::struct_field_names)]
    command_line_string: Option<OsString>,

    /// An optional path to a movie file to be played.
    #[arg(name("movie"), short, long)]
    movie_path: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let arguments = Arguments::parse();
    let mut conductor = Conductor::new(
        &arguments.executable_path,
        arguments.command_line_string.unwrap_or_else(|| {
            let executable_path = arguments.executable_path.as_os_str();
            let mut string = OsString::with_capacity(executable_path.len() + 2);
            string.push("\"");
            string.push(executable_path);
            string.push("\"");
            string
        }),
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
