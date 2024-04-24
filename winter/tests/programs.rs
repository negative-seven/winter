use anyhow::Result;
use std::{
    ffi::OsStr,
    path::Path,
    sync::{Arc, Mutex, Once},
    time::Duration,
};
use test_utilities::{build, Architecture};
use test_utilities_macros::test_per_architecture;
use tracing::info;
use winter::InactiveState;

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

async fn run_and_get_stdout(
    executable_path: impl AsRef<Path>,
    executable_command_line_string: impl AsRef<OsStr>,
    events: &[Event],
) -> Result<Vec<Vec<u8>>> {
    let stdout = Arc::new(Mutex::new(Vec::new()));
    let stdout_callback = {
        let stdout = Arc::clone(&stdout);
        move |bytes: &_| {
            for line in String::from_utf8_lossy(bytes).lines() {
                const LINE_LENGTH_LIMIT: usize = 256;
                if line.len() <= LINE_LENGTH_LIMIT {
                    info!("stdout: {}", line);
                } else {
                    info!("stdout: {} (...)", &line[..LINE_LENGTH_LIMIT]);
                }
            }

            stdout.lock().unwrap().extend_from_slice(bytes);
        }
    };
    let mut stdout_by_instant = Vec::new();
    let mut conductor = winter::Conductor::new(
        executable_path.as_ref().to_str().unwrap(),
        &executable_command_line_string.as_ref().to_os_string(),
        Some(stdout_callback),
    )
    .await?;
    conductor.resume().await?;
    for event in events {
        match event {
            Event::AdvanceTime(duration) => {
                assert!(conductor.wait_until_inactive().await? == InactiveState::Idle);
                stdout_by_instant.push(std::mem::take(&mut *stdout.lock().unwrap()));
                conductor.advance_time(*duration).await?;
            }
            Event::SetKeyState { id, state } => {
                conductor.set_key_state(*id, *state).await?;
            }
        }
    }
    assert!(conductor.wait_until_inactive().await? == InactiveState::Terminated);
    stdout_by_instant.push(std::mem::take(&mut *stdout.lock().unwrap()));
    Ok(stdout_by_instant)
}

#[test_per_architecture]
async fn stdout(architecture: Architecture) -> Result<()> {
    init_test();
    let stdout = run_and_get_stdout(build("stdout", architecture), "", &[]).await?;
    assert_eq!(stdout, vec![b"abcABC123!\"_\x99\xaa\xbb"]);
    Ok(())
}

#[test_per_architecture]
async fn stdout_large(architecture: Architecture) -> Result<()> {
    init_test();
    let stdout = run_and_get_stdout(build("stdout_large", architecture), "", &[]).await?;
    assert_eq!(stdout.len(), 1);
    assert_eq!(stdout[0].len(), 1024 * 1024 - 1);
    assert!(stdout[0].iter().all(|&byte| byte == b's'));
    Ok(())
}

#[test_per_architecture]
async fn command_line_string(architecture: Architecture) -> Result<()> {
    init_test();
    let stdout = run_and_get_stdout(
        build("echo_command_line_string", architecture),
        "abcABC123!\"_",
        &[],
    )
    .await?;
    assert_eq!(stdout, vec![b"abcABC123!\"_"]);
    Ok(())
}

#[test_per_architecture]
async fn get_tick_count(architecture: Architecture) -> Result<()> {
    init_test();
    let stdout = run_and_get_stdout(
        build("get_tick_count", architecture),
        "",
        &[
            Event::AdvanceTime(Duration::from_secs_f64(1.0 / 60.0)),
            Event::AdvanceTime(Duration::from_secs_f64(1.0 / 30.0)),
        ],
    )
    .await?
    .iter()
    .map(|b| String::from_utf8_lossy(b).to_string())
    .collect::<Vec<_>>();
    assert_eq!(
        stdout,
        vec![
            "0\r\n".repeat(99),
            "16\r\n".repeat(100),
            "50\r\n".to_string()
        ]
    );
    Ok(())
}

#[test_per_architecture]
async fn get_tick_count_and_sleep(architecture: Architecture) -> Result<()> {
    init_test();
    let stdout = run_and_get_stdout(
        build("get_tick_count_and_sleep", architecture),
        "",
        &[
            &Event::AdvanceTime(Duration::from_millis(78)),
            &Event::AdvanceTime(Duration::from_millis(1)),
        ]
        .repeat(10)
        .into_iter()
        .cloned()
        .collect::<Vec<_>>(),
    )
    .await?
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

#[test_per_architecture]
async fn get_tick_count_64(architecture: Architecture) -> Result<()> {
    init_test();
    let stdout = run_and_get_stdout(
        build("get_tick_count_64", architecture),
        "",
        &[
            Event::AdvanceTime(Duration::from_secs_f64(0.1)),
            Event::AdvanceTime(Duration::from_secs_f64(0.2)),
        ],
    )
    .await?
    .iter()
    .map(|b| String::from_utf8_lossy(b).to_string())
    .collect::<Vec<_>>();
    assert_eq!(
        stdout,
        vec![
            "0\r\n".repeat(99),
            "100\r\n".repeat(100),
            "300\r\n".to_string()
        ]
    );
    Ok(())
}

#[test_per_architecture]
async fn get_tick_count_64_and_sleep(architecture: Architecture) -> Result<()> {
    init_test();
    let stdout = run_and_get_stdout(
        build("get_tick_count_64_and_sleep", architecture),
        "",
        &[
            &Event::AdvanceTime(Duration::from_millis(206)),
            &Event::AdvanceTime(Duration::from_millis(1)),
        ]
        .repeat(10)
        .into_iter()
        .cloned()
        .collect::<Vec<_>>(),
    )
    .await?
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

#[test_per_architecture]
async fn time_get_time(architecture: Architecture) -> Result<()> {
    init_test();
    let stdout = run_and_get_stdout(
        build("time_get_time", architecture),
        "",
        &[
            Event::AdvanceTime(Duration::from_secs_f64(100.0)),
            Event::AdvanceTime(Duration::from_secs_f64(0.001)),
        ],
    )
    .await?
    .iter()
    .map(|b| String::from_utf8_lossy(b).to_string())
    .collect::<Vec<_>>();
    assert_eq!(
        stdout,
        vec![
            "0\r\n".repeat(99),
            "100000\r\n".repeat(100),
            "100001\r\n".to_string()
        ]
    );
    Ok(())
}

#[test_per_architecture]
async fn time_get_time_and_sleep(architecture: Architecture) -> Result<()> {
    init_test();
    let stdout = run_and_get_stdout(
        build("time_get_time_and_sleep", architecture),
        "",
        &[
            &Event::AdvanceTime(Duration::from_millis(40)),
            &Event::AdvanceTime(Duration::from_millis(1)),
        ]
        .repeat(10)
        .into_iter()
        .cloned()
        .collect::<Vec<_>>(),
    )
    .await?
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

#[test_per_architecture]
async fn get_system_time_as_file_time(architecture: Architecture) -> Result<()> {
    init_test();
    let stdout = run_and_get_stdout(
        build("get_system_time_as_file_time", architecture),
        "",
        &[
            Event::AdvanceTime(Duration::from_secs_f64(2.0 / 3.0)),
            Event::AdvanceTime(Duration::from_secs_f64(1.0 / 3.0)),
        ],
    )
    .await?
    .iter()
    .map(|b| String::from_utf8_lossy(b).to_string())
    .collect::<Vec<_>>();
    assert_eq!(
        stdout,
        vec![
            "0 0\r\n".repeat(99),
            "0 6666666\r\n".repeat(100),
            "0 10000000\r\n".to_string()
        ]
    );
    Ok(())
}

#[test_per_architecture]
async fn get_system_time_as_file_time_and_sleep(architecture: Architecture) -> Result<()> {
    init_test();
    let stdout = run_and_get_stdout(
        build("get_system_time_as_file_time_and_sleep", architecture),
        "",
        &[
            &Event::AdvanceTime(Duration::from_millis(192)),
            &Event::AdvanceTime(Duration::from_millis(1)),
        ]
        .repeat(10)
        .into_iter()
        .cloned()
        .collect::<Vec<_>>(),
    )
    .await?
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
    Ok(())
}

#[test_per_architecture]
async fn get_system_time_precise_as_file_time(architecture: Architecture) -> Result<()> {
    init_test();
    let stdout = run_and_get_stdout(
        build("get_system_time_precise_as_file_time", architecture),
        "",
        &[
            Event::AdvanceTime(Duration::from_secs_f64(2.0 / 5.0)),
            Event::AdvanceTime(Duration::from_secs_f64(17.0 / 100.0)),
        ],
    )
    .await?
    .iter()
    .map(|b| String::from_utf8_lossy(b).to_string())
    .collect::<Vec<_>>();
    assert_eq!(
        stdout,
        vec![
            "0 0\r\n".repeat(99),
            "0 4000000\r\n".repeat(100),
            "0 5700000\r\n".to_string()
        ]
    );
    Ok(())
}

#[test_per_architecture]
async fn get_system_time_precise_as_file_time_and_sleep(architecture: Architecture) -> Result<()> {
    init_test();
    let stdout = run_and_get_stdout(
        build(
            "get_system_time_precise_as_file_time_and_sleep",
            architecture,
        ),
        "",
        &[
            &Event::AdvanceTime(Duration::from_millis(6)),
            &Event::AdvanceTime(Duration::from_millis(1)),
        ]
        .repeat(10)
        .into_iter()
        .cloned()
        .collect::<Vec<_>>(),
    )
    .await?
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
    Ok(())
}

#[test_per_architecture]
async fn query_performance_counter(architecture: Architecture) -> Result<()> {
    init_test();
    let stdout = run_and_get_stdout(
        build("query_performance_counter", architecture),
        "",
        &[
            Event::AdvanceTime(Duration::from_secs_f64(1.0 / 25.0)),
            Event::AdvanceTime(Duration::from_secs_f64(1.0 / 50.0)),
        ],
    )
    .await?
    .iter()
    .map(|b| String::from_utf8_lossy(b).to_string())
    .collect::<Vec<_>>();
    let frequency =
        str::parse::<u64>(stdout[0].lines().next().unwrap().split_once('/').unwrap().1).unwrap();
    assert_eq!(
        stdout,
        vec![
            format!("{}/{}\r\n", 0, frequency).repeat(99),
            format!("{}/{}\r\n", frequency / 25, frequency).repeat(100),
            format!("{}/{}\r\n", frequency * 3 / 50, frequency).to_string()
        ]
    );
    Ok(())
}

#[test_per_architecture]
async fn query_performance_counter_and_sleep(architecture: Architecture) -> Result<()> {
    init_test();
    let stdout = run_and_get_stdout(
        build("query_performance_counter_and_sleep", architecture),
        "",
        &[
            &Event::AdvanceTime(Duration::from_millis(46)),
            &Event::AdvanceTime(Duration::from_millis(1)),
        ]
        .repeat(10)
        .into_iter()
        .cloned()
        .collect::<Vec<_>>(),
    )
    .await?
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

#[test_per_architecture]
async fn register_class_ex_a(architecture: Architecture) -> Result<()> {
    init_test();
    let stdout = run_and_get_stdout(build("register_class_ex_a", architecture), "", &[])
        .await?
        .iter()
        .map(|b| String::from_utf8_lossy(b).to_string())
        .collect::<Vec<_>>();

    assert_eq!(stdout, vec!["275\r\n"]);

    Ok(())
}

#[test_per_architecture]
async fn register_class_ex_w(architecture: Architecture) -> Result<()> {
    init_test();
    let stdout = run_and_get_stdout(build("register_class_ex_w", architecture), "", &[])
        .await?
        .iter()
        .map(|b| String::from_utf8_lossy(b).to_string())
        .collect::<Vec<_>>();

    assert_eq!(stdout, vec!["275\r\n"]);

    Ok(())
}

async fn helper_for_key_state_tests(
    program_name: impl AsRef<str>,
    architecture: Architecture,
) -> Result<()> {
    fn key_event(id: u8, state: bool) -> Event {
        Event::SetKeyState { id, state }
    }

    init_test();
    let stdout = run_and_get_stdout(
        build(&program_name, architecture),
        "",
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
    )
    .await?
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

#[test_per_architecture]
async fn get_key_state(architecture: Architecture) -> Result<()> {
    helper_for_key_state_tests("get_key_state", architecture).await
}

#[test_per_architecture]
async fn get_async_key_state(architecture: Architecture) -> Result<()> {
    helper_for_key_state_tests("get_async_key_state", architecture).await
}

#[test_per_architecture]
async fn get_keyboard_state(architecture: Architecture) -> Result<()> {
    helper_for_key_state_tests("get_keyboard_state", architecture).await
}

#[test_per_architecture]
async fn key_down_and_key_up(architecture: Architecture) -> Result<()> {
    fn key_event(id: u8, state: bool) -> Event {
        Event::SetKeyState { id, state }
    }

    init_test();
    let stdout = run_and_get_stdout(
        build("key_down_and_key_up", architecture),
        "",
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
    )
    .await?
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

#[test_per_architecture]
async fn nt_set_information_thread(architecture: Architecture) -> Result<()> {
    init_test();
    let stdout = run_and_get_stdout(build("nt_set_information_thread", architecture), "", &[])
        .await?
        .iter()
        .map(|b| String::from_utf8_lossy(b).to_string())
        .collect::<Vec<_>>();
    assert_eq!(stdout, vec!["start\r\nbreakpoint\r\nend\r\n"]);
    Ok(())
}
