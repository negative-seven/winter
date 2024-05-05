use anyhow::Result;
use std::{sync::Once, time::Duration};
use test_utilities::{Architecture, Event, Instance};
use test_utilities_macros::test_for;

fn init_test() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
            .init();
    });
}

#[test_for(architecture)]
async fn stdout(architecture: Architecture) -> Result<()> {
    init_test();
    let stdout = Instance::new("stdout", architecture).stdout().await?;
    assert_eq!(stdout, vec![b"abcABC123!\"_\x99\xaa\xbb"]);
    Ok(())
}

#[test_for(architecture)]
async fn stdout_large(architecture: Architecture) -> Result<()> {
    init_test();
    let stdout = Instance::new("stdout_large", architecture).stdout().await?;
    assert_eq!(stdout.len(), 1);
    assert_eq!(stdout[0].len(), 1024 * 1024 - 1);
    assert!(stdout[0].iter().all(|&byte| byte == b's'));
    Ok(())
}

#[test_for(architecture, unicode)]
async fn command_line_string(architecture: Architecture, unicode: bool) -> Result<()> {
    init_test();
    let stdout = Instance::new("echo_command_line_string", architecture)
        .with_unicode_flag(unicode)
        .with_command_line_string("abcABC123!\"_".into())
        .stdout()
        .await?;
    assert_eq!(stdout, vec![b"abcABC123!\"_"]);
    Ok(())
}

#[test_for(architecture)]
async fn get_tick_count(architecture: Architecture) -> Result<()> {
    init_test();
    let stdout = Instance::new("get_tick_count", architecture)
        .with_events([
            Event::AdvanceTime(Duration::from_secs_f64(1.0 / 60.0)),
            Event::AdvanceTime(Duration::from_secs_f64(1.0 / 30.0)),
        ])
        .stdout_from_utf8_lossy()
        .await?;
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

#[test_for(architecture)]
async fn get_tick_count_and_sleep(architecture: Architecture) -> Result<()> {
    init_test();
    let stdout = Instance::new("get_tick_count_and_sleep", architecture)
        .with_events(
            [
                &Event::AdvanceTime(Duration::from_millis(78)),
                &Event::AdvanceTime(Duration::from_millis(1)),
            ]
            .repeat(10)
            .into_iter()
            .cloned(),
        )
        .stdout_from_utf8_lossy()
        .await?;

    let mut expected_stdout = Vec::new();
    for index in 0..10 {
        expected_stdout.push(format!("{}\r\n", index * 79));
        expected_stdout.push(String::new());
    }
    expected_stdout.push(String::new());
    assert_eq!(stdout, expected_stdout);
    Ok(())
}

#[test_for(architecture)]
async fn get_tick_count_64(architecture: Architecture) -> Result<()> {
    init_test();
    let stdout = Instance::new("get_tick_count_64", architecture)
        .with_events([
            Event::AdvanceTime(Duration::from_secs_f64(0.1)),
            Event::AdvanceTime(Duration::from_secs_f64(0.2)),
        ])
        .stdout_from_utf8_lossy()
        .await?;
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

#[test_for(architecture)]
async fn get_tick_count_64_and_sleep(architecture: Architecture) -> Result<()> {
    init_test();
    let stdout = Instance::new("get_tick_count_64_and_sleep", architecture)
        .with_events(
            [
                &Event::AdvanceTime(Duration::from_millis(206)),
                &Event::AdvanceTime(Duration::from_millis(1)),
            ]
            .repeat(10)
            .into_iter()
            .cloned(),
        )
        .stdout_from_utf8_lossy()
        .await?;

    let mut expected_stdout = Vec::new();
    for index in 0..10 {
        expected_stdout.push(format!("{}\r\n", index * 207));
        expected_stdout.push(String::new());
    }
    expected_stdout.push(String::new());
    assert_eq!(stdout, expected_stdout);
    Ok(())
}

#[test_for(architecture)]
async fn time_get_time(architecture: Architecture) -> Result<()> {
    init_test();
    let stdout = Instance::new("time_get_time", architecture)
        .with_events([
            Event::AdvanceTime(Duration::from_secs_f64(100.0)),
            Event::AdvanceTime(Duration::from_secs_f64(0.001)),
        ])
        .stdout_from_utf8_lossy()
        .await?;
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

#[test_for(architecture)]
async fn time_get_time_and_sleep(architecture: Architecture) -> Result<()> {
    init_test();
    let stdout = Instance::new("time_get_time_and_sleep", architecture)
        .with_events(
            [
                &Event::AdvanceTime(Duration::from_millis(40)),
                &Event::AdvanceTime(Duration::from_millis(1)),
            ]
            .repeat(10)
            .into_iter()
            .cloned(),
        )
        .stdout_from_utf8_lossy()
        .await?;

    let mut expected_stdout = Vec::new();
    for index in 0..10 {
        expected_stdout.push(format!("{}\r\n", index * 41));
        expected_stdout.push(String::new());
    }
    expected_stdout.push(String::new());
    assert_eq!(stdout, expected_stdout);
    Ok(())
}

#[test_for(architecture)]
async fn get_system_time_as_file_time(architecture: Architecture) -> Result<()> {
    init_test();
    let stdout = Instance::new("get_system_time_as_file_time", architecture)
        .with_events([
            Event::AdvanceTime(Duration::from_secs_f64(2.0 / 3.0)),
            Event::AdvanceTime(Duration::from_secs_f64(1.0 / 3.0)),
        ])
        .stdout_from_utf8_lossy()
        .await?;
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

#[test_for(architecture)]
async fn get_system_time_as_file_time_and_sleep(architecture: Architecture) -> Result<()> {
    init_test();
    let stdout = Instance::new("get_system_time_as_file_time_and_sleep", architecture)
        .with_events(
            [
                &Event::AdvanceTime(Duration::from_millis(192)),
                &Event::AdvanceTime(Duration::from_millis(1)),
            ]
            .repeat(10)
            .into_iter()
            .cloned(),
        )
        .stdout_from_utf8_lossy()
        .await?;

    let mut expected_stdout = Vec::new();
    for index in 0..10 {
        expected_stdout.push(format!("0 {}\r\n", index * 1_930_000));
        expected_stdout.push(String::new());
    }
    expected_stdout.push(String::new());
    assert_eq!(stdout, expected_stdout);
    Ok(())
}

#[test_for(architecture)]
async fn get_system_time_precise_as_file_time(architecture: Architecture) -> Result<()> {
    init_test();
    let stdout = Instance::new("get_system_time_precise_as_file_time", architecture)
        .with_events([
            Event::AdvanceTime(Duration::from_secs_f64(2.0 / 5.0)),
            Event::AdvanceTime(Duration::from_secs_f64(17.0 / 100.0)),
        ])
        .stdout_from_utf8_lossy()
        .await?;
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

#[test_for(architecture)]
async fn get_system_time_precise_as_file_time_and_sleep(architecture: Architecture) -> Result<()> {
    init_test();
    let stdout = Instance::new(
        "get_system_time_precise_as_file_time_and_sleep",
        architecture,
    )
    .with_events(
        [
            &Event::AdvanceTime(Duration::from_millis(6)),
            &Event::AdvanceTime(Duration::from_millis(1)),
        ]
        .repeat(10)
        .into_iter()
        .cloned(),
    )
    .stdout_from_utf8_lossy()
    .await?;

    let mut expected_stdout = Vec::new();
    for index in 0..10 {
        expected_stdout.push(format!("0 {}\r\n", index * 70_000));
        expected_stdout.push(String::new());
    }
    expected_stdout.push(String::new());
    assert_eq!(stdout, expected_stdout);
    Ok(())
}

#[test_for(architecture)]
async fn query_performance_counter(architecture: Architecture) -> Result<()> {
    init_test();
    let stdout = Instance::new("query_performance_counter", architecture)
        .with_events([
            Event::AdvanceTime(Duration::from_secs_f64(1.0 / 25.0)),
            Event::AdvanceTime(Duration::from_secs_f64(1.0 / 50.0)),
        ])
        .stdout_from_utf8_lossy()
        .await?;
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

#[test_for(architecture)]
async fn query_performance_counter_and_sleep(architecture: Architecture) -> Result<()> {
    init_test();
    let stdout = Instance::new("query_performance_counter_and_sleep", architecture)
        .with_events(
            [
                &Event::AdvanceTime(Duration::from_millis(46)),
                &Event::AdvanceTime(Duration::from_millis(1)),
            ]
            .repeat(10)
            .into_iter()
            .cloned(),
        )
        .stdout_from_utf8_lossy()
        .await?;
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

#[test_for(architecture, unicode)]
async fn register_class_ex(architecture: Architecture, unicode: bool) -> Result<()> {
    init_test();
    let stdout = Instance::new("register_class_ex", architecture)
        .with_unicode_flag(unicode)
        .stdout_from_utf8_lossy()
        .await?;

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
    let stdout = Instance::new(program_name.as_ref(), architecture)
        .with_events([
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
        ])
        .stdout_from_utf8_lossy()
        .await?;
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

#[test_for(architecture)]
async fn get_key_state(architecture: Architecture) -> Result<()> {
    helper_for_key_state_tests("get_key_state", architecture).await
}

#[test_for(architecture)]
async fn get_async_key_state(architecture: Architecture) -> Result<()> {
    helper_for_key_state_tests("get_async_key_state", architecture).await
}
#[test_for(architecture)]
async fn get_keyboard_state(architecture: Architecture) -> Result<()> {
    helper_for_key_state_tests("get_keyboard_state", architecture).await
}

#[derive(Debug, Eq, PartialEq)]
struct Message {
    milliseconds: u32,
    id: u32,
    w_parameter: usize,
    l_parameter: isize,
}

impl Message {
    fn new(milliseconds: u32, id: u32, w_parameter: usize, l_parameter: isize) -> Self {
        Self {
            milliseconds,
            id,
            w_parameter,
            l_parameter,
        }
    }
}

fn extract_messages_from_stdout(stdout: &[Vec<u8>], message_ids: &[u32]) -> Vec<Vec<Message>> {
    stdout
        .iter()
        .map(|stdout_group| {
            String::from_utf8_lossy(stdout_group)
                .lines()
                .filter_map(|line| {
                    let mut tokens = line.split_ascii_whitespace();
                    let milliseconds = tokens.next().unwrap().parse().unwrap();
                    let id = tokens.next().unwrap().parse().unwrap();
                    if !message_ids.contains(&id) {
                        return None;
                    }
                    let w_parameter = tokens.next().unwrap().parse().unwrap();
                    let l_parameter = tokens.next().unwrap().parse().unwrap();
                    Some(Message {
                        milliseconds,
                        id,
                        w_parameter,
                        l_parameter,
                    })
                })
                .collect()
        })
        .collect()
}

#[test_for(architecture, unicode)]
async fn peek_message_with_key_messages(architecture: Architecture, unicode: bool) -> Result<()> {
    const WM_KEYDOWN: u32 = 256;
    const WM_KEYUP: u32 = 257;

    fn key_event(id: u8, state: bool) -> Event {
        Event::SetKeyState { id, state }
    }

    init_test();
    let messages = extract_messages_from_stdout(
        &Instance::new("peek_message", architecture)
            .with_unicode_flag(unicode)
            .with_events([
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
            ])
            .stdout()
            .await?,
        &[WM_KEYDOWN, WM_KEYUP],
    );

    let message = Message::new;
    assert_eq!(
        messages,
        [
            &[] as &[Message],
            &[
                message(1, WM_KEYDOWN, 65, 1),
                message(1, WM_KEYDOWN, 65, (1 << 30) | 1),
                message(1, WM_KEYDOWN, 66, 1),
                message(1, WM_KEYDOWN, 67, 1),
            ],
            &[
                message(78, WM_KEYDOWN, 65, (1 << 30) | 1),
                message(78, WM_KEYDOWN, 67, (1 << 30) | 1)
            ],
            &[
                message(96, WM_KEYDOWN, 68, 1),
                message(96, WM_KEYUP, 67, (1 << 31) | (1 << 30) | 1),
                message(96, WM_KEYUP, 67, (1 << 31) | 1),
            ],
            &[
                message(97, WM_KEYDOWN, 37, 1),
                message(97, WM_KEYUP, 65, (1 << 31) | (1 << 30) | 1),
                message(97, WM_KEYUP, 37, (1 << 31) | (1 << 30) | 1),
                message(97, WM_KEYUP, 66, (1 << 31) | (1 << 30) | 1),
                message(97, WM_KEYUP, 68, (1 << 31) | (1 << 30) | 1)
            ],
            &[
                message(98, WM_KEYUP, 40, (1 << 31) | 1),
                message(98, WM_KEYDOWN, 40, 1)
            ],
        ]
    );
    Ok(())
}

#[test_for(architecture, unicode)]
async fn get_message_with_key_messages(architecture: Architecture, unicode: bool) -> Result<()> {
    const WM_KEYDOWN: u32 = 256;
    const WM_KEYUP: u32 = 257;

    fn key_event(id: u8, state: bool) -> Event {
        Event::SetKeyState { id, state }
    }

    init_test();
    let messages = extract_messages_from_stdout(
        &Instance::new("get_message", architecture)
            .with_unicode_flag(unicode)
            .with_events([
                key_event(65, true),
                key_event(65, true),
                key_event(66, true),
                key_event(67, true),
                Event::AdvanceTime(Duration::from_millis(12)),
                key_event(65, true),
                key_event(67, true),
                Event::AdvanceTime(Duration::from_millis(34)),
                key_event(68, true),
                key_event(67, false),
                key_event(67, false),
                Event::AdvanceTime(Duration::from_millis(56)),
                key_event(37, true),
                key_event(65, false),
                key_event(37, false),
                key_event(66, false),
                key_event(68, false),
                Event::AdvanceTime(Duration::from_millis(78)),
                key_event(40, false),
                key_event(40, true),
                Event::AdvanceTime(Duration::from_millis(90)),
            ])
            .stdout()
            .await?,
        &[WM_KEYDOWN, WM_KEYUP],
    );

    let message = Message::new;
    assert_eq!(
        messages,
        [
            &[] as &[Message],
            &[
                message(12, WM_KEYDOWN, 65, 1),
                message(12, WM_KEYDOWN, 65, (1 << 30) | 1),
                message(12, WM_KEYDOWN, 66, 1),
                message(12, WM_KEYDOWN, 67, 1),
            ],
            &[
                message(46, WM_KEYDOWN, 65, (1 << 30) | 1),
                message(46, WM_KEYDOWN, 67, (1 << 30) | 1)
            ],
            &[
                message(102, WM_KEYDOWN, 68, 1),
                message(102, WM_KEYUP, 67, (1 << 31) | (1 << 30) | 1),
                message(102, WM_KEYUP, 67, (1 << 31) | 1),
            ],
            &[
                message(180, WM_KEYDOWN, 37, 1),
                message(180, WM_KEYUP, 65, (1 << 31) | (1 << 30) | 1),
                message(180, WM_KEYUP, 37, (1 << 31) | (1 << 30) | 1),
                message(180, WM_KEYUP, 66, (1 << 31) | (1 << 30) | 1),
                message(180, WM_KEYUP, 68, (1 << 31) | (1 << 30) | 1)
            ],
            &[
                message(270, WM_KEYUP, 40, (1 << 31) | 1),
                message(270, WM_KEYDOWN, 40, 1)
            ],
        ]
    );
    Ok(())
}

#[test_for(architecture)]
async fn nt_set_information_thread(architecture: Architecture) -> Result<()> {
    init_test();
    let stdout = Instance::new("nt_set_information_thread", architecture)
        .stdout_from_utf8_lossy()
        .await?;
    assert_eq!(stdout, vec!["start\r\nbreakpoint\r\nend\r\n"]);
    Ok(())
}
