mod fixtures;

use anyhow::Result;
use fixtures::*;
use rstest::*;

#[rstest]
fn test_mask_no_mask(mut sandbox: SandboxManager) -> Result<()> {
    let test_dir = sandbox.test_filename("not_masked");
    std::fs::create_dir(&test_dir)?;
    let test_file = format!("{}/should-not-be-masked", test_dir);
    std::fs::write(&test_file, "test")?;

    sandbox.run(&["ls", "-l", &test_dir])?;
    println!("{}", sandbox.last_stdout);
    assert!(sandbox.last_stdout.contains("should-not-be-masked"));
    Ok(())
}

#[rstest]
fn test_mask_mask_cli(mut sandbox: SandboxManager) -> Result<()> {
    let test_dir = sandbox.test_filename("mask_from_env");
    std::fs::create_dir(&test_dir)?;
    let test_file = format!("{}/should-be-masked", test_dir);
    std::fs::write(&test_file, "test")?;
    sandbox.run(&["--mask", &test_dir, "ls", "-l", &test_dir])?;
    println!("{}", sandbox.last_stdout);
    assert!(!sandbox.last_stdout.contains("should-be-masked"));
    Ok(())
}

#[rstest]
fn test_mask_mask_cli_no_config(mut sandbox: SandboxManager) -> Result<()> {
    let test_dir = sandbox.test_filename("mask_from_env");
    std::fs::create_dir(&test_dir)?;
    let test_file = format!("{}/should-be-masked", test_dir);
    std::fs::write(&test_file, "test")?;
    sandbox.run(&[
        "--no-config",
        "--mask",
        &test_dir,
        "ls",
        "-l",
        &test_dir,
    ])?;
    println!("{}", sandbox.last_stdout);
    assert!(!sandbox.last_stdout.contains("should-be-masked"));
    Ok(())
}

#[rstest]
fn test_mask_env(mut sandbox: SandboxManager) -> Result<()> {
    let test_dir = sandbox.test_filename("mask_from_env");
    std::fs::create_dir(&test_dir)?;
    let test_file = format!("{}/should-be-masked", test_dir);
    std::fs::write(&test_file, "test")?;
    sandbox.run_with_env(
        &["--no-config", "ls", "-l", &test_dir],
        "SANDBOX_MASK",
        &test_dir,
    )?;
    println!("{}", sandbox.last_stdout);
    assert!(!sandbox.last_stdout.contains("should-be-masked"));
    Ok(())
}

#[rstest]
fn test_mask_empty_env(mut sandbox: SandboxManager) -> Result<()> {
    let test_dir = sandbox.test_filename("mask_from_env");
    std::fs::create_dir(&test_dir)?;
    let test_file = format!("{}/should-not-be-masked", test_dir);
    std::fs::write(&test_file, "test")?;
    sandbox.run_with_env(
        &["--no-config", "ls", "-l", &test_dir],
        "SANDBOX_MASK",
        "",
    )?;
    println!("{}", sandbox.last_stdout);
    assert!(sandbox.last_stdout.contains("should-not-be-masked"));
    Ok(())
}

#[rstest]
fn test_mask_bind_mask(mut sandbox: SandboxManager) -> Result<()> {
    let test_dir = sandbox.test_filename("mask_from_env");
    std::fs::create_dir(&test_dir)?;
    let test_file = format!("{}/should-be-masked", test_dir);
    std::fs::write(&test_file, "test")?;
    sandbox.run_with_env(
        &["ls", "-l", &test_dir],
        "SANDBOX_BIND",
        &format!("{}:{}:mask", test_dir, test_dir),
    )?;
    println!("{}", sandbox.last_stdout);
    assert!(!sandbox.last_stdout.contains("should-be-masked"));
    Ok(())
}

#[rstest]
fn test_mask_file(mut sandbox: SandboxManager) -> Result<()> {
    let test_dir = sandbox.test_filename("mask_from_env");
    std::fs::create_dir(&test_dir)?;
    let test_file = format!("{}/should-be-masked", test_dir);
    std::fs::write(&test_file, "test")?;
    sandbox.run_with_env(
        &["cat", &test_file],
        "SANDBOX_BIND",
        &format!("{}:{}:mask", test_file, test_file),
    )?;
    // file exists but is masked so won't contain the test we wrote
    assert!(!sandbox.last_stdout.contains("test"));

    sandbox.run_with_env(
        &["ls", &test_file],
        "SANDBOX_BIND",
        &format!("{}:{}:mask", test_file, test_file),
    )?;
    println!("{}", sandbox.last_stdout);
    // we should see it with ls
    assert!(sandbox.last_stdout.contains("should-be-masked"));

    Ok(())
}

#[rstest]
fn test_mask_combined(mut sandbox: SandboxManager) -> Result<()> {
    sandbox.run_with_env(
        &["--mask", "/tmp", "config"],
        "SANDBOX_MASK",
        "/run",
    )?;
    assert!(sandbox.last_stdout.contains("/tmp"));
    assert!(sandbox.last_stdout.contains("/run"));

    Ok(())
}

#[rstest]
fn test_bind_conflict(mut sandbox: SandboxManager) -> Result<()> {
    assert!(sandbox.xfail(&["--bind", "/tmp::ro,/tmp::rw"]));
    Ok(())
}
