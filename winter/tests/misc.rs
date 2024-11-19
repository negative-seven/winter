#![allow(non_snake_case)]

use anyhow::Result;
use test_utilities::{init_test, Architecture, Instance};
use test_utilities_macros::test_for;

#[test_for(architecture, unicode)]
async fn GetCommandLine(architecture: Architecture, unicode: bool) -> Result<()> {
    init_test();
    let stdout = Instance::new("hooks/misc/GetCommandLine", architecture)
        .with_unicode_flag(unicode)
        .with_command_line_string("abcABC123!\"_".into())
        .stdout()
        .await?;
    assert_eq!(stdout, b"abcABC123!\"_");
    Ok(())
}

#[test_for(architecture)]
async fn NtSetInformationThread(architecture: Architecture) -> Result<()> {
    init_test();
    let stdout = Instance::new("hooks/misc/NtSetInformationThread", architecture)
        .stdout_from_utf8_lossy()
        .await?;
    assert_eq!(stdout, "start\r\nbreakpoint\r\nend\r\n");
    Ok(())
}
