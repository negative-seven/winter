use anyhow::Result;
use futures::executor::block_on;
use std::{
    path::Path,
    sync::{Arc, Mutex, Once},
    time::Duration,
};
use test_utilities::build;
use tracing::info;

fn init_test() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
            .init();
    });
}

#[derive(Clone)]
enum Event {
    AdvanceTime(Duration),
    SetKeyState { id: u8, state: bool },
}

fn run_and_get_stdout(executable_path: impl AsRef<Path>, events: &[Event]) -> Result<Vec<Vec<u8>>> {
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
    let mut conductor = block_on(winter::Conductor::new(
        executable_path.as_ref().to_str().unwrap(),
        Some(stdout_callback),
    ))?;
    block_on(conductor.resume())?;
    for event in events {
        match event {
            Event::AdvanceTime(duration) => {
                block_on(conductor.wait_until_idle())?;
                stdout_by_instant.push(std::mem::take(&mut *stdout.lock().unwrap()));
                block_on(conductor.advance_time(*duration))?;
            }
            Event::SetKeyState { id, state } => {
                block_on(conductor.set_key_state(*id, *state))?;
            }
        }
    }
    block_on(conductor.wait_until_exit())?; // TODO: check that process only exited after the last time advancement
    stdout_by_instant.push(std::mem::take(&mut *stdout.lock().unwrap()));
    Ok(stdout_by_instant)
}

#[test]
fn stdout() -> Result<()> {
    init_test();
    for executable_path in build("stdout") {
        let stdout = run_and_get_stdout(executable_path, &[])?;
        assert_eq!(stdout, vec![b"abcABC123!\"_\x99\xaa\xbb"]);
    }
    Ok(())
}

#[test]
fn get_tick_count() -> Result<()> {
    init_test();
    for executable_path in build("get_tick_count") {
        let stdout = run_and_get_stdout(
            executable_path,
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
    }
    Ok(())
}

#[test]
fn get_tick_count_and_sleep() -> Result<()> {
    init_test();
    for executable_path in build("get_tick_count_and_sleep") {
        let stdout = run_and_get_stdout(
            executable_path,
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
    }
    Ok(())
}

#[test]
fn get_tick_count_64() -> Result<()> {
    init_test();
    for executable_path in build("get_tick_count_64") {
        let stdout = run_and_get_stdout(
            executable_path,
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
    }
    Ok(())
}

#[test]
fn get_tick_count_64_and_sleep() -> Result<()> {
    init_test();
    for executable_path in build("get_tick_count_64_and_sleep") {
        let stdout = run_and_get_stdout(
            executable_path,
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
    }
    Ok(())
}

#[test]
fn time_get_time() -> Result<()> {
    init_test();
    for executable_path in build("time_get_time") {
        let stdout = run_and_get_stdout(
            executable_path,
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
    }
    Ok(())
}

#[test]
fn time_get_time_and_sleep() -> Result<()> {
    init_test();
    for executable_path in build("time_get_time_and_sleep") {
        let stdout = run_and_get_stdout(
            executable_path,
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
    }
    Ok(())
}

#[test]
fn get_system_time_as_file_time() -> Result<()> {
    init_test();
    for executable_path in build("get_system_time_as_file_time") {
        let stdout = run_and_get_stdout(
            executable_path,
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
                "0 0\r\n".repeat(99),
                "0 166666\r\n".repeat(100),
                "0 333333\r\n".to_string()
            ]
        );
    }
    Ok(())
}

#[test]
fn get_system_time_as_file_time_and_sleep() -> Result<()> {
    init_test();
    for executable_path in build("get_system_time_as_file_time_and_sleep") {
        let stdout = run_and_get_stdout(
            executable_path,
            &[
                &Event::AdvanceTime(Duration::from_millis(192)),
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
            expected_stdout.push(format!("0 {}\r\n", index * 1_930_000));
            expected_stdout.push(String::new());
        }
        expected_stdout.push(String::new());
        assert_eq!(stdout, expected_stdout);
    }
    Ok(())
}

#[test]
fn get_system_time_precise_as_file_time() -> Result<()> {
    init_test();
    for executable_path in build("get_system_time_precise_as_file_time") {
        let stdout = run_and_get_stdout(
            executable_path,
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
                "0 0\r\n".repeat(99),
                "0 166666\r\n".repeat(100),
                "0 333333\r\n".to_string()
            ]
        );
    }
    Ok(())
}

#[test]
fn get_system_time_precise_as_file_time_and_sleep() -> Result<()> {
    init_test();
    for executable_path in build("get_system_time_precise_as_file_time_and_sleep") {
        let stdout = run_and_get_stdout(
            executable_path,
            &[
                &Event::AdvanceTime(Duration::from_millis(6)),
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
            expected_stdout.push(format!("0 {}\r\n", index * 70_000));
            expected_stdout.push(String::new());
        }
        expected_stdout.push(String::new());
        assert_eq!(stdout, expected_stdout);
    }
    Ok(())
}

#[test]
fn query_performance_counter() -> Result<()> {
    init_test();
    for executable_path in build("query_performance_counter") {
        let stdout = run_and_get_stdout(
            executable_path,
            &[
                Event::AdvanceTime(Duration::from_secs_f64(1.0 / 60.0)),
                Event::AdvanceTime(Duration::from_secs_f64(1.0 / 60.0)),
            ],
        )?
        .iter()
        .map(|b| String::from_utf8_lossy(b).to_string())
        .collect::<Vec<_>>();
        let frequency =
            str::parse::<u64>(stdout[0].lines().next().unwrap().split_once('/').unwrap().1)
                .unwrap();
        assert_eq!(
            stdout,
            vec![
                format!("{}/{}\r\n", 0, frequency).repeat(99),
                format!("{}/{}\r\n", frequency / 60, frequency).repeat(100),
                format!("{}/{}\r\n", frequency * 2 / 60, frequency).to_string()
            ]
        );
    }
    Ok(())
}

#[test]
fn query_performance_counter_and_sleep() -> Result<()> {
    init_test();
    for executable_path in build("query_performance_counter_and_sleep") {
        let stdout = run_and_get_stdout(
            executable_path,
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
            str::parse::<u64>(stdout[0].lines().next().unwrap().split_once('/').unwrap().1)
                .unwrap();

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
    }
    Ok(())
}

fn helper_for_key_state_tests(program_name: impl AsRef<str>) -> Result<()> {
    fn key_event(id: u8, state: bool) -> Event {
        Event::SetKeyState { id, state }
    }

    init_test();
    for executable_path in build(program_name) {
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
    }
    Ok(())
}

#[test]
fn get_key_state() -> Result<()> {
    helper_for_key_state_tests("get_key_state")
}

#[test]
fn get_async_key_state() -> Result<()> {
    helper_for_key_state_tests("get_async_key_state")
}

#[test]
fn get_keyboard_state() -> Result<()> {
    helper_for_key_state_tests("get_keyboard_state")
}

#[test]
fn key_down_and_key_up() -> Result<()> {
    fn key_event(id: u8, state: bool) -> Event {
        Event::SetKeyState { id, state }
    }

    init_test();
    for executable_path in build("key_down_and_key_up") {
        let stdout = run_and_get_stdout(
            executable_path,
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
    }
    Ok(())
}
