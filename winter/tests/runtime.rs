use anyhow::Result;
use std::{io::Read, process::Command, sync::Once};

#[allow(clippy::missing_panics_doc)]
pub fn init_test() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
            .init();

        assert!(Command::new("tests/programs/build.bat")
            .spawn()
            .unwrap()
            .wait()
            .unwrap()
            .success());
    });
}

#[test]
fn stdout() -> Result<()> {
    init_test();

    let mut runtime = winter::Runtime::new("tests/programs/bin/stdout.exe", "hooks32.dll")?;
    runtime.resume()?;
    runtime.wait_until_exit()?;
    let mut stdout = Vec::new();
    runtime.stdout_mut().read_to_end(&mut stdout)?;

    assert_eq!(
        stdout,
        "abcABC123!\"_"
            .bytes()
            .chain([0x99, 0xaa, 0xbb])
            .collect::<Vec<u8>>()
    );

    Ok(())
}

#[test]
fn get_tick_count() -> Result<()> {
    init_test();

    let mut runtime = winter::Runtime::new("tests/programs/bin/get_tick_count.exe", "hooks32.dll")?;
    runtime.resume()?;
    runtime.wait_until_exit()?;
    let mut stdout = String::new();
    runtime.stdout_mut().read_to_string(&mut stdout)?;

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

    let mut runtime = winter::Runtime::new(
        "tests/programs/bin/get_tick_count_and_sleep.exe",
        "hooks32.dll",
    )?;
    runtime.resume()?;
    runtime.wait_until_exit()?;
    let mut stdout = String::new();
    runtime.stdout_mut().read_to_string(&mut stdout)?;

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

    let mut runtime = winter::Runtime::new("tests/programs/bin/time_get_time.exe", "hooks32.dll")?;
    runtime.resume()?;
    runtime.wait_until_exit()?;
    let mut stdout = String::new();
    runtime.stdout_mut().read_to_string(&mut stdout)?;

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

    let mut runtime = winter::Runtime::new(
        "tests/programs/bin/time_get_time_and_sleep.exe",
        "hooks32.dll",
    )?;
    runtime.resume()?;
    runtime.wait_until_exit()?;
    let mut stdout = String::new();
    runtime.stdout_mut().read_to_string(&mut stdout)?;

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

    let mut runtime = winter::Runtime::new(
        "tests/programs/bin/query_performance_counter.exe",
        "hooks32.dll",
    )?;
    runtime.resume()?;
    runtime.wait_until_exit()?;
    let mut stdout = String::new();
    runtime.stdout_mut().read_to_string(&mut stdout)?;

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

    let mut runtime = winter::Runtime::new(
        "tests/programs/bin/query_performance_counter_and_sleep.exe",
        "hooks32.dll",
    )?;
    runtime.resume()?;
    runtime.wait_until_exit()?;
    let mut stdout = String::new();
    runtime.stdout_mut().read_to_string(&mut stdout)?;

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