mod fixtures;

use anyhow::Result;
use fixtures::*;
use rstest::*;

#[rstest]
fn test_create_old_root_host_path_failure(
    mut sandbox: SandboxManager,
) -> Result<()> {
    assert!(sandbox.exfail(
        &["true"],
        "TEST_CREATE_OLD_ROOT_HOST_PATH_FAILURE",
        "1"
    ));
    assert!(
        sandbox
            .last_stderr
            .contains("Failed to create place to pivot our old root to")
    );
    Ok(())
}
