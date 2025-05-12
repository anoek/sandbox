mod fixtures;

use nix::unistd::chdir;
use std::{path::Path, process::Command};

use anyhow::Result;
use fixtures::*;
use rstest::*;

#[rstest]
fn test_exercise_unmovable_files(mut sandbox: SandboxManager) -> Result<()> {
    sandbox.run(&["true"])?;
    sandbox.run(&["config", "sandbox_dir"])?;
    let base = sandbox.last_stdout.trim();
    let upper_dir = Path::new(base).join("upper");
    // F4 = /
    let char_device = upper_dir.join("F4").join("char-device");
    println!("setting up char_device: {}", char_device.display());
    let output = Command::new("sudo")
        .args(["mknod", char_device.to_str().unwrap(), "c", "1", "3"])
        .output()?;
    assert!(output.status.success());

    sandbox.run(&["status", "/"])?;
    let status = sandbox.last_stdout.trim();
    assert!(status.contains("!"));
    assert!(status.contains("char-device"));

    let cwd = std::env::current_dir()?;
    chdir("/")?;
    assert!(sandbox.xfail(&["accept", "/char-device"]));
    chdir(&cwd)?; // restore for cleanup
    let status = sandbox.last_stderr.trim();
    assert!(status.contains("Unsupported file type"));

    Ok(())
}
