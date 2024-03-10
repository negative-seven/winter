use anyhow::Result;
use std::{
    path::Path,
    process::Command,
    sync::{Arc, Mutex, Once},
};

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

fn run_and_get_stdout(executable_path: &str) -> Result<Vec<u8>> {
    let stdout = Arc::new(Mutex::new(Vec::new()));
    let stdout_callback = {
        let stdout = Arc::clone(&stdout);
        move |bytes: &_| {
            stdout.lock().unwrap().extend_from_slice(bytes);
        }
    };
    let mut runtime = winter::Runtime::new(executable_path, "hooks32.dll", Some(stdout_callback))?;
    runtime.resume()?;
    runtime.wait_until_exit()?;
    Ok(Arc::try_unwrap(stdout).unwrap().into_inner().unwrap())
}

#[test]
fn stdout() -> Result<()> {
    init_test();
    let stdout = run_and_get_stdout("tests/programs/bin/stdout.exe")?;
    assert_eq!(stdout, b"abcABC123!\"_\x99\xaa\xbb");
    Ok(())
}

#[test]
fn get_tick_count() -> Result<()> {
    init_test();
    let stdout = run_and_get_stdout("tests/programs/bin/get_tick_count.exe")?;
    let stdout = String::from_utf8_lossy(&stdout);

    let tick_values = stdout
        .lines()
        .map(str::parse::<u32>)
        .collect::<Result<Vec<_>, _>>()?;

    assert_eq!(tick_values.len(), 200);
    for (index, tick_value) in tick_values.iter().enumerate() {
        let expected_tick_value = match index {
            0..=98 => 0,
            99..=198 => 16,
            199 => 33,
            _ => panic!("index is outside expected bounds"),
        };
        assert_eq!(
            *tick_value, expected_tick_value,
            "unexpected value at index {index}"
        );
    }

    Ok(())
}

#[test]
fn get_tick_count_and_sleep() -> Result<()> {
    init_test();
    let stdout = run_and_get_stdout("tests/programs/bin/get_tick_count_and_sleep.exe")?;
    let stdout = String::from_utf8_lossy(&stdout);

    let tick_values = stdout
        .lines()
        .map(str::parse::<u32>)
        .collect::<Result<Vec<_>, _>>()?;

    assert_eq!(tick_values.len(), 10);
    #[allow(clippy::cast_possible_truncation)]
    for (index, tick_value) in tick_values.iter().enumerate() {
        assert_eq!(
            *tick_value,
            index as u32 * 79,
            "unexpected value at index {index}"
        );
    }

    Ok(())
}

#[test]
fn time_get_time() -> Result<()> {
    init_test();
    let stdout = run_and_get_stdout("tests/programs/bin/time_get_time.exe")?;
    let stdout = String::from_utf8_lossy(&stdout);

    let tick_values = stdout
        .lines()
        .map(str::parse::<u32>)
        .collect::<Result<Vec<_>, _>>()?;

    assert_eq!(tick_values.len(), 200);
    for (index, tick_value) in tick_values.iter().enumerate() {
        let expected_tick_value = match index {
            0..=98 => 0,
            99..=198 => 16,
            199 => 33,
            _ => panic!("index is outside expected bounds"),
        };
        assert_eq!(
            *tick_value, expected_tick_value,
            "unexpected value at index {index}"
        );
    }

    Ok(())
}

#[test]
fn time_get_time_and_sleep() -> Result<()> {
    init_test();
    let stdout = run_and_get_stdout("tests/programs/bin/time_get_time_and_sleep.exe")?;
    let stdout = String::from_utf8_lossy(&stdout);

    let tick_values = stdout
        .lines()
        .map(str::parse::<u32>)
        .collect::<Result<Vec<_>, _>>()?;

    assert_eq!(tick_values.len(), 10);
    #[allow(clippy::cast_possible_truncation)]
    for (index, tick_value) in tick_values.iter().enumerate() {
        assert_eq!(
            *tick_value,
            index as u32 * 41,
            "unexpected value at index {index}"
        );
    }

    Ok(())
}

#[test]
fn query_performance_counter() -> Result<()> {
    init_test();
    let stdout = run_and_get_stdout("tests/programs/bin/query_performance_counter.exe")?;
    let stdout = String::from_utf8_lossy(&stdout);

    let mut counter_values = Vec::new();
    let mut frequency_values = Vec::new();
    for line in stdout.lines() {
        let (counter_value_str, frequency_value_str) =
            line.split_once('/').expect("could not split output by '/'");
        counter_values.push(str::parse::<u64>(counter_value_str)?);
        frequency_values.push(str::parse::<u64>(frequency_value_str)?);
    }

    assert_eq!(counter_values.len(), 200);
    assert_eq!(frequency_values.len(), 200);
    assert!(frequency_values.iter().all(|v| *v == frequency_values[0]));
    for (index, counter_value) in counter_values.iter().enumerate() {
        let expected_counter_value = {
            let (numerator, denominator) = match index {
                0..=98 => (0, 60),
                99..=198 => (1, 60),
                199 => (2, 60),
                _ => panic!("index is outside expected bounds"),
            };
            frequency_values[0] * numerator / denominator
        };
        assert_eq!(
            *counter_value, expected_counter_value,
            "unexpected value at index {index}"
        );
    }

    Ok(())
}

#[test]
fn query_performance_counter_and_sleep() -> Result<()> {
    init_test();
    let stdout = run_and_get_stdout("tests/programs/bin/query_performance_counter_and_sleep.exe")?;
    let stdout = String::from_utf8_lossy(&stdout);

    let mut counter_values = Vec::new();
    let mut frequency_values = Vec::new();
    for line in stdout.lines() {
        let (counter_value_str, frequency_value_str) =
            line.split_once('/').expect("could not split output by '/'");
        counter_values.push(str::parse::<u64>(counter_value_str)?);
        frequency_values.push(str::parse::<u64>(frequency_value_str)?);
    }

    assert_eq!(counter_values.len(), 10);
    assert_eq!(frequency_values.len(), 10);
    assert!(frequency_values.iter().all(|v| *v == frequency_values[0]));
    for (index, (counter_value, frequency_value)) in
        counter_values.iter().zip(&frequency_values).enumerate()
    {
        assert_eq!(
            *counter_value,
            frequency_value * (index as u64) * 47 / 1000,
            "unexpected value at index {index}"
        );
    }

    Ok(())
}
