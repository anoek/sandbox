mod fixtures;

use anyhow::Result;
use fixtures::*;
use rstest::*;
use std::fs;

#[rstest]
fn test_bind_mount_validation_failure(mut sandbox: SandboxManager) -> Result<()> {
    // Create temporary directories using the test filename pattern
    let bind_source1 = sandbox.test_filename("bind-source-1");
    let bind_source2 = sandbox.test_filename("bind-source-2");
    fs::create_dir_all(&bind_source1)?;
    fs::create_dir_all(&bind_source2)?;
    fs::write(format!("{}/test.txt", bind_source1), "test content")?;
    fs::write(format!("{}/test.txt", bind_source2), "different content")?;
    
    // Create and start sandbox with initial bind mount
    let initial_bind = format!("{}:{}", bind_source1, "/mnt/test");
    let _output1 = sandbox.run(&["--bind", &initial_bind, "echo", "initial setup"])?;
    
    // Try to run another command with a DIFFERENT bind mount - this should fail
    let different_bind = format!("{}:{}", bind_source2, "/mnt/test");
    
    let result = sandbox.run(&["--bind", &different_bind, "echo", "should fail"]);
    
    // This should fail because bind mount configuration changed
    assert!(result.is_err(), "Expected bind mount validation to fail when bind mounts change");
    
    // Check that the error message shows detailed changes
    let error_output = format!("{}{}", sandbox.last_stderr, sandbox.last_stdout);
    assert!(
        error_output.contains("Bind mount configuration has changed") &&
        error_output.contains("Removed bind mounts:") &&
        error_output.contains("Added bind mounts:"),
        "Expected detailed bind mount change information, got: {}",
        error_output
    );
    
    Ok(())
}

#[rstest] 
fn test_bind_mount_validation_success(mut sandbox: SandboxManager) -> Result<()> {
    // Create temporary directory using the test filename pattern
    let bind_source = sandbox.test_filename("bind-source-same");
    fs::create_dir_all(&bind_source)?;
    fs::write(format!("{}/test.txt", bind_source), "test content")?;
    
    // Create and start sandbox with bind mount
    let bind_mount = format!("{}:{}", bind_source, "/mnt/test");
    let _output1 = sandbox.run(&["--bind", &bind_mount, "echo", "first command"])?;
    
    // Run another command with the SAME bind mount - this should succeed
    let _output2 = sandbox.run(&["--bind", &bind_mount, "echo", "second command"])?;
    
    Ok(())
}

#[rstest]
fn test_bind_mount_options_validation(mut sandbox: SandboxManager) -> Result<()> {
    // Create temporary directory using the test filename pattern
    let bind_source = sandbox.test_filename("bind-source-options");
    fs::create_dir_all(&bind_source)?;
    
    // Start with read-write bind mount
    let bind_rw = format!("{}:{}", bind_source, "/mnt/test");
    let _output1 = sandbox.run(&["--bind", &bind_rw, "echo", "read-write"])?;
    
    // Try to change to read-only bind mount - this should fail
    let bind_ro = format!("{}:{}:ro", bind_source, "/mnt/test");
    let result = sandbox.run(&["--bind", &bind_ro, "echo", "read-only"]);
    
    // This should fail because bind mount options changed
    assert!(result.is_err(), "Expected bind mount validation to fail when options change");
    
    Ok(())
}