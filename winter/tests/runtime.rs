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
    for (range, (numerator, denominator)) in [
        (0..=98, (0, 60)),
        (100..=198, (1, 60)),
        (199..=199, (2, 60)),
    ] {
        for (counter_value_index, counter_value) in counter_values[range].iter().enumerate() {
            assert_eq!(
                *counter_value,
                frequency_values[0] * numerator / denominator,
                "unexpected value at index {counter_value_index}"
            );
        }
    }

    Ok(())
}
