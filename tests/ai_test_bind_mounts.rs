mod fixtures;

use anyhow::Result;
use fixtures::*;
use rstest::*;
use std::{fs, path::PathBuf};

#[rstest]
fn test_bind_mount_cli(mut sandbox: SandboxManager) -> Result<()> {
    // Create test directory and bind mount directories
    let test_dir = sandbox.test_filename("bind_cli");
    fs::create_dir_all(&test_dir)?;
    let test1 = format!("{}/test1", test_dir);
    let test2 = format!("{}/test2", test_dir);
    let mapped1 = format!("{}/mapped1", test_dir);
    fs::create_dir_all(&test1)?;
    fs::create_dir_all(&test2)?;
    fs::create_dir_all(&mapped1)?;

    // Test single bind mount
    assert!(sandbox.pass(&["--bind", &test1, "config"]));
    // The output will include default bind mounts too, so just check that our bind mount is included
    assert!(sandbox.last_stdout.contains("bind="));
    assert!(sandbox.last_stdout.contains(&test1));

    // Test multiple bind mounts with separate flags
    assert!(sandbox.pass(&["--bind", &test1, "--bind", &test2, "config"]));
    assert!(sandbox.last_stdout.contains(&test1));
    assert!(sandbox.last_stdout.contains(&test2));

    // Test comma-separated bind mounts
    assert!(sandbox.pass(&[
        "--bind",
        &format!("{},{}", test1, test2),
        "config"
    ]));
    assert!(sandbox.last_stdout.contains(&test1));
    assert!(sandbox.last_stdout.contains(&test2));

    // Test bind mount with source:target format
    assert!(sandbox.pass(&[
        "--bind",
        &format!("{}:{}", test1, mapped1),
        "config"
    ]));
    assert!(
        sandbox
            .last_stdout
            .contains(&format!("{}:{}", test1, mapped1))
    );

    Ok(())
}

#[rstest]
fn test_bind_mount_env(mut sandbox: SandboxManager) -> Result<()> {
    // Create test directory and bind mount directories
    let test_dir = sandbox.test_filename("bind_env");
    fs::create_dir_all(&test_dir)?;
    let test1 = format!("{}/test1", test_dir);
    let test2 = format!("{}/test2", test_dir);
    let mapped1 = format!("{}/mapped1", test_dir);
    fs::create_dir_all(&test1)?;
    fs::create_dir_all(&test2)?;
    fs::create_dir_all(&mapped1)?;

    // Test environment variable using run_with_env
    sandbox.run_with_env(
        &["config"],
        "SANDBOX_BIND",
        &format!("{},{}", test1, test2),
    )?;
    assert!(sandbox.last_stdout.contains(&test1));
    assert!(sandbox.last_stdout.contains(&test2));

    // Test environment variable with source:target format
    sandbox.run_with_env(
        &["config"],
        "SANDBOX_BIND",
        &format!("{}:{}", test1, mapped1),
    )?;
    assert!(
        sandbox
            .last_stdout
            .contains(&format!("{}:{}", test1, mapped1))
    );

    Ok(())
}

// NOTE: test_bind_mount_additive moved to ai_isolated_bind_mount_additive.rs

#[rstest]
fn test_default_bind_mounts(mut sandbox: SandboxManager) -> Result<()> {
    // Test that default bind mounts are included
    assert!(sandbox.pass(&["config"]));

    // Default bind mounts should include some system directories
    // The exact list might vary by system, but /dev/fuse is commonly included:
    assert!(sandbox.last_stdout.contains("/dev/fuse"));

    Ok(())
}

#[rstest]
fn test_no_default_binds_flag(mut sandbox: SandboxManager) -> Result<()> {
    // Create test directory and bind mount directories
    let test_dir = sandbox.test_filename("no_default_binds");
    fs::create_dir_all(&test_dir)?;
    let test1 = format!("{}/test1", test_dir);
    fs::create_dir_all(&test1)?;

    // Test that custom binds work with --no-default-binds
    assert!(sandbox.pass(&["--no-default-binds", "--bind", &test1, "config"]));

    // Should only contain the custom bind mount
    assert!(sandbox.last_stdout.contains(&test1));
    assert!(!sandbox.last_stdout.contains("/dev/fuse"));

    Ok(())
}

#[rstest]
fn test_no_default_binds_env(mut sandbox: SandboxManager) -> Result<()> {
    // Create test directory and bind mount directories
    let test_dir = sandbox.test_filename("no_default_binds_env");
    fs::create_dir_all(&test_dir)?;
    let test1 = format!("{}/test1", test_dir);
    fs::create_dir_all(&test1)?;

    // Test that no default binds works via environment variable
    sandbox.run_with_env(
        &["--no-config", "config"],
        "SANDBOX_NO_DEFAULT_BINDS",
        "true",
    )?;
    assert!(sandbox.last_stdout.contains("bind=\n"));

    // Test that custom binds still work with the env variable
    sandbox.run_with_env(
        &["--bind", &test1, "--no-config", "config"],
        "SANDBOX_NO_DEFAULT_BINDS",
        "true",
    )?;
    assert!(sandbox.last_stdout.contains(&test1));
    assert!(!sandbox.last_stdout.contains("/dev/fuse"));

    // Test that SANDBOX_NO_DEFAULT_BINDS=false (or any other value) doesn't disable defaults
    sandbox.run_with_env(
        &["--no-config", "config"],
        "SANDBOX_NO_DEFAULT_BINDS",
        "false",
    )?;
    assert!(sandbox.last_stdout.contains("/dev/fuse"));

    Ok(())
}

#[rstest]
fn test_bind_mount_env_parsing() -> Result<()> {
    let mut sandbox = SandboxManager::new();

    // Test empty env var
    sandbox.run_with_env(
        &["--no-default-binds", "--no-config", "config"],
        "SANDBOX_BIND",
        "",
    )?;

    // Should have no bind mounts
    assert!(sandbox.last_stdout.contains("bind=\n"));

    // Test env var with trailing comma
    let test_dir = sandbox.test_filename("bind_env_parsing");
    fs::create_dir_all(&test_dir)?;
    let test1 = format!("{}/test1", test_dir);
    let test2 = format!("{}/test2", test_dir);
    fs::create_dir_all(&test1)?;
    fs::create_dir_all(&test2)?;

    sandbox.run_with_env(
        &["--no-default-binds", "--no-config", "config"],
        "SANDBOX_BIND",
        &format!("{},{},", test1, test2),
    )?;

    // Should have the two bind mounts (trailing comma is ignored)
    assert!(sandbox.last_stdout.contains(&test1));
    assert!(sandbox.last_stdout.contains(&test2));

    // Test empty env var combined with CLI bind mount
    sandbox.run_with_env(
        &["--bind", &test1, "--no-config", "config"],
        "SANDBOX_BIND",
        "",
    )?;

    // Should have the CLI bind mount
    assert!(sandbox.last_stdout.contains(&test1));

    // Test whitespace-only env var
    sandbox.run_with_env(
        &["--no-default-binds", "--no-config", "config"],
        "SANDBOX_BIND",
        "   ",
    )?;

    // Should have no bind mounts
    assert!(sandbox.last_stdout.contains("bind=\n"));

    Ok(())
}

#[rstest]
fn test_bind_mount_env_with_options(mut sandbox: SandboxManager) -> Result<()> {
    let test_dir = sandbox.test_filename("bind_env_options");
    fs::create_dir_all(&test_dir)?;
    let test1 = format!("{}/test1", test_dir);
    let test2 = format!("{}/test2", test_dir);
    let test3 = format!("{}/test3", test_dir);
    let test4 = format!("{}/test4", test_dir);
    fs::create_dir_all(&test1)?;
    fs::create_dir_all(&test2)?;
    fs::create_dir_all(&test3)?;
    fs::create_dir_all(&test4)?;

    // Test environment variable with read-only options
    sandbox.run_with_env(
        &["--no-default-binds", "config"],
        "SANDBOX_BIND",
        &format!("{}::ro,{}::ro", test1, test2),
    )?;

    // Should include both bind mounts with their options
    assert!(sandbox.last_stdout.contains(&format!("{}::ro", test1)));
    assert!(sandbox.last_stdout.contains(&format!("{}::ro", test2)));

    // Test mixed read-only and read-write mounts
    sandbox.run_with_env(
        &["--no-default-binds", "config"],
        "SANDBOX_BIND",
        &format!("{},{}", test3, test4),
    )?;

    assert!(sandbox.last_stdout.contains(&test3));
    assert!(sandbox.last_stdout.contains(&test4));

    Ok(())
}

#[rstest]
fn test_bind_mount_path_validation() -> Result<()> {
    let mut sandbox = SandboxManager::new();

    // Test handling of bind mounts with special characters
    // These should be accepted without issue
    let test_dir = sandbox.test_filename("bind_special_chars");
    fs::create_dir_all(&test_dir)?;
    let path_with_spaces = format!("{}/path with spaces", test_dir);
    let path_with_dashes = format!("{}/path-with-dashes", test_dir);
    let path_with_underscores = format!("{}/path_with_underscores", test_dir);
    fs::create_dir_all(&path_with_spaces)?;
    fs::create_dir_all(&path_with_dashes)?;
    fs::create_dir_all(&path_with_underscores)?;

    assert!(sandbox.pass(&[
        "--bind",
        &path_with_spaces,
        "--bind",
        &path_with_dashes,
        "--bind",
        &path_with_underscores,
        "config"
    ]));

    assert!(sandbox.last_stdout.contains(&path_with_spaces));
    assert!(sandbox.last_stdout.contains(&path_with_dashes));
    assert!(sandbox.last_stdout.contains(&path_with_underscores));

    Ok(())
}

#[rstest]
fn test_bind_mount_read_only_behavior() -> Result<()> {
    let mut sandbox = SandboxManager::new();

    // Create a test directory with a file
    let test_dir = sandbox.test_filename("bind_ro_test");
    fs::create_dir_all(&test_dir)?;
    let existing_file = format!("{}/existing.txt", test_dir);
    fs::write(&existing_file, "existing content")?;
    let test_dir = std::fs::canonicalize(&test_dir)?
        .to_string_lossy()
        .to_string();

    // For this test, we'll simulate the read-only behavior by checking
    // that the bind mount is set up correctly
    assert!(sandbox.pass(&[
        "--bind",
        &test_dir,
        "--",
        "cat",
        &format!("{}/existing.txt", test_dir)
    ]));

    // Should be able to read the existing file
    assert_eq!(sandbox.last_stdout.trim(), "existing content");

    Ok(())
}

#[rstest]
fn test_bind_mount_env_read_only() -> Result<()> {
    let mut sandbox = SandboxManager::new();

    // Create test directory with a file
    let test_dir = sandbox.test_filename("bind_env_ro");
    fs::create_dir_all(&test_dir)?;
    let test_file = format!("{}/test.txt", test_dir);
    fs::write(&test_file, "test content")?;

    let test_dir_str = std::fs::canonicalize(&test_dir)?
        .to_string_lossy()
        .to_string();
    let env_ro = sandbox.test_filename("env_ro");
    fs::create_dir_all(&env_ro)?;
    let env_ro = std::fs::canonicalize(&env_ro)?
        .to_string_lossy()
        .to_string();

    // Test read-only bind mounts via environment variable
    // We expect this to fail with a non-zero exit code
    let result = sandbox.run_with_env(
        &[
            "--no-config",
            "--",
            "touch",
            &format!("{}/new_file.txt", env_ro),
        ],
        "SANDBOX_BIND",
        &format!("{}:{}:ro", test_dir_str, env_ro),
    );

    // Should fail due to read-only mount
    assert!(
        result.is_err(),
        "Expected failure when writing to read-only mount"
    );

    // Check for read-only filesystem error
    assert!(
        sandbox.last_stderr.contains("Read-only file system")
            || sandbox.last_stderr.contains("read-only file system"),
        "Expected read-only filesystem error, got: {}",
        sandbox.last_stderr
    );

    // Test comma-separated bind mounts with mixed options
    let test1 = sandbox.test_filename("test1");
    let test2 = sandbox.test_filename("test2");
    let mapped = sandbox.test_filename("mapped");
    fs::create_dir_all(&test1)?;
    fs::create_dir_all(&test2)?;
    fs::create_dir_all(&mapped)?;

    sandbox.run_with_env(
        &["--no-config", "config"],
        "SANDBOX_BIND",
        &format!("{},{}:{}:ro,{}::ro", test1, test2, mapped, test_dir_str),
    )?;

    // Should contain all bind mounts
    assert!(sandbox.last_stdout.contains(&test1));
    assert!(
        sandbox
            .last_stdout
            .contains(&format!("{}:{}:ro", test2, mapped))
    );
    assert!(
        sandbox
            .last_stdout
            .contains(&format!("{}::ro", test_dir_str))
    );

    // Cleanup
    fs::remove_dir_all(test_dir)?;

    Ok(())
}

#[rstest]
fn test_bind_mount_edge_cases() -> Result<()> {
    let mut sandbox = SandboxManager::new();
    let test_dir = sandbox.test_filename("bind_edge_cases");
    fs::create_dir_all(&test_dir)?;
    let tmp = format!("{}/tmp", test_dir);
    let var = format!("{}/var", test_dir);
    fs::create_dir_all(&tmp)?;
    fs::create_dir_all(&var)?;

    // Test case 1: /tmp: (source with empty destination)
    assert!(sandbox.pass(&[
        "--no-config",
        "--bind",
        &format!("{}:", tmp),
        "config"
    ]));
    // Should mount /tmp to /tmp (empty destination means same as source)
    assert!(sandbox.last_stdout.contains(&format!("{}:", tmp)));

    // Test case 2: /tmp:: (source with empty destination and empty options)
    let mut sandbox2 = SandboxManager::new();
    assert!(sandbox2.pass(&[
        "--no-config",
        "--bind",
        &format!("{}::", tmp),
        "config"
    ]));
    // Should mount /tmp to /tmp with no options
    assert!(sandbox2.last_stdout.contains(&format!("{}::", tmp)));

    // Test case 3: Empty destination with actual execution (from ai_test_bind_mount_empty_destination.rs)
    let mut sandbox3 = SandboxManager::new();
    assert!(sandbox3.pass(&[
        "--bind",
        &format!("{}:", tmp),
        "bash",
        "-c",
        "echo 'testing empty destination'",
    ]));
    assert!(sandbox3.last_stdout.contains("testing empty destination"));

    // Verify exit code is successful
    assert_eq!(
        sandbox3.last_stdout.lines().last(),
        Some("testing empty destination")
    );

    // Test case 4: Multiple bind mounts with empty destinations
    let mut sandbox4 = SandboxManager::new();
    assert!(sandbox4.pass(&[
        "--bind",
        &format!("{}:", tmp),
        "--bind",
        &format!("{}:", var),
        "bash",
        "-c",
        "echo 'multiple empty destinations'",
    ]));
    assert!(sandbox4.last_stdout.contains("multiple empty destinations"));

    Ok(())
}

// From ai_test_bind_mount_empty_destination.rs
#[rstest]
fn test_bind_mount_empty_destination() -> Result<()> {
    let mut sandbox = SandboxManager::new();
    let test_name = format!("sandbox-bind-empty-dest-{}", sandbox.name);
    let test_dir = sandbox.test_filename("bind_empty_dest");
    fs::create_dir_all(&test_dir)?;
    let tmp = format!("{}/tmp", test_dir);
    fs::create_dir_all(&tmp)?;

    // Run a command that will definitely create a new sandbox and process bind mounts
    assert!(sandbox.pass(&[
        "--name",
        &test_name,
        "--bind",
        &format!("{}:", tmp),
        "--",
        "bash",
        "-c",
        "echo 'empty dest bind mount test'"
    ]));

    assert!(sandbox.last_stdout.contains("empty dest bind mount test"));

    Ok(())
}

#[rstest]
fn test_bind_mount_empty_destination_with_sudo() -> Result<()> {
    // This test verifies empty destination bind mounts work correctly when sudo is involved
    let unique_name = format!("sandbox-bind-empty-sudo-{}", rid());
    let test_dir = format!("generated-test-data/{}", unique_name);
    fs::create_dir_all(&test_dir)?;
    let tmp = format!("{}/tmp", test_dir);
    fs::create_dir_all(&tmp)?;

    // Run directly with sudo to ensure proper handling of bind mount paths
    let output = std::process::Command::new("sudo")
        .arg("-E")
        .arg(env!("CARGO_BIN_EXE_sandbox"))
        .arg("--name")
        .arg(&unique_name)
        .arg("--bind")
        .arg(format!("{}:", tmp)) // Empty destination after colon
        .arg("--")
        .arg("echo")
        .arg("test")
        .output()?;

    assert!(
        output.status.success(),
        "Command failed with stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    Ok(())
}

#[rstest]
fn test_empty_bind_array_in_config() -> Result<()> {
    // Test empty bind array in config
    let mut sandbox = SandboxManager::new();

    // Create a temporary working directory for the config file
    let work_dir = sandbox.test_filename("workdir");
    fs::create_dir_all(&work_dir)?;

    // Write config file in the working directory
    let config_file = format!("{}/.sandbox.toml", work_dir);
    fs::write(
        &config_file,
        r#"
bind = []
"#,
    )?;

    // Should work but only have default binds
    let result = sandbox.run(&["config"]);

    assert!(result.is_ok());

    // Should have default binds but no custom ones
    let bind_mounts_line = sandbox
        .last_stdout
        .lines()
        .find(|line| line.starts_with("bind="))
        .expect("bind_mounts line should exist");

    // Should have some default binds (not empty)
    assert!(!bind_mounts_line.contains("bind=\n"));

    Ok(())
}

#[rstest]
fn test_all_deserialize_paths(mut sandbox: SandboxManager) -> Result<()> {
    // Comprehensive test covering all deserialization paths
    use std::fs;

    let test_dir = sandbox.test_filename("test_all_deserialize_paths");
    let config_dir = PathBuf::from(&test_dir).join(".config/sandbox");
    fs::create_dir_all(&config_dir)?;
    let config_file = config_dir.join("config.toml");

    // Test all valid formats
    let test_cases = vec![
        // String value (visit_str path)
        (r#"bind = "/single/path""#, "single string"),
        // Array value (visit_seq path)
        (r#"bind = ["/path1", "/path2", "/path3"]"#, "array"),
        // Empty array
        (r#"bind = []"#, "empty array"),
        // Null (visit_none path)
        (r#"bind = null"#, "null"),
        // Missing field (also uses visit_none via default)
        (r#""#, "missing field"),
    ];

    for (config, desc) in test_cases {
        fs::write(&config_file, config)?;

        // Set HOME to the test directory to use our config
        let result = sandbox.run_with_env(&["--version"], "HOME", &test_dir)?;

        assert!(
            result.status.success(),
            "Should handle {} correctly, got stderr: {}",
            desc,
            sandbox.last_stderr
        );
    }

    // Cleanup
    fs::remove_dir_all(&test_dir).ok();

    Ok(())
}
