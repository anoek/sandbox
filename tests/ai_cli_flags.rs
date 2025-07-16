mod fixtures;

use anyhow::Result;
use fixtures::*;
use rstest::*;

#[rstest]
fn test_flags_before_action(mut sandbox: SandboxManager) -> Result<()> {
    // Disable default -v flag to avoid conflicts
    sandbox.no_default_options = true;
    let name = sandbox.name.clone();

    // Test flags before action - traditional style
    assert!(sandbox.pass(&["--name", &name, "config"]));
    assert!(
        sandbox
            .last_stdout
            .contains(format!("name={}", name).as_str())
    );

    assert!(sandbox.pass(&["--name", &name, "--net=host", "config"]));
    assert!(sandbox.last_stdout.contains("net=host"));

    assert!(sandbox.pass(&["--name", &name, "-v", "config"]));
    assert!(sandbox.last_stdout.contains("log_level=TRACE"));

    assert!(sandbox.pass(&["--name", &name, "--log-level=debug", "config"]));
    assert!(sandbox.last_stdout.contains("log_level=DEBUG"));

    assert!(sandbox.pass(&["--name", &name, "--json", "config"]));
    // JSON output should be valid JSON
    assert!(
        serde_json::from_str::<serde_json::Value>(&sandbox.last_stdout).is_ok()
    );

    // Test a simple action without extra flags
    assert!(sandbox.pass(&["--name", &name, "status"]));

    Ok(())
}

#[rstest]
fn test_flags_after_action(mut sandbox: SandboxManager) -> Result<()> {
    // Disable default -v flag to avoid conflicts
    sandbox.no_default_options = true;
    let name = sandbox.name.clone();

    // Test flags after action - new flexible style
    assert!(sandbox.pass(&["config", "--name", "test-after"]));
    assert!(sandbox.last_stdout.contains("name=test-after"));

    assert!(sandbox.pass(&["--name", &name, "config", "--net=host"]));
    assert!(sandbox.last_stdout.contains("net=host"));

    assert!(sandbox.pass(&["--name", &name, "config", "-v"]));
    assert!(sandbox.last_stdout.contains("log_level=TRACE"));

    assert!(sandbox.pass(&["--name", &name, "config", "--log-level=debug"]));
    assert!(sandbox.last_stdout.contains("log_level=DEBUG"));

    assert!(sandbox.pass(&["--name", &name, "config", "--json"]));
    // JSON output should be valid JSON
    assert!(
        serde_json::from_str::<serde_json::Value>(&sandbox.last_stdout).is_ok()
    );

    // Test a simple action without extra flags
    assert!(sandbox.pass(&["--name", &name, "status"]));

    Ok(())
}

#[rstest]
fn test_flags_mixed_position(mut sandbox: SandboxManager) -> Result<()> {
    // Test mixing flags before and after action
    assert!(sandbox.pass(&["--name", "test-mixed", "config", "--net=host"]));
    assert!(sandbox.last_stdout.contains("name=test-mixed"));
    assert!(sandbox.last_stdout.contains("net=host"));

    assert!(sandbox.pass(&["-v", "config", "--net=none"]));
    assert!(sandbox.last_stdout.contains("log_level=TRACE"));
    assert!(sandbox.last_stdout.contains("net=none"));

    assert!(sandbox.pass(&["--json", "config", "--name=json-test", "-v"]));
    let json: serde_json::Value = serde_json::from_str(&sandbox.last_stdout)?;
    assert_eq!(json["name"], "json-test");
    assert_eq!(json["log_level"], "TRACE");

    Ok(())
}

#[rstest]
fn test_flags_with_complex_actions(mut sandbox: SandboxManager) -> Result<()> {
    // Create a test file to test actions
    sandbox.run(&["sh", "-c", "echo test > testfile.txt"])?;

    // Test flags with various actions that take arguments
    assert!(sandbox.pass(&["status", "testfile.txt", "--json"]));
    assert!(
        serde_json::from_str::<serde_json::Value>(&sandbox.last_stdout).is_ok()
    );

    assert!(sandbox.pass(&["--json", "status", "testfile.txt"]));
    assert!(
        serde_json::from_str::<serde_json::Value>(&sandbox.last_stdout).is_ok()
    );

    // Test with list action instead of diff (diff doesn't support JSON)
    assert!(sandbox.pass(&["list", "--json"]));
    assert!(
        serde_json::from_str::<serde_json::Value>(&sandbox.last_stdout).is_ok()
    );

    assert!(sandbox.pass(&["--json", "list"]));
    assert!(
        serde_json::from_str::<serde_json::Value>(&sandbox.last_stdout).is_ok()
    );

    Ok(())
}

#[rstest]
fn test_flag_errors_in_different_positions(
    mut sandbox: SandboxManager,
) -> Result<()> {
    // Test invalid flag values in different positions
    assert!(sandbox.xfail(&["--net=invalid", "config"]));
    assert!(sandbox.xfail(&["config", "--net=invalid"]));
    assert!(sandbox.xfail(&["--bind-fuse=invalid", "config"]));
    assert!(sandbox.xfail(&["config", "--bind-fuse=invalid"]));

    // Test unknown flags in different positions
    assert!(sandbox.xfail(&["--unknown-flag", "config"]));
    assert!(sandbox.xfail(&["config", "--unknown-flag"]));

    Ok(())
}

#[rstest]
fn test_boolean_flags_positioning(mut sandbox: SandboxManager) -> Result<()> {
    // Disable default ignored flag to avoid conflicts
    sandbox.set_ignored(false);

    // Test boolean flags without values in different positions
    assert!(sandbox.pass(&["--ignored", "config"]));
    assert!(sandbox.last_stdout.contains("ignored=true"));

    sandbox.set_ignored(false);
    assert!(sandbox.pass(&["config", "--ignored"]));
    assert!(sandbox.last_stdout.contains("ignored=true"));

    sandbox.set_ignored(false);
    assert!(sandbox.pass(&["--json", "config", "--ignored"]));
    let json: serde_json::Value = serde_json::from_str(&sandbox.last_stdout)?;
    assert_eq!(json["ignored"], "true");

    Ok(())
}

#[rstest]
fn test_short_flags_positioning(mut sandbox: SandboxManager) -> Result<()> {
    // Test short flags in different positions
    assert!(sandbox.pass(&["-v", "config"]));
    assert!(sandbox.last_stdout.contains("log_level=TRACE"));

    assert!(sandbox.pass(&["config", "-v"]));
    assert!(sandbox.last_stdout.contains("log_level=TRACE"));

    // Test combining with other flags
    assert!(sandbox.pass(&["-v", "config", "--json"]));
    let json: serde_json::Value = serde_json::from_str(&sandbox.last_stdout)?;
    assert_eq!(json["log_level"], "TRACE");

    Ok(())
}

#[rstest]
fn test_no_config_flag_positioning(mut sandbox: SandboxManager) -> Result<()> {
    sandbox.no_default_options = true;

    // Test --no-config flag in different positions
    assert!(sandbox.pass(&["--no-config", "config"]));
    // Should show defaults when no config is loaded
    assert!(sandbox.last_stdout.contains("net=none"));

    assert!(sandbox.pass(&["config", "--no-config"]));
    assert!(sandbox.last_stdout.contains("net=none"));

    // Test with other flags
    assert!(sandbox.pass(&["--no-config", "--net=host", "config"]));
    assert!(sandbox.last_stdout.contains("net=host"));

    assert!(sandbox.pass(&["config", "--no-config", "--net=host"]));
    assert!(sandbox.last_stdout.contains("net=host"));

    Ok(())
}
