use anyhow::Result;
use std::{
    path::Path,
    process::Command,
    sync::{Arc, Mutex, Once},
    time::Duration,
};
use tracing::info;

#[allow(clippy::missing_panics_doc)]
pub fn init_test() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
            .init();

        assert!(
            std::env::var("VCVARS_DIR").map_or(false, |vcvars_dir| Path::exists(
                &Path::new(&vcvars_dir).join("vcvars32.bat")
            )),
            "the environment variable VCVARS_DIR must be set to a directory containing vcvars scripts"
        );

        assert!(Command::new("tests/programs/build.bat")
            .spawn()
            .unwrap()
            .wait()
            .unwrap()
            .success());
    });
}

fn run_and_get_stdout(
    executable_path: &str,
    advance_time_periods: Vec<Duration>,
) -> Result<Vec<Vec<u8>>> {
    let stdout = Arc::new(Mutex::new(Vec::new()));
    let stdout_callback = {
        let stdout = Arc::clone(&stdout);
        move |bytes: &_| {
            for line in String::from_utf8_lossy(bytes).lines() {
                info!("stdout: {}", line);
            }

            stdout.lock().unwrap().extend_from_slice(bytes);
        }
    };
    let mut stdout_by_instant = Vec::new();
    let mut runtime = winter::Runtime::new(executable_path, "hooks32.dll", Some(stdout_callback))?;
    runtime.resume()?;
    {
        runtime.wait_until_idle();
        let mut stdout = stdout.lock().unwrap();
        stdout_by_instant.push(std::mem::take(&mut *stdout));
    }
    for advance_time_period in advance_time_periods {
        runtime.advance_time(advance_time_period)?;
        runtime.wait_until_idle();
        let mut stdout = stdout.lock().unwrap();
        stdout_by_instant.push(std::mem::take(&mut *stdout));
    }
    runtime.wait_until_exit()?; // TODO: check that process only exited after the last time advancement
    Ok(stdout_by_instant)
}

#[test]
fn stdout() -> Result<()> {
    init_test();
    let stdout = run_and_get_stdout("tests/programs/bin/stdout.exe", Vec::new())?;
    assert_eq!(stdout, vec![b"abcABC123!\"_\x99\xaa\xbb"]);
    Ok(())
}

#[test]
fn get_tick_count() -> Result<()> {
    init_test();
    let stdout = run_and_get_stdout(
        "tests/programs/bin/get_tick_count.exe",
        vec![
            Duration::from_secs_f64(1.0 / 60.0),
            Duration::from_secs_f64(1.0 / 60.0),
        ],
    )?
    .iter()
    .map(|b| String::from_utf8_lossy(b).to_string())
    .collect::<Vec<_>>();
    assert_eq!(
        stdout,
        vec![
            "0\r\n".repeat(99),
            "16\r\n".repeat(100),
            "33\r\n".to_string()
        ]
    );
    Ok(())
}

#[test]
fn get_tick_count_and_sleep() -> Result<()> {
    init_test();
    let stdout = run_and_get_stdout(
        "tests/programs/bin/get_tick_count_and_sleep.exe",
        [Duration::from_millis(78), Duration::from_millis(1)].repeat(10),
    )?
    .iter()
    .map(|b| String::from_utf8_lossy(b).to_string())
    .collect::<Vec<_>>();

    let mut expected_stdout = Vec::new();
    for index in 0..10 {
        expected_stdout.push(format!("{}\r\n", index * 79));
        expected_stdout.push(String::new());
    }
    expected_stdout.push(String::new());
    assert_eq!(stdout, expected_stdout);
    Ok(())
}

#[test]
fn time_get_time() -> Result<()> {
    init_test();
    let stdout = run_and_get_stdout(
        "tests/programs/bin/time_get_time.exe",
        vec![
            Duration::from_secs_f64(1.0 / 60.0),
            Duration::from_secs_f64(1.0 / 60.0),
        ],
    )?
    .iter()
    .map(|b| String::from_utf8_lossy(b).to_string())
    .collect::<Vec<_>>();
    assert_eq!(
        stdout,
        vec![
            "0\r\n".repeat(99),
            "16\r\n".repeat(100),
            "33\r\n".to_string()
        ]
    );
    Ok(())
}

#[test]
fn time_get_time_and_sleep() -> Result<()> {
    init_test();
    let stdout = run_and_get_stdout(
        "tests/programs/bin/time_get_time_and_sleep.exe",
        [Duration::from_millis(40), Duration::from_millis(1)].repeat(10),
    )?
    .iter()
    .map(|b| String::from_utf8_lossy(b).to_string())
    .collect::<Vec<_>>();

    let mut expected_stdout = Vec::new();
    for index in 0..10 {
        expected_stdout.push(format!("{}\r\n", index * 41));
        expected_stdout.push(String::new());
    }
    expected_stdout.push(String::new());
    assert_eq!(stdout, expected_stdout);
    Ok(())
}

#[test]
fn query_performance_counter() -> Result<()> {
    init_test();
    let stdout = run_and_get_stdout(
        "tests/programs/bin/query_performance_counter.exe",
        vec![
            Duration::from_secs_f64(1.0 / 60.0),
            Duration::from_secs_f64(1.0 / 60.0),
        ],
    )?
    .iter()
    .map(|b| String::from_utf8_lossy(b).to_string())
    .collect::<Vec<_>>();
    let frequency =
        str::parse::<u64>(stdout[0].lines().next().unwrap().split_once('/').unwrap().1).unwrap();
    assert_eq!(
        stdout,
        vec![
            format!("{}/{}\r\n", 0, frequency).repeat(99),
            format!("{}/{}\r\n", frequency / 60, frequency).repeat(100),
            format!("{}/{}\r\n", frequency * 2 / 60, frequency).to_string()
        ]
    );
    Ok(())
}

#[test]
fn query_performance_counter_and_sleep() -> Result<()> {
    init_test();
    let stdout = run_and_get_stdout(
        "tests/programs/bin/query_performance_counter_and_sleep.exe",
        [Duration::from_millis(46), Duration::from_millis(1)].repeat(10),
    )?
    .iter()
    .map(|b| String::from_utf8_lossy(b).to_string())
    .collect::<Vec<_>>();
    let frequency =
        str::parse::<u64>(stdout[0].lines().next().unwrap().split_once('/').unwrap().1).unwrap();

    let mut expected_stdout = Vec::new();
    for index in 0..10 {
        expected_stdout.push(format!(
            "{}/{}\r\n",
            frequency * index * 47 / 1000,
            frequency
        ));
        expected_stdout.push(String::new());
    }
    expected_stdout.push(String::new());
    assert_eq!(stdout, expected_stdout);
    Ok(())
}
