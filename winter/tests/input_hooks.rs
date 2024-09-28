#![allow(non_snake_case)]

use anyhow::Result;
use std::time::Duration;
use test_utilities::{init_test, Architecture, Event, Instance};
use test_utilities_macros::test_for;

async fn test_helper(program_name: impl AsRef<str>, architecture: Architecture) -> Result<()> {
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
        .stdout_by_instant_from_utf8_lossy()
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
async fn GetKeyState(architecture: Architecture) -> Result<()> {
    test_helper("hooks/input/GetKeyState", architecture).await
}

#[test_for(architecture)]
async fn GetAsyncKeyState(architecture: Architecture) -> Result<()> {
    test_helper("hooks/input/GetAsyncKeyState", architecture).await
}
#[test_for(architecture)]
async fn GetKeyboardState(architecture: Architecture) -> Result<()> {
    test_helper("hooks/input/GetKeyboardState", architecture).await
}
