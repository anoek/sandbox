mod fixtures;

use anyhow::Result;
use fixtures::*;
use rstest::*;

#[rstest]
fn test_pivot_root_failure(mut sandbox: SandboxManager) -> Result<()> {
    assert!(sandbox.exfail(&["true"], "TEST_PIVOT_ROOT_FAILURE", "1"));
    assert!(sandbox.last_stderr.contains("failed to pivot_root"));
    Ok(())
}
