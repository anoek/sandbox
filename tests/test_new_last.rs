mod fixtures;

use anyhow::Result;
use fixtures::*;
use rstest::*;

#[rstest]
fn test_new_last_mutual_exclusivity(mut sandbox: SandboxManager) -> Result<()> {
    sandbox.no_default_options = true;

    assert!(sandbox.xfail(&["--new", "--last", "ls"]));
    assert!(sandbox.xfail(&["--name=foo", "--last", "ls"]));
    assert!(sandbox.xfail(&["--name=foo", "--new", "ls"]));
    assert!(sandbox.xfail(&["--name=foo", "--new", "--last", "ls"]));
    assert!(sandbox.exfail(&["--new", "ls"], "SANDBOX_NAME", "foo"));
    assert!(sandbox.exfail(&["--last", "ls"], "SANDBOX_NAME", "foo"));

    Ok(())
}

#[rstest]
fn test_new_last_name_with_ms(mut sandbox: SandboxManager) -> Result<()> {
    sandbox.no_default_options = true;

    for _ in 0..6 {
        // We spin here to test to ensure we've exercised the code that
        // dedupes names at the ms level. 6 times in case this test case
        // is crossing a second-boundary (the clock second is rolling over that is)

        assert!(sandbox.epass(
            &["--new", "-v", "true"],
            "TEST_NAME_WITH_MS",
            "sandbox-coverage-new"
        ));
    }

    Ok(())
}

#[rstest]
fn test_last_is_empty(mut sandbox: SandboxManager) -> Result<()> {
    sandbox.no_default_options = true;

    assert!(sandbox.exfail(
        &["--last", "-v", "true"],
        "TEST_LAST_IS_EMPTY",
        "1"
    ));
    assert!(sandbox.last_stderr.contains("No sandboxes found"));

    Ok(())
}
