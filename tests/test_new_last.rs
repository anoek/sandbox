mod fixtures;

use anyhow::Result;
use fixtures::*;
use rstest::*;

#[rstest]
fn test_new_last() -> Result<()> {
    let mut sandbox1 = SandboxManager::new();
    let mut sandbox2 = SandboxManager::new();
    let mut sandbox3 = SandboxManager::new();

    sandbox1.no_default_options = true;
    sandbox2.no_default_options = true;
    sandbox3.no_default_options = true;

    assert!(sandbox1.pass(&["--new", "-v", "ls"]));
    assert!(sandbox1.pass(&["--last", "config", "name"]));
    let sandbox1_name = sandbox1.last_stdout.trim();
    sandbox1.name = sandbox1_name.to_string();

    assert!(sandbox2.pass(&["--new", "-v", "ls"]));
    assert!(sandbox2.pass(&["--last", "config", "name"]));
    let sandbox2_name = sandbox2.last_stdout.trim();
    sandbox2.name = sandbox2_name.to_string();

    assert!(sandbox3.pass(&["--new", "-v", "ls"]));
    assert!(sandbox3.pass(&["--last", "config", "name"]));
    let sandbox3_name = sandbox3.last_stdout.to_string();
    sandbox3.name = sandbox3_name.trim().to_string();

    assert!(sandbox3.pass(&["--last", "-v", "ls"]));
    assert!(sandbox3.pass(&["--last", "config", "name"]));
    let sandbox3_last_name = sandbox3.last_stdout.trim().to_string();

    assert_ne!(sandbox1_name, sandbox2_name);
    assert_ne!(sandbox1_name, sandbox3_name);
    assert_ne!(sandbox2_name, sandbox3_name);
    assert_eq!(sandbox3_name.trim(), sandbox3_last_name);

    Ok(())
}

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
