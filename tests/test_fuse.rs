mod fixtures;

use anyhow::Result;
use fixtures::*;
use rstest::*;

#[rstest]
fn test_disable_fuse(mut sandbox: SandboxManager) -> Result<()> {
    assert!(sandbox.xfail(&["--bind-fuse=false", "ls", "-l", "/dev/fuse"]));
    Ok(())
}
