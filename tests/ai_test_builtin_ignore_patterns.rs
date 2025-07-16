mod fixtures;

use anyhow::Result;
use fixtures::*;
use rstest::*;

#[rstest]
fn test_builtin_ignore_patterns_tmp(mut sandbox: SandboxManager) -> Result<()> {
    sandbox.set_ignored(false);

    // Create parent .gitignore to ensure test directory isn't ignored
    let parent_gitignore_path =
        format!("generated-test-data/{}/.gitignore", &sandbox.name);
    sandbox.run(&[
        "sh",
        "-c",
        &format!("echo '!*/' > {}", parent_gitignore_path),
    ])?;

    // Create files in /tmp that would normally be tracked
    sandbox.run(&["sh", "-c", "mkdir -p /tmp/sandbox-builtin-test && echo 'test' > /tmp/sandbox-builtin-test/should-be-ignored.txt"])?;

    // Also create a visible file for comparison
    let test_dir = sandbox.test_filename("tmp-test");
    std::fs::create_dir_all(&test_dir)?;
    sandbox.run(&[
        "sh",
        "-c",
        &format!("echo 'visible' > {}/visible-file.txt", test_dir),
    ])?;

    // First verify with --ignored flag that /tmp files exist as changes
    sandbox.set_ignored(true);
    sandbox.run(&["status", "/"])?;
    let stdout_ignored = sandbox.last_stdout.clone();
    sandbox.set_ignored(false);

    // Run without --ignored
    sandbox.run(&["status", "/"])?;
    let stdout = sandbox.last_stdout.clone();

    // The /tmp file should appear with --ignored but not without
    assert!(
        stdout_ignored.contains("sandbox-builtin-test")
            || stdout_ignored.contains("should-be-ignored"),
        "Files in /tmp should show with --ignored flag. Got: {}",
        stdout_ignored
    );

    assert!(
        !stdout.contains("sandbox-builtin-test")
            && !stdout.contains("should-be-ignored"),
        "Files in /tmp should be ignored by built-in patterns. Got: {}",
        stdout
    );

    // The visible file should always show
    assert!(
        stdout.contains("visible-file.txt"),
        "Visible file should be shown. Got: {}",
        stdout
    );

    // Clean up
    sandbox.run(&["sh", "-c", "rm -rf /tmp/sandbox-builtin-test"])?;

    Ok(())
}

#[rstest]
fn test_builtin_ignore_patterns_home_dotfiles(
    mut sandbox: SandboxManager,
) -> Result<()> {
    sandbox.set_ignored(false);

    // Create parent .gitignore to ensure test directory isn't ignored
    let parent_gitignore_path =
        format!("generated-test-data/{}/.gitignore", &sandbox.name);
    sandbox.run(&[
        "sh",
        "-c",
        &format!("echo '!*/' > {}", parent_gitignore_path),
    ])?;

    // The patterns /home/*/.*/** and /home/*/.*  ignore hidden files in home dirs
    // Create hidden files in user's home
    sandbox.run(&["sh", "-c", "mkdir -p ~/.sandbox-builtin-test && echo 'hidden' > ~/.sandbox-builtin-test/file.txt"])?;
    sandbox.run(&[
        "sh",
        "-c",
        "echo 'hidden' > ~/.sandbox-builtin-hidden-file",
    ])?;

    // Create a visible file for comparison in test directory
    let test_dir = sandbox.test_filename("home-test");
    std::fs::create_dir_all(&test_dir)?;
    sandbox.run(&[
        "sh",
        "-c",
        &format!("echo 'visible' > {}/visible-file.txt", test_dir),
    ])?;

    // Run with --ignored first to confirm files were created
    sandbox.set_ignored(true);
    sandbox.run(&["status", "/"])?;
    let stdout_ignored = sandbox.last_stdout.clone();
    sandbox.set_ignored(false);

    // Run without --ignored
    sandbox.run(&["status", "/"])?;
    let stdout = sandbox.last_stdout.clone();

    // Hidden files should show with --ignored but not without
    assert!(
        stdout_ignored.contains(".sandbox-builtin")
            || stdout_ignored.contains("hidden-file"),
        "Hidden files should show with --ignored flag. Got: {}",
        stdout_ignored
    );

    assert!(
        !stdout.contains(".sandbox-builtin") && !stdout.contains("hidden-file"),
        "Hidden files in home should be ignored by built-in patterns. Got: {}",
        stdout
    );

    // Visible file should always show
    assert!(
        stdout.contains("visible-file.txt"),
        "Visible file should be shown. Got: {}",
        stdout
    );

    // Clean up
    sandbox.run(&[
        "sh",
        "-c",
        "rm -rf ~/.sandbox-builtin-test ~/.sandbox-builtin-hidden-file",
    ])?;

    Ok(())
}

#[rstest]
fn test_builtin_ignore_patterns_git(mut sandbox: SandboxManager) -> Result<()> {
    sandbox.set_ignored(false);

    // Create parent .gitignore to ensure test directory isn't ignored
    let parent_gitignore_path =
        format!("generated-test-data/{}/.gitignore", &sandbox.name);
    sandbox.run(&[
        "sh",
        "-c",
        &format!("echo '!*/' > {}", parent_gitignore_path),
    ])?;

    // Create test directory
    let test_dir = sandbox.test_filename("builtin-git-test");
    std::fs::create_dir_all(&test_dir)?;

    // The patterns **/.git/** and **/.git ignore .git directories
    // Create a .git directory with files
    sandbox.run(&[
        "sh",
        "-c",
        &format!(
            "mkdir -p {}/.git && echo 'gitconfig' > {}/.git/config",
            test_dir, test_dir
        ),
    ])?;
    sandbox.run(&[
        "sh",
        "-c",
        &format!("echo 'HEAD' > {}/.git/HEAD", test_dir),
    ])?;

    // Create regular file
    sandbox.run(&[
        "sh",
        "-c",
        &format!("echo 'regular' > {}/regular-file.txt", test_dir),
    ])?;

    // Run with --ignored first
    sandbox.set_ignored(true);
    sandbox.run(&["status", &test_dir])?;
    let stdout_ignored = sandbox.last_stdout.clone();
    sandbox.set_ignored(false);

    // Run without --ignored
    sandbox.run(&["status", &test_dir])?;
    let stdout = sandbox.last_stdout.clone();

    // .git files should show with --ignored but not without
    assert!(
        stdout_ignored.contains(".git"),
        ".git directory should show with --ignored flag. Got: {}",
        stdout_ignored
    );

    assert!(
        !stdout.contains(".git"),
        ".git directory should be ignored by built-in patterns. Got: {}",
        stdout
    );

    // Regular file should always show
    assert!(
        stdout.contains("regular-file.txt"),
        "Regular file should be shown. Got: {}",
        stdout
    );

    Ok(())
}
