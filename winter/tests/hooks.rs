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

#[derive(Clone)]
enum Event {
    AdvanceTime(Duration),
    SetKeyState { id: u8, state: bool },
}

fn run_and_get_stdout(executable_path: &str, events: &[Event]) -> Result<Vec<Vec<u8>>> {
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
    let mut conductor = winter::Conductor::new(executable_path, Some(stdout_callback))?;
    conductor.resume()?;
    for event in events {
        match event {
            Event::AdvanceTime(duration) => {
                conductor.wait_until_idle()?;
                stdout_by_instant.push(std::mem::take(&mut *stdout.lock().unwrap()));
                conductor.advance_time(*duration)?;
            }
            Event::SetKeyState { id, state } => {
                conductor.set_key_state(*id, *state)?;
            }
        }
    }
    conductor.wait_until_exit()?; // TODO: check that process only exited after the last time advancement
    stdout_by_instant.push(std::mem::take(&mut *stdout.lock().unwrap()));
    Ok(stdout_by_instant)
}

#[test]
fn stdout() -> Result<()> {
    init_test();
    let stdout = run_and_get_stdout("tests/programs/bin/stdout.exe", &[])?;
    assert_eq!(stdout, vec![b"abcABC123!\"_\x99\xaa\xbb"]);
    Ok(())
}

#[test]
fn get_tick_count() -> Result<()> {
    init_test();
    let stdout = run_and_get_stdout(
        "tests/programs/bin/get_tick_count.exe",
        &[
            Event::AdvanceTime(Duration::from_secs_f64(1.0 / 60.0)),
            Event::AdvanceTime(Duration::from_secs_f64(1.0 / 60.0)),
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
        &[
            &Event::AdvanceTime(Duration::from_millis(78)),
            &Event::AdvanceTime(Duration::from_millis(1)),
        ]
        .repeat(10)
        .into_iter()
        .cloned()
        .collect::<Vec<_>>(),
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
fn get_tick_count_64() -> Result<()> {
    init_test();
    let stdout = run_and_get_stdout(
        "tests/programs/bin/get_tick_count_64.exe",
        &[
            Event::AdvanceTime(Duration::from_secs_f64(1.0 / 60.0)),
            Event::AdvanceTime(Duration::from_secs_f64(1.0 / 60.0)),
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
fn get_tick_count_64_and_sleep() -> Result<()> {
    init_test();
    let stdout = run_and_get_stdout(
        "tests/programs/bin/get_tick_count_64_and_sleep.exe",
        &[
            &Event::AdvanceTime(Duration::from_millis(206)),
            &Event::AdvanceTime(Duration::from_millis(1)),
        ]
        .repeat(10)
        .into_iter()
        .cloned()
        .collect::<Vec<_>>(),
    )?
    .iter()
    .map(|b| String::from_utf8_lossy(b).to_string())
    .collect::<Vec<_>>();

    let mut expected_stdout = Vec::new();
    for index in 0..10 {
        expected_stdout.push(format!("{}\r\n", index * 207));
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
        &[
            Event::AdvanceTime(Duration::from_secs_f64(1.0 / 60.0)),
            Event::AdvanceTime(Duration::from_secs_f64(1.0 / 60.0)),
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
        &[
            &Event::AdvanceTime(Duration::from_millis(40)),
            &Event::AdvanceTime(Duration::from_millis(1)),
        ]
        .repeat(10)
        .into_iter()
        .cloned()
        .collect::<Vec<_>>(),
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
        &[
            Event::AdvanceTime(Duration::from_secs_f64(1.0 / 60.0)),
            Event::AdvanceTime(Duration::from_secs_f64(1.0 / 60.0)),
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
        &[
            &Event::AdvanceTime(Duration::from_millis(46)),
            &Event::AdvanceTime(Duration::from_millis(1)),
        ]
        .repeat(10)
        .into_iter()
        .cloned()
        .collect::<Vec<_>>(),
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

fn helper_for_key_state_tests(executable_path: &str) -> Result<()> {
    fn key_event(id: u8, state: bool) -> Event {
        Event::SetKeyState { id, state }
    }

    init_test();
    let stdout = run_and_get_stdout(
        executable_path,
        &[
            key_event(65, true),
            key_event(65, true),
            key_event(66, true),
            key_event(67, true),
            Event::AdvanceTime(Duration::from_millis(20)),
            key_event(65, true),
            key_event(67, true),
            Event::AdvanceTime(Duration::from_millis(20)),
            key_event(68, true),
            key_event(67, false),
            key_event(67, false),
            Event::AdvanceTime(Duration::from_millis(20)),
            key_event(37, true),
            key_event(65, false),
            key_event(37, false),
            key_event(66, false),
            key_event(68, false),
            Event::AdvanceTime(Duration::from_millis(20)),
            key_event(40, false),
            key_event(40, true),
            Event::AdvanceTime(Duration::from_millis(20)),
        ],
    )?
    .iter()
    .map(|b| String::from_utf8_lossy(b).to_string())
    .collect::<Vec<_>>();
    assert_eq!(
        stdout,
        [
            "",
            "65 66 67 \r\n",
            "65 66 67 \r\n",
            "65 66 68 \r\n",
            "\r\n",
            "40 \r\n",
        ]
    );
    Ok(())
}

#[test]
fn get_key_state() -> Result<()> {
    helper_for_key_state_tests("tests/programs/bin/get_key_state.exe")
}

#[test]
fn get_async_key_state() -> Result<()> {
    helper_for_key_state_tests("tests/programs/bin/get_async_key_state.exe")
}

#[test]
fn get_keyboard_state() -> Result<()> {
    helper_for_key_state_tests("tests/programs/bin/get_keyboard_state.exe")
}

#[test]
fn key_down_and_key_up() -> Result<()> {
    fn key_event(id: u8, state: bool) -> Event {
        Event::SetKeyState { id, state }
    }

    init_test();
    let stdout = run_and_get_stdout(
        "tests/programs/bin/key_down_and_key_up.exe",
        &[
            key_event(65, true),
            key_event(65, true),
            key_event(66, true),
            key_event(67, true),
            Event::AdvanceTime(Duration::from_millis(77)),
            key_event(65, true),
            key_event(67, true),
            Event::AdvanceTime(Duration::from_millis(18)),
            key_event(68, true),
            key_event(67, false),
            key_event(67, false),
            Event::AdvanceTime(Duration::from_millis(1)),
            key_event(37, true),
            key_event(65, false),
            key_event(37, false),
            key_event(66, false),
            key_event(68, false),
            Event::AdvanceTime(Duration::from_millis(1)),
            key_event(40, false),
            key_event(40, true),
            Event::AdvanceTime(Duration::from_millis(3)),
        ],
    )?
    .iter()
    .map(|b| String::from_utf8_lossy(b).to_string())
    .collect::<Vec<_>>();
    assert_eq!(
        stdout,
        [
            &[] as &[&str],
            &[
                "KEYDOWN 65 00000001",
                "KEYDOWN 65 40000001",
                "KEYDOWN 66 00000001",
                "KEYDOWN 67 00000001",
            ],
            &["KEYDOWN 65 40000001", "KEYDOWN 67 40000001"],
            &[
                "KEYDOWN 68 00000001",
                "KEYUP 67 c0000001",
                "KEYUP 67 80000001",
            ],
            &[
                "KEYDOWN 37 00000001",
                "KEYUP 65 c0000001",
                "KEYUP 37 c0000001",
                "KEYUP 66 c0000001",
                "KEYUP 68 c0000001"
            ],
            &["KEYUP 40 80000001", "KEYDOWN 40 00000001"],
        ]
        .map(|item| if item.is_empty() {
            String::new()
        } else {
            item.join("\r\n") + "\r\n"
        })
    );
    Ok(())
}