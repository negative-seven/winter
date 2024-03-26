use anyhow::Result;
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    process::Command,
    str::FromStr,
    sync::{Arc, Mutex, Once, OnceLock},
    time::Duration,
};
use tracing::info;

fn init_test() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
            .init();
    });
}

fn should_build(source_file_path: impl AsRef<Path>, binary_file_path: impl AsRef<Path>) -> bool {
    let Ok(source_file_modified_time) = source_file_path
        .as_ref()
        .metadata()
        .map(|m| m.modified().unwrap())
    else {
        return true;
    };

    let Ok(binary_file_modified_time) = binary_file_path
        .as_ref()
        .metadata()
        .map(|m| m.modified().unwrap())
    else {
        return true;
    };

    binary_file_modified_time <= source_file_modified_time
}

fn build(program_name: impl AsRef<str>) -> PathBuf {
    static ENVIRONMENT_VARIABLES: OnceLock<Vec<(OsString, OsString)>> = OnceLock::new();

    let source_file_path = PathBuf::from(format!("tests/programs/src/{}.c", program_name.as_ref()));
    let binary_file_path =
        PathBuf::from(format!("tests/programs/bin/{}.exe", program_name.as_ref()));
    if !should_build(&source_file_path, &binary_file_path) {
        return binary_file_path;
    }

    let environment_variables = ENVIRONMENT_VARIABLES.get_or_init(|| {
        const VCVARS_DIR_ERROR: &str = "the environment variable VCVARS_DIR must be set to a \
            directory containing vcvars scripts to successfully build tests";

        let vcvars_script_path =
            Path::new(&std::env::var("VCVARS_DIR").expect(VCVARS_DIR_ERROR)).join("vcvars32.bat");
        assert!(vcvars_script_path.exists(), "{}", VCVARS_DIR_ERROR);

        let command = Command::new("cmd")
            .arg("/C")
            .arg(vcvars_script_path)
            .args([">NUL", "&&", "set"])
            .output()
            .unwrap();
        eprint!("{}", String::from_utf8_lossy(&command.stderr));
        assert!(command.status.success());
        let stdout = String::from_utf8(command.stdout).unwrap();
        stdout
            .lines()
            .map(|line| {
                let (key, value) = line.split_once('=').unwrap();
                let key = OsString::from_str(key).unwrap();
                let value = OsString::from_str(value).unwrap();
                (key, value)
            })
            .collect::<Vec<_>>()
    });

    std::fs::create_dir_all("tests/programs/obj").unwrap();
    std::fs::create_dir_all("tests/programs/bin").unwrap();
    let command_output = Command::new("cl")
        .envs(environment_variables.clone())
        .arg(source_file_path)
        .arg("user32.lib")
        .arg("winmm.lib")
        .args(["/Fo:", "tests/programs/obj/"])
        .args(["/Fe:", "tests/programs/bin/"])
        .output()
        .unwrap();
    print!("{}", String::from_utf8_lossy(&command_output.stdout));
    eprint!("{}", String::from_utf8_lossy(&command_output.stderr));
    assert!(command_output.status.success());

    binary_file_path
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
    let mut conductor = winter::Conductor::new(
        executable_path.as_ref().to_str().unwrap(),
        Some(stdout_callback),
    )?;
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
    let stdout = run_and_get_stdout(build("stdout"), &[])?;
    assert_eq!(stdout, vec![b"abcABC123!\"_\x99\xaa\xbb"]);
    Ok(())
}

#[test]
fn get_tick_count() -> Result<()> {
    init_test();
    let stdout = run_and_get_stdout(
        build("get_tick_count"),
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
        build("get_tick_count_and_sleep"),
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
        build("get_tick_count_64"),
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
        build("get_tick_count_64_and_sleep"),
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
        build("time_get_time"),
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
        build("time_get_time_and_sleep"),
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
        build("query_performance_counter"),
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
        build("query_performance_counter_and_sleep"),
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

fn helper_for_key_state_tests(executable_path: impl AsRef<Path>) -> Result<()> {
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
    helper_for_key_state_tests(build("get_key_state"))
}

#[test]
fn get_async_key_state() -> Result<()> {
    helper_for_key_state_tests(build("get_async_key_state"))
}

#[test]
fn get_keyboard_state() -> Result<()> {
    helper_for_key_state_tests(build("get_keyboard_state"))
}

#[test]
fn key_down_and_key_up() -> Result<()> {
    fn key_event(id: u8, state: bool) -> Event {
        Event::SetKeyState { id, state }
    }

    init_test();
    let stdout = run_and_get_stdout(
        build("key_down_and_key_up"),
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
