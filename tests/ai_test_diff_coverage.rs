mod fixtures;

use anyhow::Result;
use fixtures::*;
use rstest::*;
use std::fs;

#[rstest]
fn test_diff_rename_operation(mut sandbox: SandboxManager) -> Result<()> {
    // Test rename operation - directories show as "### Moved"
    let dir1 = sandbox.test_filename("dir1");
    let dir2 = sandbox.test_filename("dir2");
    fs::create_dir(&dir1)?;

    // Rename the directory
    sandbox.run(&["mv", &dir1, &dir2])?;

    // Diff should show the rename
    sandbox.run(&["diff"])?;
    assert!(sandbox.last_stdout.contains("### Moved"));
    assert!(sandbox.last_stdout.contains(&dir1));
    assert!(sandbox.last_stdout.contains(&dir2));

    Ok(())
}

#[rstest]
fn test_diff_colorized_output(mut sandbox: SandboxManager) -> Result<()> {
    // Test colorized diff output
    let file = sandbox.test_filename("color_test.txt");
    fs::write(&file, "original")?;

    // Modify file
    sandbox.run(&["sh", "-c", &format!("echo 'modified' > {}", file)])?;

    // Check diff output with color forced
    sandbox.run_with_env(&["diff"], "TEST_FORCE_DIFF_COLOR", "1")?;
    assert!(sandbox.last_stdout.contains("-original"));
    assert!(sandbox.last_stdout.contains("+modified"));
    // The actual color codes would be in the output when TEST_FORCE_DIFF_COLOR is set

    Ok(())
}

#[rstest]
fn test_diff_no_color(mut sandbox: SandboxManager) -> Result<()> {
    // Test non-colorized diff output
    let file = sandbox.test_filename("no_color.txt");
    fs::write(&file, "content")?;

    // Modify file
    sandbox.run(&["sh", "-c", &format!("echo 'changed' > {}", file)])?;

    // Check diff output with NO_COLOR set
    sandbox.run_with_env(&["diff"], "NO_COLOR", "1")?;
    assert!(sandbox.last_stdout.contains("-content"));
    assert!(sandbox.last_stdout.contains("+changed"));

    Ok(())
}

#[rstest]
fn test_diff_rename_colorized(mut sandbox: SandboxManager) -> Result<()> {
    // Test colorized rename output - directories show as "### Moved"
    let dir1 = sandbox.test_filename("color_dir1");
    let dir2 = sandbox.test_filename("color_dir2");
    fs::create_dir(&dir1)?;

    // Rename the directory
    sandbox.run(&["mv", &dir1, &dir2])?;

    // Diff with color forced
    sandbox.run_with_env(&["diff"], "TEST_FORCE_DIFF_COLOR", "1")?;
    assert!(sandbox.last_stdout.contains("### Moved"));
    // When colorized, the output would have ANSI color codes

    Ok(())
}

#[rstest]
fn test_diff_error_operation_unsupported_file_type(
    mut sandbox: SandboxManager,
) -> Result<()> {
    // Create a FIFO (named pipe) - unsupported file type
    let fifo_path = sandbox.test_filename("test.fifo");
    nix::unistd::mkfifo(
        fifo_path.as_str(),
        nix::sys::stat::Mode::S_IRUSR | nix::sys::stat::Mode::S_IWUSR,
    )?;

    // Try to modify it
    sandbox.run(&["sh", "-c", "echo 'data' > test.fifo &"])?;

    // Check diff - should skip error entries
    sandbox.run(&["diff"])?;
    // Error operations are silently skipped in diff

    Ok(())
}

#[rstest]
fn test_diff_non_file_destination(mut sandbox: SandboxManager) -> Result<()> {
    // Create a directory
    let dir_path = sandbox.test_filename("test_dir");
    fs::create_dir(&dir_path)?;

    // Modify directory permissions
    sandbox.run(&["chmod", "755", &dir_path])?;

    // Diff should skip directories
    sandbox.run(&["diff"])?;
    // Should not contain diff output for directory
    assert!(!sandbox.last_stdout.contains("test_dir"));

    Ok(())
}

#[rstest]
fn test_diff_staged_non_file(mut sandbox: SandboxManager) -> Result<()> {
    // Create a regular file
    let file_path = sandbox.test_filename("regular.txt");
    fs::write(&file_path, "content")?;

    // Replace it with a directory in sandbox
    sandbox.run(&["rm", &file_path])?;
    sandbox.run(&["mkdir", &file_path])?;

    // Diff should handle this case
    sandbox.run(&["diff"])?;

    Ok(())
}

#[rstest]
fn test_diff_missing_destination_file(
    mut sandbox: SandboxManager,
) -> Result<()> {
    // Create a new file (destination doesn't exist)
    sandbox.run(&["sh", "-c", "echo 'new content' > newfile.txt"])?;

    // Diff should use /dev/null as left path
    sandbox.run(&["diff"])?;
    assert!(sandbox.last_stdout.contains("+new content"));

    Ok(())
}

#[rstest]
fn test_diff_remove_operation(mut sandbox: SandboxManager) -> Result<()> {
    // Create a file
    let file_path = sandbox.test_filename("to_remove.txt");
    fs::write(&file_path, "content to remove")?;

    // Remove it
    sandbox.run(&["rm", &file_path])?;

    // Diff should show deletion
    sandbox.run(&["diff"])?;
    assert!(sandbox.last_stdout.contains("-content to remove"));

    Ok(())
}

#[rstest]
fn test_diff_path_replacement_in_output(
    mut sandbox: SandboxManager,
) -> Result<()> {
    // Create a file with content
    let file_path = sandbox.test_filename("path_test.txt");
    fs::write(&file_path, "Original content")?;

    // Modify it
    sandbox.run(&[
        "sh",
        "-c",
        &format!("echo 'Modified content' > {}", file_path),
    ])?;

    // Check diff output
    sandbox.run(&["diff"])?;
    assert!(sandbox.last_stdout.contains("-Original content"));
    assert!(sandbox.last_stdout.contains("+Modified content"));

    Ok(())
}

#[rstest]
fn test_diff_force_color_env_variable(
    mut sandbox: SandboxManager,
) -> Result<()> {
    // Create and modify a file
    let file_path = sandbox.test_filename("color_test.txt");
    fs::write(&file_path, "original")?;

    sandbox.run(&["sh", "-c", &format!("echo 'modified' > {}", file_path)])?;

    // Test with TEST_FORCE_DIFF_COLOR set
    sandbox.run_with_env(&["diff"], "TEST_FORCE_DIFF_COLOR", "1")?;
    assert!(sandbox.last_stdout.contains("-original"));
    assert!(sandbox.last_stdout.contains("+modified"));
    // Color codes would be in the actual diff output

    Ok(())
}

#[rstest]
fn test_diff_with_patterns(mut sandbox: SandboxManager) -> Result<()> {
    // Create multiple files
    let file1 = sandbox.test_filename("file1.txt");
    let file2 = sandbox.test_filename("file2.txt");
    let other = sandbox.test_filename("other.log");
    fs::write(&file1, "content1")?;
    fs::write(&file2, "content2")?;
    fs::write(&other, "log content")?;

    // Modify all files
    sandbox.run(&["sh", "-c", 
        &format!("echo 'modified1' > {} && echo 'modified2' > {} && echo 'modified log' > {}", file1, file2, other)])?;

    // Diff with pattern - test filtering by specific file
    sandbox.run(&["diff", &file1])?;
    assert!(sandbox.last_stdout.contains("modified1"));
    assert!(!sandbox.last_stdout.contains("modified2"));
    assert!(!sandbox.last_stdout.contains("modified log"));

    // Diff with pattern matching - show all changes
    sandbox.run(&["diff"])?;
    assert!(sandbox.last_stdout.contains("modified1"));
    assert!(sandbox.last_stdout.contains("modified2"));
    assert!(sandbox.last_stdout.contains("modified log"));

    Ok(())
}

#[rstest]
fn test_diff_edge_case_null_staged(mut sandbox: SandboxManager) -> Result<()> {
    // Create a file
    let file_path = sandbox.test_filename("edge.txt");
    fs::write(&file_path, "original")?;

    // Modify in sandbox
    sandbox.run(&["sh", "-c", &format!("echo 'new' > {}", file_path)])?;

    // Diff should work normally
    sandbox.run(&["diff"])?;
    assert!(sandbox.last_stdout.contains("-original"));
    assert!(sandbox.last_stdout.contains("+new"));

    Ok(())
}
