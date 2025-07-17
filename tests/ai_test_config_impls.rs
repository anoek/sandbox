// Integration tests for config implementations
// These tests verify that config parsing works correctly end-to-end

mod fixtures;

use anyhow::Result;
use fixtures::*;
use rstest::*;
use std::fs;
use std::sync::Mutex;

// Use mutex to avoid config file conflicts between tests
static CONFIG_TEST_MUTEX: Mutex<()> = Mutex::new(());

#[rstest]
fn test_network_options_in_help() -> Result<()> {
    // Verify that network options are properly displayed in help
    let mut sandbox = SandboxManager::new();

    assert!(sandbox.pass(&["--help"]));
    assert!(
        sandbox.last_stdout.contains("--net"),
        "Help should contain --net option"
    );
    assert!(
        sandbox.last_stdout.contains("none")
            && sandbox.last_stdout.contains("host"),
        "Network options (none, host) should be visible in help"
    );

    Ok(())
}

#[rstest]
fn test_valid_config_formats() -> Result<()> {
    let _guard = CONFIG_TEST_MUTEX.lock().unwrap();
    let mut sandbox = SandboxManager::new();
    sandbox.no_sudo = true;

    // Create a local config file in test directory
    let test_dir = format!("generated-test-data/{}", sandbox.name);
    fs::create_dir_all(&test_dir)?;
    let config_file = format!("{}/.sandbox.toml", test_dir);

    // Test valid configs - just verify they don't cause parse errors
    let bind_single = format!("{}/single", test_dir);
    let bind_path1 = format!("{}/path1", test_dir);
    let bind_path2 = format!("{}/path2", test_dir);
    fs::create_dir_all(&bind_single)?;
    fs::create_dir_all(&bind_path1)?;
    fs::create_dir_all(&bind_path2)?;

    let valid_configs = [
        format!(r#"bind = "{}""#, bind_single),
        format!(r#"bind = ["{}", "{}"]"#, bind_path1, bind_path2),
        r#"log_level = "DEBUG""#.to_string(),
        r#"net = "host""#.to_string(),
        r#"net = "none""#.to_string(),
    ];

    for config in valid_configs.iter() {
        fs::write(&config_file, config)?;

        let config_passed = sandbox.pass(&["config"]);

        if config_passed {
            // For bind configs, verify they appear in the output
            if config.contains("bind") && config != r#"bind = null"# {
                assert!(
                    sandbox.last_stdout.contains("bind_mounts"),
                    "Config output should contain bind_mounts for config: {}",
                    config
                );
            }
        } else {
            // If it fails due to permissions, that's OK
            if !sandbox.last_stderr.contains("Insufficient permissions") {
                panic!(
                    "Valid config '{}' should not cause errors, got: {}",
                    config, sandbox.last_stderr
                );
            }
        }
    }

    Ok(())
}

#[rstest]
fn test_config_file_locations() -> Result<()> {
    let _guard = CONFIG_TEST_MUTEX.lock().unwrap();
    let mut sandbox = SandboxManager::new();
    sandbox.no_sudo = true;

    // Test that sandbox looks for config in the right places
    // 1. .sandbox.toml in current directory
    let test_dir = format!("generated-test-data/{}", sandbox.name);
    fs::create_dir_all(&test_dir)?;
    let local_config = format!("{}/.sandbox.toml", test_dir);
    fs::write(&local_config, r#"log_level = "TRACE""#)?;

    // Change to test directory and run config
    let original_dir = std::env::current_dir()?;
    std::env::set_current_dir(&test_dir)?;

    if sandbox.pass(&["config"]) {
        assert!(
            sandbox.last_stdout.contains("TRACE"),
            "Should pick up local .sandbox.toml config"
        );
    }

    // Don't change back yet, we need to test precedence

    // 2. Test precedence - CLI args should override config file
    if sandbox.pass(&["--log-level", "ERROR", "config"]) {
        assert!(
            sandbox.last_stdout.contains("ERROR"),
            "CLI args should override config file"
        );
    }

    // Now change back
    std::env::set_current_dir(&original_dir)?;

    Ok(())
}
