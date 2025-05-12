mod fixtures;

use anyhow::Result;
use fixtures::*;
use rstest::*;
use std::{path::Path, process::Command};

/* This test needs to be the only test in this file since we chdir
 * and tests are run in parallel per file. */

#[rstest]
fn test_accept_root_file(mut sandbox: SandboxManager) -> Result<()> {
    let dirname = "/delete-me-this-is-a-sandbox-coverage-test-dir";
    let filename =
        dirname.to_string() + "/delete-me-this-is-a-sandbox-coverage-test-file";
    Command::new("sudo").arg("mkdir").arg(dirname).output()?;
    Command::new("sudo").arg("touch").arg(&filename).output()?;
    Command::new("sudo")
        .arg("chmod")
        .arg("666")
        .arg(&filename)
        .output()?;
    Command::new("sudo")
        .arg("chmod")
        .arg("777")
        .arg(dirname)
        .output()?;
    let path = Path::new(&filename);
    sandbox.run(&["rm", &filename])?;
    assert!(path.exists());

    sandbox.run(&["accept", &filename])?;
    assert!(!path.exists());

    Command::new("sudo").arg("rmdir").arg(dirname).output()?;
    Ok(())
}
