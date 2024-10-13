use anyhow::Result;
use std::time::Duration;
use test_utilities::{init_test, Architecture, Event, Instance};
use test_utilities_macros::test_for;

async fn test_helper(program_name: impl AsRef<str>, architecture: Architecture) -> Result<()> {
    init_test();
    let stdout = Instance::new(program_name.as_ref(), architecture)
        .with_events([
            Event::AdvanceTime(Duration::from_millis(1)),
            Event::SaveState,
            Event::AdvanceTime(Duration::from_millis(2)),
            Event::LoadState,
            Event::AdvanceTime(Duration::from_millis(1)),
            Event::LoadState,
            Event::AdvanceTime(Duration::from_millis(4)),
        ])
        .stdout_by_instant_from_utf8_lossy()
        .await?;

    assert_eq!(stdout, ["0", "1", "23", "2", "234"]);
    Ok(())
}

#[test_for(architecture)]
async fn stack_memory(architecture: Architecture) -> Result<()> {
    test_helper("saved_state/stack_memory", architecture).await
}

#[test_for(architecture)]
async fn allocated_memory(architecture: Architecture) -> Result<()> {
    test_helper("saved_state/allocated_memory", architecture).await
}

#[test_for(architecture)]
async fn pipe(architecture: Architecture) -> Result<()> {
    test_helper("saved_state/pipe", architecture).await
}
