#![allow(non_snake_case)]

use anyhow::Result;
use std::time::Duration;
use test_utilities::{init_test, Architecture, Event, Instance};
use test_utilities_macros::test_for;

#[test_for(architecture)]
async fn GetTickCount(architecture: Architecture) -> Result<()> {
    init_test();
    let stdout = Instance::new("hooks/time/GetTickCount", architecture)
        .with_events(
            [
                &Event::AdvanceTime(Duration::from_millis(78)),
                &Event::AdvanceTime(Duration::from_millis(1)),
            ]
            .repeat(10)
            .into_iter()
            .cloned(),
        )
        .stdout_by_instant_from_utf8_lossy()
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
async fn GetTickCount_busy_wait(architecture: Architecture) -> Result<()> {
    init_test();
    let stdout = Instance::new("hooks/time/GetTickCount_busy_wait", architecture)
        .with_events([
            Event::AdvanceTime(Duration::from_secs_f64(1.0 / 60.0)),
            Event::AdvanceTime(Duration::from_secs_f64(1.0 / 30.0)),
        ])
        .stdout_by_instant_from_utf8_lossy()
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
async fn GetTickCount64(architecture: Architecture) -> Result<()> {
    init_test();
    let stdout = Instance::new("hooks/time/GetTickCount64", architecture)
        .with_events(
            [
                &Event::AdvanceTime(Duration::from_millis(206)),
                &Event::AdvanceTime(Duration::from_millis(1)),
            ]
            .repeat(10)
            .into_iter()
            .cloned(),
        )
        .stdout_by_instant_from_utf8_lossy()
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
async fn GetTickCount64_busy_wait(architecture: Architecture) -> Result<()> {
    init_test();
    let stdout = Instance::new("hooks/time/GetTickCount64_busy_wait", architecture)
        .with_events([
            Event::AdvanceTime(Duration::from_secs_f64(0.1)),
            Event::AdvanceTime(Duration::from_secs_f64(0.2)),
        ])
        .stdout_by_instant_from_utf8_lossy()
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
async fn timeGetTime(architecture: Architecture) -> Result<()> {
    init_test();
    let stdout = Instance::new("hooks/time/timeGetTime", architecture)
        .with_events(
            [
                &Event::AdvanceTime(Duration::from_millis(40)),
                &Event::AdvanceTime(Duration::from_millis(1)),
            ]
            .repeat(10)
            .into_iter()
            .cloned(),
        )
        .stdout_by_instant_from_utf8_lossy()
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
async fn timeGetTime_busy_wait(architecture: Architecture) -> Result<()> {
    init_test();
    let stdout = Instance::new("hooks/time/timeGetTime_busy_wait", architecture)
        .with_events([
            Event::AdvanceTime(Duration::from_secs_f64(100.0)),
            Event::AdvanceTime(Duration::from_secs_f64(0.001)),
        ])
        .stdout_by_instant_from_utf8_lossy()
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
async fn GetSystemTimeAsFileTime(architecture: Architecture) -> Result<()> {
    init_test();
    let stdout = Instance::new("hooks/time/GetSystemTimeAsFileTime", architecture)
        .with_events(
            [
                &Event::AdvanceTime(Duration::from_millis(192)),
                &Event::AdvanceTime(Duration::from_millis(1)),
            ]
            .repeat(10)
            .into_iter()
            .cloned(),
        )
        .stdout_by_instant_from_utf8_lossy()
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
async fn GetSystemTimeAsFileTime_busy_wait(architecture: Architecture) -> Result<()> {
    init_test();
    let stdout = Instance::new("hooks/time/GetSystemTimeAsFileTime_busy_wait", architecture)
        .with_events([
            Event::AdvanceTime(Duration::from_secs_f64(2.0 / 3.0)),
            Event::AdvanceTime(Duration::from_secs_f64(1.0 / 3.0)),
        ])
        .stdout_by_instant_from_utf8_lossy()
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
async fn GetSystemTimePreciseAsFileTime(architecture: Architecture) -> Result<()> {
    init_test();
    let stdout = Instance::new("hooks/time/GetSystemTimePreciseAsFileTime", architecture)
        .with_events(
            [
                &Event::AdvanceTime(Duration::from_millis(6)),
                &Event::AdvanceTime(Duration::from_millis(1)),
            ]
            .repeat(10)
            .into_iter()
            .cloned(),
        )
        .stdout_by_instant_from_utf8_lossy()
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
async fn GetSystemTimePreciseAsFileTime_busy_wait(architecture: Architecture) -> Result<()> {
    init_test();
    let stdout = Instance::new(
        "hooks/time/GetSystemTimePreciseAsFileTime_busy_wait",
        architecture,
    )
    .with_events([
        Event::AdvanceTime(Duration::from_secs_f64(2.0 / 5.0)),
        Event::AdvanceTime(Duration::from_secs_f64(17.0 / 100.0)),
    ])
    .stdout_by_instant_from_utf8_lossy()
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
async fn QueryPerformanceCounter(architecture: Architecture) -> Result<()> {
    init_test();
    let stdout = Instance::new("hooks/time/QueryPerformanceCounter", architecture)
        .with_events(
            [
                &Event::AdvanceTime(Duration::from_millis(46)),
                &Event::AdvanceTime(Duration::from_millis(1)),
            ]
            .repeat(10)
            .into_iter()
            .cloned(),
        )
        .stdout_by_instant_from_utf8_lossy()
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

#[test_for(architecture)]
async fn QueryPerformanceCounter_busy_wait(architecture: Architecture) -> Result<()> {
    init_test();
    let stdout = Instance::new("hooks/time/QueryPerformanceCounter_busy_wait", architecture)
        .with_events([
            Event::AdvanceTime(Duration::from_secs_f64(1.0 / 25.0)),
            Event::AdvanceTime(Duration::from_secs_f64(1.0 / 50.0)),
        ])
        .stdout_by_instant_from_utf8_lossy()
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

#[test_for(architecture, unicode)]
async fn waitable_timer(architecture: Architecture, unicode: bool) -> Result<()> {
    init_test();
    let stdout = Instance::new("hooks/time/waitable_timer", architecture)
        .with_unicode_flag(unicode)
        .with_events([
            Event::AdvanceTime(Duration::from_millis(1)),
            Event::AdvanceTime(Duration::from_millis(4)),
            Event::AdvanceTime(Duration::from_millis(9)),
            Event::AdvanceTime(Duration::from_millis(16)),
            Event::AdvanceTime(Duration::from_millis(25)),
            Event::AdvanceTime(Duration::from_millis(67)),
        ])
        .stdout_from_utf8_lossy()
        .await?;
    assert_eq!(
        stdout,
        concat!(
            "12 0\r\n",
            "27 0\r\n",
            "36 0\r\n",
            "39 0\r\n",
            "42 0\r\n",
            "\r\n",
            "50 1\r\n",
            "59 1\r\n",
            "60 1\r\n",
            "61 1\r\n",
            "62 1\r\n",
            "\r\n",
            "72 0\r\n",
            "74 0\r\n",
            "82 0\r\n",
            "89 0\r\n",
            "96 0\r\n",
            "103 0\r\n",
            "\r\n",
            "112 1\r\n",
            "117 1\r\n",
            "121 1\r\n",
            "122 1\r\n",
            "\r\n",
        )
    );
    Ok(())
}
