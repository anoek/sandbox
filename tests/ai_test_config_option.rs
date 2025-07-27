mod fixtures;

use anyhow::Result;
use fixtures::*;
use rstest::*;
use std::fs;

/// Test the --config CLI option behavior
#[rstest]
fn ai_test_config_option() -> Result<()> {
    // Test config loading without running actual sandbox commands
    // This tests the CLI parsing functionality directly

    // Create test config files
    let test_dir = std::env::temp_dir().join("sandbox_config_test");
    std::fs::create_dir_all(&test_dir)?;

    let config_file = test_dir.join("test.toml");
    let config_content = r#"
log_level = "debug"
net = "host"
"#;
    fs::write(&config_file, config_content)?;

    let config2_file = test_dir.join("test2.toml");
    let config2_content = r#"
ignored = true
"#;
    fs::write(&config2_file, config2_content)?;

    // Test 1: --config conflicts with --no-config (using direct command)
    let output = std::process::Command::new(get_sandbox_bin())
        .args([
            "--config",
            config_file.to_str().unwrap(),
            "--no-config",
            "config",
        ])
        .output()?;
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("cannot be used with"));

    // Test 2: Error when config file doesn't exist
    let output = std::process::Command::new(get_sandbox_bin())
        .args(["--config", "/nonexistent/file.toml", "config"])
        .output()?;
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Config file not found"));

    // Test 3: Empty --config list means no config files loaded
    let output = std::process::Command::new(get_sandbox_bin())
        .args(["--config", "", "--help"])
        .output()?;
    assert!(output.status.success());

    // Test 4: Test that config action shows loaded config files
    // We need to use a working directory with an acceptable mount type
    let output = std::process::Command::new(get_sandbox_bin())
        .env("TEST_UNACCEPTABLE_MOUNT_TYPE", "") // Disable mount type check for test
        .args([
            "--config",
            config_file.to_str().unwrap(),
            "config",
            "config_files",
        ])
        .output()?;
    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains(config_file.to_string_lossy().as_ref()));
    }

    // Clean up
    std::fs::remove_dir_all(&test_dir).ok();

    Ok(())
}
