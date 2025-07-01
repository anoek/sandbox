mod fixtures;

use std::{fs::create_dir, path::Path, process::Command};

use anyhow::Result;
use fixtures::*;
use rstest::*;
use serde_json::Value;

#[rstest]
fn test_program_runs_with_no_args(mut sandbox: SandboxManager) -> Result<()> {
    sandbox.run(&["env"])?;
    sandbox.run(&["status"])?;
    sandbox.run(&["touch", "testfile"])?;
    sandbox.run(&["status"])?;
    sandbox.run(&["list"])?;
    sandbox.run(&["diff"])?;
    sandbox.run(&[])?; // should launch bash, which should exit without an input terminal
    Ok(())
}

#[rstest]
fn test_weird_sudo_env_variables(sandbox: SandboxManager) -> Result<()> {
    let sandbox_bin = sandbox.sandbox_bin.clone();

    let mut cmd = Command::new("sudo");
    cmd.args(["-E", "SUDO_UID=nan", &sandbox_bin]);
    cmd.args([format!("--name={}", &sandbox.name)]);
    let output = cmd.output()?;
    assert!(output.status.code().unwrap() != 0);
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("Failed to parse SUDO_UID")
    );

    let mut cmd = Command::new("sudo");
    cmd.args(["-E", "SUDO_GID=nan", &sandbox_bin]);
    cmd.args([format!("--name={}", &sandbox.name)]);
    let output = cmd.output()?;
    assert!(output.status.code().unwrap() != 0);
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("Failed to parse SUDO_GID")
    );

    let mut cmd = Command::new("sudo");
    cmd.args(["-E", "SUDO_HOME=", &sandbox_bin]);
    cmd.args([format!("--name={}", &sandbox.name)]);
    let output = cmd.output()?;
    assert!(output.status.code().unwrap() != 0);
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("Home directory is not absolute")
    );

    let mut cmd = Command::new("sudo");
    cmd.args(["-E", "SUDO_HOME=~/foobar", &sandbox_bin]);
    cmd.args([format!("--name={}", &sandbox.name)]);
    let output = cmd.output()?;
    assert!(output.status.code().unwrap() != 0);

    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("Home directory is not absolute")
    );

    let mut cmd = Command::new("sudo");
    cmd.args(["-E", "SUDO_HOME=/invalid-home-directory", &sandbox_bin]);
    cmd.args([format!("--name={}", &sandbox.name)]);
    let output = cmd.output()?;
    assert!(output.status.code().unwrap() != 0);

    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("Home directory does not exist")
    );

    /* Spaces aren't supported (yet?) - need to figure if we can escape them in the mount options
     * string or not */
    let dir_with_spaces = "/tmp/dir with spaces";
    let dir_with_spaces = Path::new(&dir_with_spaces);
    if !dir_with_spaces.exists() {
        create_dir(dir_with_spaces)?;
    }
    let absolute_dir_with_spaces = dir_with_spaces.canonicalize()?;
    println!(
        "Absolute dir with spaces: {}",
        absolute_dir_with_spaces.display()
    );

    let mut cmd = Command::new("sudo");
    cmd.args([
        "-E",
        format!("SANDBOX_STORAGE_DIR={}", absolute_dir_with_spaces.display())
            .as_str(),
        &sandbox_bin,
    ]);
    cmd.args([format!("--name={}", &sandbox.name)]);
    cmd.args(["true"]);
    println!("cmd: {:?}", cmd);
    let output = cmd.output()?;
    println!("output: {}", String::from_utf8_lossy(&output.stdout));
    println!("stderr: {}", String::from_utf8_lossy(&output.stderr));
    println!("status: {}", output.status.code().unwrap());

    assert!(output.status.code().unwrap() != 0);
    println!("Error: {}", String::from_utf8_lossy(&output.stderr));
    println!("Output: {}", String::from_utf8_lossy(&output.stdout));

    assert!(String::from_utf8_lossy(&output.stderr).contains("Storage path"));
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("contains invalid character")
    );

    Ok(())
}

#[rstest]
fn test_setuid_works_like_sudo(mut sandbox: SandboxManager) -> Result<()> {
    let sudo_output = sandbox.run(&["whoami"])?;

    let sandbox_bin = sandbox.sandbox_bin.clone();
    let sandbox_setuid = sandbox_bin + "-setuid";

    let mut cmd = Command::new(&sandbox_setuid);
    cmd.args([format!("--name={}", &sandbox.name)]);
    cmd.args(["whoami"]);
    let setuid_output = cmd.output()?;

    assert_eq!(sudo_output.stdout, setuid_output.stdout);

    Ok(())
}

#[rstest]
fn test_sandbox_no_print_to_stdout_by_default(
    mut sandbox: SandboxManager,
) -> Result<()> {
    sandbox.run(&["echo", "-n", "Hello, world!"])?;
    assert_eq!(&sandbox.last_stdout, "Hello, world!");
    Ok(())
}

#[rstest]
fn test_sandbox_env_variable_is_set(mut sandbox: SandboxManager) -> Result<()> {
    let output = sandbox.run(&["sh", "-c", "echo -n $SANDBOX"])?;
    assert_eq!(String::from_utf8_lossy(&output.stdout), sandbox.name);
    Ok(())
}

#[rstest]
fn test_config(mut sandbox: SandboxManager) -> Result<()> {
    let output = sandbox.run(&["config", "name"])?;
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        format!("{}\n", sandbox.name)
    );

    let output = sandbox.run(&["config"])?;
    assert!(output.status.success());

    let output = sandbox.run(&["--json", "config"])?;
    assert!(output.status.success());
    let json: Value =
        serde_json::from_str(&String::from_utf8_lossy(&output.stdout))?;
    assert_eq!(json["name"], sandbox.name);

    assert!(sandbox.xfail(&["config", "non-existent-key"]));
    Ok(())
}

#[rstest]
fn test_diff(mut sandbox: SandboxManager) -> Result<()> {
    sandbox.run(&["touch", "testfile"])?;
    assert!(sandbox.epass(&["diff"], "TEST_FORCE_DIFF_COLOR", "true"));
    assert!(sandbox.xfail(&["--json", "diff"]));
    Ok(())
}

#[rstest]
fn test_shell_completion_generation(mut sandbox: SandboxManager) -> Result<()> {
    assert!(sandbox.epass(&[], "COMPLETE", "zsh"));
    assert!(sandbox.epass(&[], "COMPLETE", "bash"));
    assert!(sandbox.epass(&[], "COMPLETE", "fish"));
    assert!(sandbox.epass(&[], "COMPLETE", "elvish"));
    assert!(sandbox.exfail(&[], "COMPLETE", "invalid-shell"));
    Ok(())
}

/* This test doesn't really test much other than ensuring things don't explode
 * with large xattrs, it is here primarily to cover the code that handles growing
 * the buffer size in the is_renamed function. It'd be better if we tested it on
 * a real file but with the limits of filename sizes and redirect=off I think it's
 * hard to do, and the code is one line simple so meh. */
#[rstest]
fn exercise_large_xattr(mut sandbox: SandboxManager) -> Result<()> {
    sandbox.run(&["touch", "testfile"])?;
    let long_value = "a".repeat(300);

    let upper_cwd_output = sandbox.run(&["config", "upper_cwd"])?;
    let upper_cwd = String::from_utf8_lossy(&upper_cwd_output.stdout)
        .trim()
        .to_string();

    let host_file_path = std::path::Path::new(&upper_cwd).join("testfile");

    // Use setfattr as root to attach a large redirect xattr.
    use std::process::Command;
    let status = Command::new("sudo")
        .args([
            "setfattr",
            "-n",
            "trusted.overlay.redirect",
            "-v",
            &long_value,
            host_file_path.to_str().expect("path conversion failed"),
        ])
        .status()?;

    assert!(status.success());
    sandbox.run(&["status"])?;

    Ok(())
}
