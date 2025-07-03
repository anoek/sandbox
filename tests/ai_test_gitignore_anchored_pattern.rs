mod fixtures;

use anyhow::Result;
use fixtures::*;
use rstest::*;
use std::path::Path;

#[rstest]
fn test_gitignore_anchored_directory_pattern(
    mut sandbox: SandboxManager,
) -> Result<()> {
    sandbox.set_ignored(false);  // Don't automatically add --ignored for gitignore tests

    // Create a working directory using test_filename
    let test_dir = sandbox.test_filename("anchored-dir-test");
    std::fs::create_dir_all(&test_dir)?;
    
    // Create parent .gitignore to un-ignore test directory
    let parent_gitignore_path =
        format!("generated-test-data/{}/.gitignore", &sandbox.name);
    sandbox.run(&[
        "sh",
        "-c",
        &format!("echo '!*/' > {}", parent_gitignore_path),
    ])?;

    // Test the /target pattern - should ignore target and everything under it
    // Create directories first (on host filesystem)
    std::fs::create_dir_all(Path::new(&test_dir).join("target/debug"))?;
    std::fs::create_dir_all(Path::new(&test_dir).join("target/release/deps"))?;
    std::fs::create_dir_all(Path::new(&test_dir).join("src/target"))?; // Should NOT be ignored
    std::fs::create_dir_all(Path::new(&test_dir).join("build/output"))?;
    std::fs::create_dir_all(Path::new(&test_dir).join("dist/assets"))?;
    std::fs::create_dir_all(Path::new(&test_dir).join("src"))?;

    // Create .gitignore file within the sandbox
    let gitignore_contents = r#"
# Anchored directory pattern
/target
# Also test other anchored patterns
/build
/dist/
"#;
    let gitignore_path = Path::new(&test_dir).join(".gitignore");
    sandbox.run(&[
        "sh",
        "-c",
        &format!("cat > {} << 'EOF'\n{}EOF", gitignore_path.to_str().unwrap(), gitignore_contents),
    ])?;

    // Create test files - we need to create them in the sandbox to see changes
    let test_files = vec![
        // Files under /target - should all be ignored
        ("target/foo", true),
        ("target/debug/app", true),
        ("target/release/deps/lib.so", true),
        // Files under src/target - should NOT be ignored (not anchored)
        ("src/target/file.txt", false),
        // Files under /build - should be ignored
        ("build/output/result.txt", true),
        // Files under /dist - should be ignored
        ("dist/assets/style.css", true),
        // Other files - should not be ignored
        ("README.md", false),
        ("src/main.rs", false),
    ];

    // Create files within the sandbox
    for (file, _) in &test_files {
        let file_path = Path::new(&test_dir).join(file);
        sandbox.run(&[
            "sh",
            "-c",
            &format!("echo 'test content' > {}", file_path.to_str().unwrap()),
        ])?;
    }

    // Run status WITHOUT --ignored flag to see only non-ignored files
    sandbox.run(&["status", &test_dir])?;
    let status_without_ignored = sandbox.last_stdout.clone();

    // Now run with --ignored flag to see all files
    // We need to temporarily set ignored to true to add the flag
    sandbox.set_ignored(true);
    sandbox.run(&["status", &test_dir])?;
    let status_with_ignored = sandbox.last_stdout.clone();
    sandbox.set_ignored(false);  // Reset it back

    // Check which files appear in each output
    for (file, should_be_ignored) in &test_files {
        let file_path = Path::new(&test_dir).join(file);
        let file_str = file_path.to_str().unwrap();
        
        if *should_be_ignored {
            // Ignored files should NOT appear in status without --ignored
            assert!(
                !status_without_ignored.contains(file_str),
                "Expected {} to be ignored (not shown without --ignored flag)",
                file
            );
            // But they SHOULD appear in status with --ignored
            assert!(
                status_with_ignored.contains(file_str),
                "Expected {} to appear when using --ignored flag",
                file
            );
        } else {
            // Non-ignored files should appear in both outputs
            assert!(
                status_without_ignored.contains(file_str),
                "Expected {} to be shown (not ignored)",
                file
            );
            assert!(
                status_with_ignored.contains(file_str),
                "Expected {} to appear when using --ignored flag",
                file
            );
        }
    }

    // The test verifies that:
    // 1. /target pattern correctly ignores files under target/ at root
    // 2. /target pattern does NOT ignore src/target/ (not anchored to root)
    // 3. /build and /dist/ patterns work as expected for anchored directories

    Ok(())
}
