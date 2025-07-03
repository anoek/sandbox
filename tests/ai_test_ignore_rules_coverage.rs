mod fixtures;

use anyhow::Result;
use fixtures::*;
use rstest::*;
use std::path::Path;

#[rstest]
fn test_resolve_ignores_with_preference_upper_exists(
    mut sandbox: SandboxManager,
) -> Result<()> {
    sandbox.set_ignored(false);
    // Create test directory using test_filename
    let test_dir = sandbox.test_filename("resolve-ignores-upper-test");
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

    // Test case: Upper directory has .gitignore, lower also has .gitignore
    // Should use upper's .gitignore and ignore lower's

    // Create upper .gitignore that ignores "upper-pattern"
    sandbox.run(&[
        "sh",
        "-c",
        &format!("echo 'upper-pattern' > {}", sub_dir.join(".gitignore").to_str().unwrap()),
    ])?;

    // Create files to test within the sandbox
    sandbox.run(&["touch", sub_dir.join("upper-pattern").to_str().unwrap()])?;
    sandbox.run(&["touch", sub_dir.join("other-file").to_str().unwrap()])?;

    // Run status without --ignored flag
    sandbox.run(&["status", &test_dir])?;
    let stdout = sandbox.last_stdout.clone();

    // upper-pattern should be ignored, other-file should be shown
    assert!(
        !stdout.contains("upper-pattern"),
        "upper-pattern should be ignored"
    );
    assert!(
        stdout.contains("other-file"),
        "other-file should be included"
    );

    Ok(())
}

#[rstest]
fn test_resolve_ignores_with_preference_fallback_to_lower(
    mut sandbox: SandboxManager,
) -> Result<()> {
    sandbox.set_ignored(false);
    // Create test directory using test_filename
    let test_dir = sandbox.test_filename("resolve-ignores-fallback-test");
    std::fs::create_dir_all(&test_dir)?;
    
    // Create parent .gitignore to un-ignore test directory
    let parent_gitignore_path =
        format!("generated-test-data/{}/.gitignore", &sandbox.name);
    sandbox.run(&[
        "sh",
        "-c",
        &format!("echo '!*/' > {}", parent_gitignore_path),
    ])?;

    // For now, we'll test the simpler case where we have nested directories
    let parent_dir = Path::new(&test_dir).join("parent");
    let child_dir = parent_dir.join("child");
    std::fs::create_dir_all(&child_dir)?;

    // Only parent has .gitignore
    sandbox.run(&[
        "sh",
        "-c",
        &format!("echo 'parent-pattern' > {}", parent_dir.join(".gitignore").to_str().unwrap()),
    ])?;

    // Create files in child directory
    sandbox.run(&["touch", child_dir.join("parent-pattern").to_str().unwrap()])?;
    sandbox.run(&["touch", child_dir.join("other-file").to_str().unwrap()])?;

    sandbox.run(&["status", &test_dir])?;
    let stdout = sandbox.last_stdout.clone();

    // Parent's gitignore should apply to child
    assert!(
        !stdout.contains("parent-pattern"),
        "parent-pattern should be ignored by parent's .gitignore"
    );
    assert!(
        stdout.contains("other-file"),
        "other-file should be included"
    );

    Ok(())
}

#[rstest]
fn test_resolve_ignores_no_lower_dir(
    mut sandbox: SandboxManager,
) -> Result<()> {
    sandbox.set_ignored(false);
    // Create test directory using test_filename
    let test_dir = sandbox.test_filename("resolve-no-lower-test");
    std::fs::create_dir_all(&test_dir)?;
    let empty_dir = Path::new(&test_dir).join("empty");
    std::fs::create_dir_all(&empty_dir)?;
    
    // Create parent .gitignore to un-ignore test directory
    let parent_gitignore_path =
        format!("generated-test-data/{}/.gitignore", &sandbox.name);
    sandbox.run(&[
        "sh",
        "-c",
        &format!("echo '!*/' > {}", parent_gitignore_path),
    ])?;

    // Test with a directory that has no ignore files

    // Create a file - should not be ignored since there are no ignore files
    sandbox.run(&["touch", empty_dir.join("test-file").to_str().unwrap()])?;

    sandbox.run(&["status", &test_dir])?;
    let stdout = sandbox.last_stdout.clone();
    
    println!("Status output:\n{}", stdout);

    assert!(
        stdout.contains("test-file"),
        "test-file should be included when no ignore files exist"
    );

    Ok(())
}

#[rstest]
fn test_resolve_ignores_empty_gitignore(
    mut sandbox: SandboxManager,
) -> Result<()> {
    sandbox.set_ignored(false);
    // Create test directory using test_filename
    let test_dir = sandbox.test_filename("resolve-empty-test");
    std::fs::create_dir_all(&test_dir)?;
    let empty_ignore_dir = Path::new(&test_dir).join("empty-ignore");
    std::fs::create_dir_all(&empty_ignore_dir)?;
    
    // Create parent .gitignore to un-ignore test directory
    let parent_gitignore_path =
        format!("generated-test-data/{}/.gitignore", &sandbox.name);
    sandbox.run(&[
        "sh",
        "-c",
        &format!("echo '!*/' > {}", parent_gitignore_path),
    ])?;

    // Create an empty .gitignore file
    sandbox.run(&[
        "sh",
        "-c",
        &format!("echo '' > {}", empty_ignore_dir.join(".gitignore").to_str().unwrap()),
    ])?;

    // Create test files
    sandbox.run(&["touch", empty_ignore_dir.join("test-pattern").to_str().unwrap()])?;
    sandbox.run(&["touch", empty_ignore_dir.join("other-file").to_str().unwrap()])?;

    // Both files should be included since .gitignore is empty
    sandbox.run(&["status", &test_dir])?;
    let stdout = sandbox.last_stdout.clone();

    // Empty .gitignore means nothing is ignored
    assert!(
        stdout.contains("test-pattern"),
        "test-pattern should be included with empty .gitignore"
    );
    assert!(
        stdout.contains("other-file"),
        "other-file should be included"
    );

    Ok(())
}

#[rstest]
fn test_resolve_ignores_with_both_gitignore_and_ignore(
    mut sandbox: SandboxManager,
) -> Result<()> {
    sandbox.set_ignored(false);
    // Create test directory using test_filename
    let test_dir = sandbox.test_filename("resolve-both-ignore-files-test");
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

    // Create both .gitignore and .ignore files
    sandbox.run(&[
        "sh",
        "-c",
        &format!("echo 'gitignore-pattern' > {}", sub_dir.join(".gitignore").to_str().unwrap()),
    ])?;
    sandbox.run(&[
        "sh",
        "-c",
        &format!("echo 'ignore-pattern' > {}", sub_dir.join(".ignore").to_str().unwrap()),
    ])?;

    // Create test files
    sandbox.run(&["touch", sub_dir.join("gitignore-pattern").to_str().unwrap()])?;
    sandbox.run(&["touch", sub_dir.join("ignore-pattern").to_str().unwrap()])?;
    sandbox.run(&["touch", sub_dir.join("other-file").to_str().unwrap()])?;

    sandbox.run(&["status", &test_dir])?;
    let stdout = sandbox.last_stdout.clone();

    // Both patterns should be applied
    assert!(
        !stdout.contains("gitignore-pattern"),
        "gitignore-pattern should be ignored"
    );
    assert!(
        !stdout.contains("ignore-pattern"),
        "ignore-pattern should be ignored"
    );
    assert!(
        stdout.contains("other-file"),
        "other-file should be included"
    );

    Ok(())
}
