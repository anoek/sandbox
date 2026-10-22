mod fixtures;

use anyhow::Result;
use fixtures::*;
use rstest::*;
use std::fs;

/// Creates a directory to bind and returns its path
fn make_dir(sandbox: &SandboxManager, prefix: &str) -> Result<String> {
    let dir = sandbox.test_filename(prefix);
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Writes a config file with the given contents and returns its path
fn write_config(
    sandbox: &SandboxManager,
    prefix: &str,
    contents: &str,
) -> Result<String> {
    let path = format!("{}.toml", sandbox.test_filename(prefix));
    fs::write(&path, contents)?;
    Ok(path)
}

#[rstest]
fn ai_test_bind_higher_precedence_config_overrides(
    mut sandbox: SandboxManager,
) -> Result<()> {
    let dir = make_dir(&sandbox, "bind_dir")?;
    let low = write_config(&sandbox, "low", &format!("bind = [\"{dir}\"]"))?;
    let high =
        write_config(&sandbox, "high", &format!("bind = [\"{dir}::ro\"]"))?;

    // Later --config files take precedence over earlier ones
    sandbox.run(&[
        "--no-default-binds",
        "--config",
        &low,
        "--config",
        &high,
        "config",
        "bind",
    ])?;
    assert_eq!(sandbox.last_stdout.trim(), format!("{dir}::ro"));

    sandbox.run(&[
        "--no-default-binds",
        "--config",
        &high,
        "--config",
        &low,
        "config",
        "bind",
    ])?;
    assert_eq!(sandbox.last_stdout.trim(), dir);

    Ok(())
}

#[rstest]
fn ai_test_bind_conflict_within_config_fails(
    mut sandbox: SandboxManager,
) -> Result<()> {
    let dir = make_dir(&sandbox, "bind_dir")?;
    let conflicting = write_config(
        &sandbox,
        "conflicting",
        &format!("bind = [\"{dir}\", \"{dir}::ro\"]"),
    )?;
    let overriding =
        write_config(&sandbox, "overriding", &format!("bind = [\"{dir}\"]"))?;

    assert!(sandbox.xfail(&["--config", &conflicting, "config"]));
    assert!(sandbox.last_stderr.contains("Conflicting bind mount"));

    // A higher precedence config does not excuse a conflict in a lower one
    assert!(sandbox.xfail(&[
        "--config",
        &conflicting,
        "--config",
        &overriding,
        "config"
    ]));

    // Bind and mask for the same target in one config also conflict
    let bind_and_mask = write_config(
        &sandbox,
        "bind_and_mask",
        &format!("bind = [\"{dir}\"]\nmask = [\"{dir}\"]"),
    )?;
    assert!(sandbox.xfail(&["--config", &bind_and_mask, "config"]));

    Ok(())
}

#[rstest]
fn ai_test_bind_identical_within_config_is_deduplicated(
    mut sandbox: SandboxManager,
) -> Result<()> {
    let dir = make_dir(&sandbox, "bind_dir")?;
    let config = write_config(
        &sandbox,
        "duplicate",
        &format!("bind = [\"{dir}\", \"{dir}:{dir}:rw\"]"),
    )?;

    sandbox.run(&[
        "--no-default-binds",
        "--config",
        &config,
        "config",
        "bind",
    ])?;
    assert_eq!(sandbox.last_stdout.trim(), dir);

    Ok(())
}

#[rstest]
fn ai_test_mask_in_higher_config_overrides_bind(
    mut sandbox: SandboxManager,
) -> Result<()> {
    let dir = make_dir(&sandbox, "bind_dir")?;
    let low = write_config(&sandbox, "low", &format!("bind = [\"{dir}\"]"))?;
    let high = write_config(&sandbox, "high", &format!("mask = [\"{dir}\"]"))?;

    sandbox.run(&[
        "--no-default-binds",
        "--config",
        &low,
        "--config",
        &high,
        "config",
        "bind",
        "mask",
    ])?;
    assert!(sandbox.last_stdout.contains("bind=\n"));
    let target = fs::canonicalize(&dir)?;
    assert!(
        sandbox
            .last_stdout
            .contains(&format!("mask={}\n", target.display()))
    );

    Ok(())
}

#[rstest]
fn ai_test_bind_env_overrides_config(
    mut sandbox: SandboxManager,
) -> Result<()> {
    let dir = make_dir(&sandbox, "bind_dir")?;
    let config =
        write_config(&sandbox, "config", &format!("bind = [\"{dir}\"]"))?;

    sandbox.run_with_env(
        &["--no-default-binds", "--config", &config, "config", "bind"],
        "SANDBOX_BIND",
        &format!("{dir}::ro"),
    )?;
    assert_eq!(sandbox.last_stdout.trim(), format!("{dir}::ro"));

    // Conflicts within the environment layer still fail
    assert!(sandbox.exfail(
        &["--no-default-binds", "config"],
        "SANDBOX_BIND",
        &format!("{dir},{dir}::ro"),
    ));

    Ok(())
}

#[rstest]
fn ai_test_bind_cli_overrides_config_and_env(
    mut sandbox: SandboxManager,
) -> Result<()> {
    let dir = make_dir(&sandbox, "bind_dir")?;
    let config =
        write_config(&sandbox, "config", &format!("bind = [\"{dir}::ro\"]"))?;

    sandbox.run_with_env(
        &[
            "--no-default-binds",
            "--config",
            &config,
            "--bind",
            &dir,
            "config",
            "bind",
        ],
        "SANDBOX_BIND",
        &format!("{dir}::ro"),
    )?;
    assert_eq!(sandbox.last_stdout.trim(), dir);

    // A CLI mask overrides a bind from a config file
    sandbox.run(&[
        "--no-default-binds",
        "--config",
        &config,
        "--mask",
        &dir,
        "config",
        "bind",
        "mask",
    ])?;
    assert!(sandbox.last_stdout.contains("bind=\n"));
    let target = fs::canonicalize(&dir)?;
    assert!(
        sandbox
            .last_stdout
            .contains(&format!("mask={}\n", target.display()))
    );

    Ok(())
}

#[rstest]
fn ai_test_bind_overrides_default_binds(
    mut sandbox: SandboxManager,
) -> Result<()> {
    // /dev/fuse is a default bind when present on the host
    if !std::path::Path::new("/dev/fuse").exists() {
        return Ok(());
    }

    sandbox.run(&[
        "--no-config",
        "--bind",
        "/dev/fuse::ro",
        "config",
        "bind",
    ])?;
    let binds: Vec<&str> = sandbox.last_stdout.trim().split(',').collect();
    assert!(binds.contains(&"/dev/fuse::ro"));
    assert!(!binds.contains(&"/dev/fuse"));

    Ok(())
}
