use anyhow::Result;
use test_utilities::{init_test, Architecture, Instance};
use test_utilities_macros::test_for;

#[test_for(architecture)]
async fn stdout(architecture: Architecture) -> Result<()> {
    init_test();
    let stdout = Instance::new("stdout", architecture).stdout().await?;
    assert_eq!(stdout, b"abcABC123!\"_\x99\xaa\xbb");
    Ok(())
}

#[test_for(architecture)]
async fn stdout_large(architecture: Architecture) -> Result<()> {
    init_test();
    let stdout = Instance::new("stdout_large", architecture).stdout().await?;
    assert_eq!(stdout.len(), 1024 * 1024 - 1);
    assert!(stdout.iter().all(|&byte| byte == b's'));
    Ok(())
}
