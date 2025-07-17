mod fixtures;

use anyhow::Result;
use fixtures::*;
use rstest::*;
use std::path::Path;

#[rstest]
fn test_can_mkdir_nested_paths(mut sandbox: SandboxManager) -> Result<()> {
    // Create test directory
    let test_dir = sandbox.test_filename("can-mkdir-test");
    std::fs::create_dir_all(&test_dir)?;

    // Create a deeply nested path that doesn't exist
    // This will force can_mkdir to traverse up the directory tree
    let nested_path = Path::new(&test_dir)
        .join("level1")
        .join("level2")
        .join("level3")
        .join("level4");

    // Try to create a file in this deeply nested path that doesn't exist
    // This should trigger the can_mkdir check with multiple parent traversals
    sandbox.run(&[
        "sh",
        "-c",
        &format!(
            "mkdir -p {} && touch {}/test.txt",
            nested_path.to_str().unwrap(),
            nested_path.to_str().unwrap()
        ),
    ])?;

    // Verify the file was created successfully
    sandbox.run(&["status", &test_dir])?;
    let stdout = sandbox.last_stdout.clone();

    assert!(
        stdout.contains("test.txt"),
        "test.txt should be created in deeply nested directory"
    );

    Ok(())
}

#[rstest]
fn test_sandbox_creation_nested_storage(
    mut sandbox: SandboxManager,
) -> Result<()> {
    let nested_storage = format!(
        "/tmp/{}",
        sandbox.test_filename("nested/storage/path/for/sandbox")
    );

    sandbox.epass(&["true"], "SANDBOX_STORAGE_DIR", &nested_storage);
    sandbox.epass(
        &["accept", "**/*.profraw"],
        "SANDBOX_STORAGE_DIR",
        &nested_storage,
    );

    sandbox.epass(&["stop"], "SANDBOX_STORAGE_DIR", &nested_storage);
    sandbox.epass(&["delete"], "SANDBOX_STORAGE_DIR", &nested_storage);

    Ok(())
}
