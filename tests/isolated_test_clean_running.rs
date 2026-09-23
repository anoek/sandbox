mod fixtures;

use anyhow::Result;
use fixtures::*;
use rstest::*;

/* This test runs `clean` against the default storage directory, so it will
 * also delete any real stopped ephemeral sandboxes older than a week. */

#[rstest]
#[ignore = "Ignoring as this will delete old non test ephemeral sandboxes"]
fn test_clean_deletes_running_test_sandboxes() -> Result<()> {
    let mut sandbox = sandbox();
    let name = sandbox.name.clone();

    sandbox.run(&["true"])?;
    sandbox.run(&["list"])?;
    assert!(sandbox.last_stdout.contains("Running sandboxes:"));
    assert!(sandbox.last_stdout.contains(&name));

    sandbox.run(&["clean", "-y"])?;
    assert!(sandbox.last_stdout.contains(&name));

    sandbox.run(&["list"])?;
    assert!(!sandbox.last_stdout.contains(&name));

    Ok(())
}
