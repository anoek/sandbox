mod fixtures;

use anyhow::Result;
use fixtures::*;
use rstest::*;

#[rstest]
fn test_diff_patterns(mut sandbox: SandboxManager) -> Result<()> {
    sandbox.run(&["sh", "-c", "echo 'file_one' > file_one"])?;
    sandbox.run(&["sh", "-c", "echo 'file_two' > file_two"])?;

    // diff for all files should show both changes
    sandbox.run(&["diff"])?;
    assert!(sandbox.last_stdout.contains("file_one"));
    assert!(sandbox.last_stdout.contains("file_two"));

    // root diff should show both files too
    sandbox.run(&["diff", "/"])?;
    assert!(sandbox.last_stdout.contains("file_one"));
    assert!(sandbox.last_stdout.contains("file_two"));

    // diff filtered to only the first file should not include the second
    sandbox.run(&["diff", "file_one"])?;
    assert!(sandbox.last_stdout.contains("file_one"));
    assert!(!sandbox.last_stdout.contains("file_two"));

    // diff filtered to a pattern that matches nothing should have neither
    sandbox.run(&["diff", "non_existent_pattern"])?;
    assert!(!sandbox.last_stdout.contains("file_one"));
    assert!(!sandbox.last_stdout.contains("file_two"));

    Ok(())
}

#[rstest]
fn test_diff_operations(mut sandbox: SandboxManager) -> Result<()> {
    sandbox.set_debug_mode(true);
    let added = sandbox.test_filename("added_file");
    let removed = sandbox.test_filename("removed_file");
    let modified = sandbox.test_filename("modified_file");
    let moved_src = sandbox.test_filename("moved_dir");
    let moved_dst = sandbox.test_filename("moved_dir_new");

    std::fs::write(&removed, "to be removed")?;
    std::fs::write(&modified, "old content")?;
    std::fs::create_dir(&moved_src)?;

    sandbox.run(&["sh", "-c", &format!("echo 'added' > {}", added)])?;
    sandbox.run(&["rm", &removed])?;
    sandbox.run(&[
        "bash",
        "-c",
        &format!("echo 'new content' > {}", modified),
    ])?;
    sandbox.run(&["mv", &moved_src, &moved_dst])?;

    sandbox.run(&["diff"])?;
    assert!(sandbox.last_stdout.contains(&added));
    assert!(sandbox.last_stdout.contains(&removed));
    assert!(sandbox.last_stdout.contains(&modified));
    assert!(sandbox.last_stdout.contains("+new content"));
    assert!(sandbox.last_stdout.contains("### Moved"));
    assert!(sandbox.last_stdout.contains(&moved_src));
    assert!(sandbox.last_stdout.contains(&moved_dst));

    Ok(())
}
