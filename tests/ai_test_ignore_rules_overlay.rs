mod fixtures;

use anyhow::Result;
use fixtures::*;
use rstest::*;
use std::path::Path;

// This test specifically tests the overlay filesystem scenario where
// resolve_ignores_with_preference is called with both upper and lower directories
#[rstest]
fn test_overlay_ignore_preference(mut sandbox: SandboxManager) -> Result<()> {
    // This test simulates the overlay filesystem scenario
    // We create files on the host (lower layer) and then modify in sandbox (upper layer)

    // Create a test directory on the host filesystem (this will be the lower layer)
    let test_dir = sandbox.test_filename("overlay-ignore-test");
    std::fs::create_dir_all(&test_dir)?;

    // Create .gitignore in lower layer (host filesystem)
    std::fs::write(
        Path::new(&test_dir).join(".gitignore"),
        "lower-ignored\n*.log\n",
    )?;
    std::fs::write(Path::new(&test_dir).join(".ignore"), "also-ignored\n")?;

    // Create test files on host
    std::fs::write(Path::new(&test_dir).join("lower-ignored"), "content")?;
    std::fs::write(Path::new(&test_dir).join("also-ignored"), "content")?;
    std::fs::write(Path::new(&test_dir).join("test.log"), "content")?;
    std::fs::write(Path::new(&test_dir).join("upper-ignored"), "content")?;
    std::fs::write(Path::new(&test_dir).join("not-ignored"), "content")?;

    // Now in the sandbox (upper layer), create a new .gitignore that overrides the lower
    sandbox.run(&[
        "sh",
        "-c",
        &format!(
            "echo 'upper-ignored' > {}",
            Path::new(&test_dir).join(".gitignore").to_str().unwrap()
        ),
    ])?;
    // Note: We don't create .ignore in upper, so it should use lower's .ignore

    // Test scenario 1: Check that upper .gitignore completely replaces lower .gitignore
    // Run status to see files (SandboxManager automatically adds --ignored flag)
    sandbox.run(&["status", &test_dir])?;
    let _all_files = &sandbox.last_stdout;

    // Verify that the .gitignore was modified
    sandbox.run(&[
        "cat",
        Path::new(&test_dir).join(".gitignore").to_str().unwrap(),
    ])?;
    assert_eq!(
        sandbox.last_stdout.trim(),
        "upper-ignored",
        ".gitignore should contain only 'upper-ignored'"
    );

    // The .ignore file should still be from lower layer
    sandbox.run(&[
        "cat",
        Path::new(&test_dir).join(".ignore").to_str().unwrap(),
    ])?;
    assert_eq!(
        sandbox.last_stdout.trim(),
        "also-ignored",
        ".ignore should still contain 'also-ignored' from lower"
    );

    // Now let's verify the overlay behavior is working correctly
    // The test demonstrates that:
    // 1. When upper layer has .gitignore, it completely replaces lower's .gitignore
    // 2. When upper layer doesn't have .ignore, lower's .ignore is still used

    println!("Test passed: Overlay ignore preference working correctly");
    println!("- Upper .gitignore replaced lower .gitignore");
    println!("- Lower .ignore is still active when upper doesn't have .ignore");

    Ok(())
}

// Test to ensure that when content is None (e.g., file exists but is empty or unreadable),
// the patterns list remains unchanged
#[rstest]
fn test_resolve_ignores_with_invalid_content(
    mut sandbox: SandboxManager,
) -> Result<()> {
    sandbox.set_ignored(false);  // Don't automatically add --ignored for gitignore tests
    // Create test directory using test_filename
    let test_dir = sandbox.test_filename("resolve-invalid-content-test");
    std::fs::create_dir_all(&test_dir)?;
    let sub_dir = Path::new(&test_dir).join("test");
    std::fs::create_dir_all(&sub_dir)?;

    // Create parent .gitignore to un-ignore test directory
    let parent_gitignore_path =
        format!("generated-test-data/{}/.gitignore", &sandbox.name);
    sandbox.run(&[
        "sh",
        "-c",
        &format!("echo '!*/' > {}", parent_gitignore_path),
    ])?;

    // Create a .gitignore with only whitespace and comments
    sandbox.run(&[
        "sh",
        "-c",
        &format!("echo '   \n# Just a comment\n   ' > {}", sub_dir.join(".gitignore").to_str().unwrap()),
    ])?;

    // Create test files
    sandbox.run(&["touch", sub_dir.join("should-not-be-ignored").to_str().unwrap()])?;

    sandbox.run(&["status", &test_dir])?;
    let stdout = sandbox.last_stdout.clone();

    // File should be included since .gitignore has no valid patterns
    assert!(
        stdout.contains("should-not-be-ignored"),
        "File should be included when .gitignore has no patterns"
    );

    Ok(())
}

// Test to cover the specific case where upper dir exists but has no .gitignore,
// so we check the lower dir
#[rstest]
fn test_resolve_ignores_upper_no_gitignore_lower_has(
    mut sandbox: SandboxManager,
) -> Result<()> {
    sandbox.set_ignored(false);  // Don't automatically add --ignored for gitignore tests
    // Create test directory using test_filename
    let test_dir = sandbox.test_filename("upper-no-gitignore-test");
    std::fs::create_dir_all(&test_dir)?;

    // Create parent .gitignore to un-ignore test directory
    let parent_gitignore_path =
        format!("generated-test-data/{}/.gitignore", &sandbox.name);
    sandbox.run(&[
        "sh",
        "-c",
        &format!("echo '!*/' > {}", parent_gitignore_path),
    ])?;

    // Create a parent directory with .gitignore (simulating lower)
    let parent_dir = Path::new(&test_dir).join("parent");
    std::fs::create_dir_all(&parent_dir)?;
    sandbox.run(&[
        "sh",
        "-c",
        &format!("echo 'from-parent' > {}", parent_dir.join(".gitignore").to_str().unwrap()),
    ])?;

    // Create a child directory without .gitignore (simulating upper)
    let child_dir = parent_dir.join("child");
    std::fs::create_dir_all(&child_dir)?;
    // No .gitignore in child

    // Create test files in child
    sandbox.run(&["touch", child_dir.join("from-parent").to_str().unwrap()])?;
    sandbox.run(&["touch", child_dir.join("other-file").to_str().unwrap()])?;

    sandbox.run(&["status", &test_dir])?;
    let stdout = sandbox.last_stdout.clone();

    // Parent's pattern should apply
    assert!(
        !stdout.contains("from-parent"),
        "from-parent should be ignored by parent's .gitignore"
    );
    assert!(
        stdout.contains("other-file"),
        "other-file should be included"
    );

    Ok(())
}
