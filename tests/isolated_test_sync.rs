mod fixtures;

use anyhow::{Context, Result};
use fixtures::*;
use rstest::*;
use std::path::PathBuf;

#[rstest]
fn test_sync(mut sandbox: SandboxManager) -> Result<()> {
    sandbox.run(&["true"])?;
    sandbox.run(&["config", "upper_cwd"])?;
    let base = sandbox.last_stdout.trim();
    let upper_dir = PathBuf::from(base);

    let t = sandbox.test_filename("test-file");
    std::fs::write(&t, "hello")?;

    sandbox.run(&[
        "bash",
        "-c",
        format!("echo -n 'from sandbox echo' > {}", t.as_str()).as_str(),
    ])?;

    std::fs::write(&t, "goodbye")?;

    sandbox.run(&["cat", t.clone().as_str()])?;
    let stdout = sandbox.last_stdout.trim();
    assert_eq!(stdout, "from sandbox echo");

    // remove the file from the upper directory
    std::fs::remove_file(upper_dir.join(&t)).context(format!(
        "failed to remove file: {} [upper={}]",
        t,
        upper_dir.display()
    ))?;
    assert!(PathBuf::from(&t).exists());

    sandbox.run(&["sync"])?;

    // after a sync, programs run in the sandbox should see the changes. Before, the results are
    // undefined.
    sandbox.run(&["cat", t.clone().as_str()])?;
    let stdout = sandbox.last_stdout.trim();
    assert_eq!(stdout, "goodbye");

    Ok(())
}
