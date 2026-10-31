// Tests for error and recovery paths that ordinary usage doesn't reach

mod fixtures;

use anyhow::Result;
use fixtures::*;
use rstest::*;
use std::fs;
use std::path::PathBuf;

#[rstest]
fn ai_test_bind_source_missing(mut sandbox: SandboxManager) -> Result<()> {
    let missing = format!("/does-not-exist-{}", rid());
    let bind = format!("--bind={}", missing);
    assert!(sandbox.xfail(&[&bind, "config", "bind"]));
    assert!(
        sandbox.last_stderr.contains(&format!(
            "Failed to canonicalize source path: {}",
            missing
        )),
        "{}",
        sandbox.last_stderr
    );
    Ok(())
}

#[rstest]
fn ai_test_config_file_unreadable(mut sandbox: SandboxManager) -> Result<()> {
    // The path exists, but it's a directory so it can't be read
    let dir = sandbox.test_filename("config-dir");
    fs::create_dir_all(&dir)?;
    assert!(sandbox.xfail(&["--config", &dir, "config"]));
    assert!(
        sandbox
            .last_stderr
            .contains(&format!("Failed to read config file {}", dir)),
        "{}",
        sandbox.last_stderr
    );
    Ok(())
}

#[rstest]
fn ai_test_new_with_unusable_storage_dir(
    mut sandbox: SandboxManager,
) -> Result<()> {
    // --new looks for an unused name in the storage directory, which fails
    // when the storage directory can't be used. --new conflicts with the
    // --name the fixture normally adds.
    sandbox.no_default_options = true;
    let storage_dir = format!("--storage-dir=/proc/sandbox-coverage-{}", rid());
    assert!(sandbox.xfail(&["--new", &storage_dir, "config", "name"]));
    assert!(
        sandbox
            .last_stderr
            .contains("Failed to find a suitable storage directory"),
        "{}",
        sandbox.last_stderr
    );
    Ok(())
}

#[rstest]
fn ai_test_running_sandbox_without_settings(
    mut sandbox: SandboxManager,
) -> Result<()> {
    sandbox.run(&["true"])?;

    // Sandboxes started before settings were recorded don't have a
    // settings file, so there is nothing to validate the config against
    sandbox.run(&["config", "sandbox_dir"])?;
    let settings =
        PathBuf::from(sandbox.last_stdout.trim()).join("data/settings.json");
    assert!(settings.exists());
    let removed = std::process::Command::new("sudo")
        .args(["rm", &settings.to_string_lossy()])
        .status()?;
    assert!(removed.success());

    sandbox.run(&["echo", "still running"])?;
    assert_eq!(sandbox.last_stdout.trim(), "still running");
    Ok(())
}
