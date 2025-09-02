// Integration tests for corrupted sandbox settings handling
// These tests verify error handling when sandbox settings files are corrupted or unreadable

mod fixtures;

use anyhow::Result;
use fixtures::*;
use rstest::*;
use std::fs;

#[rstest]
fn test_get_or_create_with_corrupted_settings(mut sandbox: SandboxManager) -> Result<()> {
    // First, create a working sandbox
    let _output = sandbox.run(&["echo", "initial setup"])?;
    
    // Get the sandbox directory from the sandbox tool itself
    let sandbox_dir = sandbox.dir()?;
    let data_dir = sandbox_dir.join("data");
    let settings_file = data_dir.join("settings.json");
    
    // Verify the sandbox was created and settings file exists
    assert!(settings_file.exists(), "Settings file should exist after initial sandbox creation at {}", settings_file.display());
    
    // Corrupt the settings file by writing invalid JSON using sudo
    let corrupt_json = "{ invalid json content that cannot be parsed }";
    let write_cmd = std::process::Command::new("sudo")
        .args(&["sh", "-c", &format!("echo '{}' > '{}'", corrupt_json, settings_file.display())])
        .output()?;
    
    if !write_cmd.status.success() {
        return Err(anyhow::anyhow!("Failed to corrupt settings file with sudo: {}", 
                                   String::from_utf8_lossy(&write_cmd.stderr)));
    }
    
    // Now try to run another command - this should trigger the error path in get_or_create
    // because the existing sandbox has corrupted settings
    let success = sandbox.pass(&["echo", "second command"]);
    
    // The command should fail due to corrupted settings
    assert!(!success, "Command should fail due to corrupted settings file");
    
    // Check that the error message contains the expected text
    let error_output = sandbox.last_stderr.clone();
    
    assert!(
        error_output.contains("Failed to load existing sandbox settings") ||
        error_output.contains("The sandbox may be corrupted") ||
        error_output.contains("Failed to parse"),
        "Expected error message about corrupted settings, got: {}",
        error_output
    );
    
    Ok(())
}

#[rstest]
fn test_get_or_create_with_unreadable_settings(mut sandbox: SandboxManager) -> Result<()> {
    // First, create a working sandbox
    let _output = sandbox.run(&["echo", "initial setup"])?;
    
    // Get the sandbox directory from the sandbox tool itself
    let sandbox_dir = sandbox.dir()?;
    let data_dir = sandbox_dir.join("data");
    let settings_file = data_dir.join("settings.json");
    
    // Verify the sandbox was created and settings file exists
    assert!(settings_file.exists(), "Settings file should exist after initial sandbox creation");
    
    // Make the file unreadable by changing permissions (if we have permission to do so)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&settings_file)?.permissions();
        perms.set_mode(0o000); // No read permissions
        
        // Only test this if we can actually change permissions (i.e., we're not root)
        if fs::set_permissions(&settings_file, perms).is_ok() {
            // Try to run another command - this should trigger the error path
            let success = sandbox.pass(&["echo", "second command"]);
            
            // The command should fail due to permission error
            assert!(!success, "Command should fail due to unreadable settings file");
            
            // Check that the error message contains the expected text
            let error_output = sandbox.last_stderr.clone();
            
            assert!(
                error_output.contains("Failed to load existing sandbox settings") ||
                error_output.contains("The sandbox may be corrupted") ||
                error_output.contains("Permission denied"),
                "Expected error message about unreadable settings, got: {}",
                error_output
            );
            
            // Restore permissions for cleanup
            let mut perms = fs::metadata(&settings_file)?.permissions();
            perms.set_mode(0o644);
            let _ = fs::set_permissions(&settings_file, perms);
        }
    }
    
    Ok(())
}

#[rstest]
fn test_get_or_create_with_directory_as_settings_file(mut sandbox: SandboxManager) -> Result<()> {
    // First, create a working sandbox
    let _output = sandbox.run(&["echo", "initial setup"])?;
    
    // Get the sandbox directory from the sandbox tool itself  
    let sandbox_dir = sandbox.dir()?;
    let data_dir = sandbox_dir.join("data");
    let settings_file = data_dir.join("settings.json");
    
    // Verify the sandbox was created and settings file exists
    assert!(settings_file.exists(), "Settings file should exist after initial sandbox creation");
    
    // Remove the settings file and replace it with a directory
    fs::remove_file(&settings_file)?;
    fs::create_dir(&settings_file)?;
    
    // Verify the path now exists as a directory
    assert!(settings_file.is_dir(), "Settings path should now be a directory");
    
    // Try to run another command - this should trigger the error path
    let success = sandbox.pass(&["echo", "second command"]);
    
    // The command should fail
    assert!(!success, "Command should fail when settings.json is a directory");
    
    // Check that the error message contains the expected text
    let error_output = sandbox.last_stderr.clone();
    
    assert!(
        error_output.contains("Failed to load existing sandbox settings") ||
        error_output.contains("The sandbox may be corrupted") ||
        error_output.contains("Is a directory"),
        "Expected error message about directory as settings file, got: {}",
        error_output
    );
    
    Ok(())
}