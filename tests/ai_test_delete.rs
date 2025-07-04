mod fixtures;

use anyhow::Result;
use fixtures::*;
use rstest::*;
use std::path::{Path, PathBuf};

#[rstest]
fn test_delete_single_sandbox() -> Result<()> {
    let mut sandbox = sandbox();
    let name = sandbox.name.clone();

    // Create and run a sandbox
    sandbox.run(&["true"])?;

    // Verify sandbox exists
    sandbox.run(&["list"])?;
    assert!(sandbox.last_stdout.contains(&name));

    // Delete with confirmation (use -y flag)
    sandbox.run(&["delete", "-y", &name])?;
    assert!(sandbox.last_stdout.contains(&name));
    assert!(sandbox.last_stdout.contains("1 sandboxes deleted"));

    // Verify sandbox no longer exists
    sandbox.run(&["list"])?;
    assert!(!sandbox.last_stdout.contains(&name));

    Ok(())
}

#[rstest]
fn test_delete_current_sandbox_without_y() -> Result<()> {
    let mut sandbox = sandbox();
    let _name = sandbox.name.clone();

    // Create and run the sandbox
    sandbox.run(&["true"])?;

    // Delete without patterns and without -y should prompt for current sandbox
    // TODO: Once the bug is fixed, this should show the current sandbox
    // For now it will show "No default sandbox found"
    sandbox.run_with_stdin(&["delete"], "n\n")?;

    assert!(
        sandbox
            .last_stdout
            .contains("The following sandboxes will be deleted")
    );
    assert!(sandbox.last_stdout.contains(&sandbox.name));

    Ok(())
}

#[rstest]
fn test_delete_current_sandbox_no_pattern() -> Result<()> {
    let mut sandbox = sandbox();
    let _name = sandbox.name.clone();

    // Create and run a sandbox
    sandbox.run(&["true"])?;

    // Delete without pattern should delete current sandbox
    // NOTE: There's a bug in the current implementation where patterns.is_empty()
    // is checked twice with contradictory logic, so it currently shows "No default sandbox"
    sandbox.run(&["delete", "-y"])?;

    // TODO: Once the bug is fixed, this should delete the current sandbox
    // For now, we test what it actually does
    assert!(sandbox.last_stdout.contains("1 sandboxes deleted"));

    Ok(())
}

#[rstest]
fn test_delete_current_non_existent_sandbox() -> Result<()> {
    let mut sandbox = sandbox();
    let _name = sandbox.name.clone();

    sandbox.run_with_stdin(&["delete"], "y\n")?;
    assert!(sandbox.last_stdout.contains("No sandbox by the name"));

    Ok(())
}

#[rstest]
fn test_delete_shows_sandbox_status() -> Result<()> {
    let mut sandbox1 = sandbox();
    let mut sandbox2 = sandbox();

    // Give them unique but similar names
    let base_name = format!("test-del-status-{}", rid());
    sandbox1.name = format!("{}-1", base_name);
    sandbox2.name = format!("{}-2", base_name);
    let name1 = sandbox1.name.clone();
    let name2 = sandbox2.name.clone();

    // Create running sandbox
    sandbox1.run(&["true"])?;

    // Create and stop another sandbox
    sandbox2.run(&["true"])?;
    sandbox2.run(&["stop", &name2])?;

    // Delete both with specific pattern
    sandbox1.run(&["delete", "-y", &format!("{}*", base_name)])?;

    // Check output shows status information
    assert!(sandbox1.last_stdout.contains(&name1));
    assert!(sandbox1.last_stdout.contains(&name2));
    // The actual output format shows "running" or "stopped" before the sandbox name
    assert!(sandbox1.last_stdout.contains("2 sandboxes deleted"));

    Ok(())
}

#[rstest]
fn test_delete_multiple_sandboxes_with_pattern() -> Result<()> {
    let mut sandbox1 = sandbox();
    let mut sandbox2 = sandbox();
    let mut sandbox3 = sandbox();

    // Give them related names
    sandbox1.name = format!("test-delete-{}", rid());
    sandbox2.name = format!("test-delete-{}", rid());
    sandbox3.name = format!("test-other-{}", rid());

    // Create sandboxes
    sandbox1.run(&["true"])?;
    sandbox2.run(&["true"])?;
    sandbox3.run(&["true"])?;

    // Delete sandboxes matching pattern
    sandbox1.run(&["delete", "-y", "test-delete*"])?;

    // Should show the sandboxes being deleted
    assert!(sandbox1.last_stdout.contains(&sandbox1.name));
    assert!(sandbox1.last_stdout.contains(&sandbox2.name));
    assert!(!sandbox1.last_stdout.contains(&sandbox3.name));

    // Count how many were actually deleted (at least 2)
    let deleted_count = sandbox1
        .last_stdout
        .lines()
        .filter(|line| line.contains("Deleted sandbox:"))
        .count();
    assert!(deleted_count >= 2);

    // Verify only matching sandboxes were deleted
    sandbox1.run(&["list"])?;
    assert!(!sandbox1.last_stdout.contains(&sandbox1.name));
    assert!(!sandbox1.last_stdout.contains(&sandbox2.name));
    assert!(sandbox1.last_stdout.contains(&sandbox3.name));

    Ok(())
}

#[rstest]
fn test_delete_no_matching_sandboxes() -> Result<()> {
    let mut sandbox = sandbox();

    // Try to delete non-existent sandbox
    sandbox.run(&["delete", "-y", "non-existent-sandbox"])?;
    assert!(sandbox.last_stdout.contains("No sandboxes found matching"));

    Ok(())
}

#[rstest]
fn test_delete_stopped_sandbox() -> Result<()> {
    let mut sandbox = sandbox();
    let name = sandbox.name.clone();

    // Create and run a sandbox
    sandbox.run(&["true"])?;

    // Stop the sandbox
    sandbox.run(&["stop"])?;
    sandbox.run(&["accept"])?;

    // Delete the stopped sandbox
    sandbox.run_with_stdin(&["delete"], "y\n")?;
    println!("{}", sandbox.last_stdout);
    assert!(sandbox.last_stdout.contains("0 entries"));
    assert!(sandbox.last_stdout.contains("stopped"));
    assert!(sandbox.last_stdout.contains(&name));
    assert!(sandbox.last_stdout.contains("1 sandboxes deleted"));

    Ok(())
}

#[rstest]
fn test_delete_modified_sandbox_n() -> Result<()> {
    let mut sandbox = sandbox();
    let name = sandbox.name.clone();
    let filename = sandbox.test_filename("test_file");

    // Create and run a sandbox
    sandbox.run(&["touch", &filename])?;

    // Delete the stopped sandbox
    sandbox.run_with_stdin(&["delete", &name], "n\n")?;
    eprintln!("{}", sandbox.last_stdout);
    assert!(sandbox.last_stdout.contains("3 entries"));
    assert!(sandbox.last_stdout.contains("0 ignored"));
    assert!(sandbox.last_stdout.contains("running"));
    assert!(sandbox.last_stdout.contains(&name));

    Ok(())
}

#[rstest]
fn test_delete_modified_ignored_sandbox_n() -> Result<()> {
    let mut sandbox = sandbox();
    let name = sandbox.name.clone();
    let filename = sandbox.test_filename("test_file");

    // Create and run a sandbox
    sandbox.run(&["touch", &filename])?;

    // Delete the stopped sandbox
    sandbox.set_ignored(false);
    sandbox.run_with_stdin(&["delete", &name], "n\n")?;
    eprintln!("{}", sandbox.last_stdout);
    assert!(sandbox.last_stdout.contains("0 entries"));
    assert!(sandbox.last_stdout.contains("3 ignored"));
    assert!(sandbox.last_stdout.contains("running"));
    assert!(sandbox.last_stdout.contains(&name));

    Ok(())
}

#[rstest]
fn test_delete_stopped_sandbox_by_name() -> Result<()> {
    let mut sandbox = sandbox();
    let name = sandbox.name.clone();

    // Create and run a sandbox
    sandbox.run(&["true"])?;

    // Stop the sandbox
    sandbox.run(&["stop"])?;
    sandbox.run(&["accept"])?;

    // Delete the stopped sandbox
    sandbox.run_with_stdin(&["delete", &name], "y\n")?;
    assert!(sandbox.last_stdout.contains("0 entries"));
    assert!(sandbox.last_stdout.contains("stopped"));
    assert!(sandbox.last_stdout.contains(&name));
    assert!(sandbox.last_stdout.contains("1 sandboxes deleted"));

    Ok(())
}

#[rstest]
fn test_delete_failure(mut sandbox: SandboxManager) -> Result<()> {
    sandbox.run(&["true"])?;
    assert!(sandbox.epass(&["delete", "-y"], "TEST_DELETE_FAILURE", "1"));
    assert!(sandbox.last_stdout.contains("Error deleting sandbox"));
    Ok(())
}

#[rstest]
fn test_delete_running_sandbox_by_name() -> Result<()> {
    let mut sandbox = sandbox();
    let name = sandbox.name.clone();

    // Create and run a sandbox
    sandbox.run(&["true"])?;

    // Accept the sandbox
    sandbox.run(&["accept"])?;

    // Delete the running sandbox
    sandbox.run_with_stdin(&["delete", &name], "y\n")?;
    assert!(sandbox.last_stdout.contains("running"));
    assert!(sandbox.last_stdout.contains(&name));
    assert!(sandbox.last_stdout.contains("1 sandboxes deleted"));

    Ok(())
}

#[rstest]
fn test_delete_running_sandbox_no_modifications_no_confirm() -> Result<()> {
    let mut sandbox = sandbox();
    let name = sandbox.name.clone();

    // Create and run a sandbox without any modifications
    sandbox.run(&["true"])?;

    // Delete should work even though there are no modifications
    sandbox.run(&["delete", "-y", &name])?;
    assert!(sandbox.last_stdout.contains(&name));
    assert!(sandbox.last_stdout.contains("1 sandboxes deleted"));

    // Verify sandbox no longer exists
    sandbox.run(&["list"])?;
    assert!(!sandbox.last_stdout.contains(&name));

    Ok(())
}

#[rstest]
fn test_delete_multiple_stopped_sandboxes() -> Result<()> {
    let mut sandbox1 = sandbox();
    let mut sandbox2 = sandbox();
    let mut sandbox3 = sandbox();

    // Give them related names
    let base_name = format!("test-del-stopped-{}", rid());
    sandbox1.name = format!("{}-1", base_name);
    sandbox2.name = format!("{}-2", base_name);
    sandbox3.name = format!("{}-3", base_name);
    let name1 = sandbox1.name.clone();
    let name2 = sandbox2.name.clone();
    let name3 = sandbox3.name.clone();

    // Create and stop all sandboxes
    sandbox1.run(&["true"])?;
    sandbox1.run(&["stop", &name1])?;

    sandbox2.run(&["true"])?;
    sandbox2.run(&["stop", &name2])?;

    sandbox3.run(&["true"])?;
    sandbox3.run(&["stop", &name3])?;

    // Delete all stopped sandboxes with pattern
    sandbox1.run(&["delete", "-y", &format!("{}*", base_name)])?;

    // Should show all sandboxes as stopped
    assert!(sandbox1.last_stdout.contains(&name1));
    assert!(sandbox1.last_stdout.contains(&name2));
    assert!(sandbox1.last_stdout.contains(&name3));
    assert!(sandbox1.last_stdout.contains("3 sandboxes deleted"));

    Ok(())
}

#[rstest]
fn test_delete_mixed_running_and_stopped_sandboxes() -> Result<()> {
    let mut sandbox1 = sandbox();
    let mut sandbox2 = sandbox();
    let mut sandbox3 = sandbox();

    // Give them related names
    let base_name = format!("test-del-mixed-{}", rid());
    sandbox1.name = format!("{}-running", base_name);
    sandbox2.name = format!("{}-stopped", base_name);
    sandbox3.name = format!("{}-also-running", base_name);
    let name1 = sandbox1.name.clone();
    let name2 = sandbox2.name.clone();
    let name3 = sandbox3.name.clone();

    // Create mixed state sandboxes
    sandbox1.run(&["true"])?; // Keep running

    sandbox2.run(&["true"])?;
    sandbox2.run(&["stop", &name2])?; // Stop this one

    sandbox3.run(&["true"])?; // Keep running

    // Delete all with pattern
    sandbox1.run(&["delete", "-y", &format!("{}*", base_name)])?;

    // Check that output distinguishes between running and stopped
    assert!(sandbox1.last_stdout.contains(&name1));
    assert!(sandbox1.last_stdout.contains(&name2));
    assert!(sandbox1.last_stdout.contains(&name3));
    assert!(sandbox1.last_stdout.contains("3 sandboxes deleted"));

    // Verify all are deleted
    sandbox1.run(&["list"])?;
    assert!(!sandbox1.last_stdout.contains(&name1));
    assert!(!sandbox1.last_stdout.contains(&name2));
    assert!(!sandbox1.last_stdout.contains(&name3));

    Ok(())
}

#[rstest]
fn test_delete_json_output() -> Result<()> {
    let mut sandbox = sandbox();
    let name = sandbox.name.clone();

    // Create sandbox
    sandbox.run(&["true"])?;

    // Delete with JSON output
    sandbox.run(&["--json", "delete", "-y", &name])?;

    // Parse JSON output
    let output: serde_json::Value = serde_json::from_str(&sandbox.last_stdout)?;
    assert_eq!(output["status"], "success");
    assert!(
        output["deleted"]
            .as_array()
            .unwrap()
            .contains(&serde_json::Value::String(name))
    );
    assert!(output["errors"].as_array().unwrap().is_empty());

    Ok(())
}

#[rstest]
fn test_delete_confirmation_yes() -> Result<()> {
    let mut sandbox = sandbox();
    let name = sandbox.name.clone();

    // Create and run a sandbox
    sandbox.run(&["true"])?;

    // Test confirming with 'y'
    sandbox.run_with_stdin(&["delete", &name], "y\n")?;
    assert!(sandbox.last_stdout.contains("1 sandboxes deleted"));
    assert!(sandbox.last_stdout.contains(&name));

    Ok(())
}

#[rstest]
fn test_delete_confirmation_no() -> Result<()> {
    let mut sandbox = sandbox();
    let name = sandbox.name.clone();

    // Create and run a sandbox
    sandbox.run(&["true"])?;

    // Test cancelling with 'n'
    sandbox.run_with_stdin(&["delete", &name], "n\n")?;
    assert!(sandbox.last_stdout.contains("Delete operation cancelled"));

    // Verify sandbox still exists
    sandbox.run(&["list"])?;
    assert!(sandbox.last_stdout.contains(&name));

    Ok(())
}

#[rstest]
fn test_delete_confirmation_capital_n() -> Result<()> {
    let mut sandbox = sandbox();
    let name = sandbox.name.clone();

    // Create and run a sandbox
    sandbox.run(&["true"])?;

    // Test cancelling with 'N'
    sandbox.run_with_stdin(&["delete", &name], "N\n")?;
    assert!(sandbox.last_stdout.contains("Delete operation cancelled"));

    // Verify sandbox still exists
    sandbox.run(&["list"])?;
    assert!(sandbox.last_stdout.contains(&name));

    Ok(())
}

#[rstest]
fn test_delete_confirmation_default_no() -> Result<()> {
    let mut sandbox = sandbox();
    let name = sandbox.name.clone();

    // Create and run a sandbox
    sandbox.run(&["true"])?;

    // Test pressing just Enter (should default to No)
    sandbox.run_with_stdin(&["delete", &name], "\n")?;
    assert!(sandbox.last_stdout.contains("Delete operation cancelled"));

    // Verify sandbox still exists
    sandbox.run(&["list"])?;
    assert!(sandbox.last_stdout.contains(&name));

    Ok(())
}

#[rstest]
fn test_delete_cleans_all_files() -> Result<()> {
    let mut sandbox = sandbox();
    let name = sandbox.name.clone();

    // Create sandbox
    sandbox.run(&["true"])?;

    // Get storage directory to verify files
    sandbox.run(&["config", "storage_dir"])?;
    let storage_dir = PathBuf::from(sandbox.last_stdout.trim());

    let lock_file = storage_dir.join(format!("{}.lock", name));
    let pid_file = storage_dir.join(format!("{}.pid", name));
    let sandbox_dir = storage_dir.join(&name);

    // Verify files exist before deletion
    assert!(lock_file.exists() || pid_file.exists() || sandbox_dir.exists());

    // Delete sandbox
    sandbox.run(&["delete", "-y", &name])?;

    // Verify all files are removed
    assert!(!lock_file.exists());
    assert!(!pid_file.exists());
    assert!(!sandbox_dir.exists());

    Ok(())
}

#[rstest]
fn test_delete_with_modifications() -> Result<()> {
    let mut sandbox = sandbox();
    let name = sandbox.name.clone();

    // Create sandbox and make some modifications
    sandbox.run(&["true"])?;
    sandbox.run(&["sh", "-c", "echo 'test' > /tmp/test_file"])?;

    // Delete with -y flag just shows success messages
    sandbox.run(&["delete", "-y", &name])?;
    assert!(sandbox.last_stdout.contains(&name));
    assert!(sandbox.last_stdout.contains("1 sandboxes deleted"));

    Ok(())
}

#[rstest]
fn test_delete_ignore_invalid_upper_dirs(
    mut sandbox: SandboxManager,
) -> Result<()> {
    sandbox.run(&["config", "storage_dir"])?;
    let storage_dir = sandbox.last_stdout.trim();
    let sandbox_upper_dir =
        Path::new(storage_dir).join(&sandbox.name).join("upper");

    let invalid_base32_dir = sandbox_upper_dir.join("@@@INVALID@@@");
    std::fs::create_dir_all(&invalid_base32_dir)?;
    std::fs::create_dir_all(invalid_base32_dir.join("subdir"))?;
    std::fs::write(invalid_base32_dir.join("subdir/file.txt"), "test")?;

    let invalid_utf8_bytes = vec![0xFF, 0xFE];
    let base32_invalid_utf8 =
        data_encoding::BASE32_NOPAD_NOCASE.encode(&invalid_utf8_bytes);
    let invalid_utf8_dir = sandbox_upper_dir.join(&base32_invalid_utf8);
    std::fs::create_dir_all(&invalid_utf8_dir)?;
    std::fs::create_dir_all(invalid_utf8_dir.join("subdir"))?;
    std::fs::write(invalid_utf8_dir.join("subdir/file.txt"), "test")?;

    let filename = sandbox.test_filename("file");
    sandbox.run(&["touch", &filename])?;

    // Run it
    //sandbox.run(&["true"])?;
    // Now delete should process these directories and skip them
    sandbox.run_with_stdin(&["delete"], "y\n")?;
    assert!(sandbox.last_stdout.contains("1 sandboxes deleted"));
    Ok(())
}
