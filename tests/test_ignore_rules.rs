mod fixtures;

use anyhow::Result;
use fixtures::*;
use rstest::*;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

#[rstest]
fn test_gitignore_functionality(mut sandbox: SandboxManager) -> Result<()> {
    let dir = sandbox.test_filename_no_rid("gitignore-test-dir");
    std::fs::create_dir_all(&dir)?;

    // Remove the repository-level .gitignore inside the sandbox so its rules do not interfere
    // with the assertions in this test.
    sandbox.run(&["rm", ".gitignore"])?;

    let gitignore_contents = r#"
# ignore single file
ignored-file

# ignore entire directory
ignored-dir/

# ignore all *.tmp in dir but keep one
dir/*.tmp
!dir/keep.tmp

# anchored ignore relative to this gitignore directory
/anchored-file

# ignore a directory but allow a child
negate-me
!negate-me/keep
"#;
    std::fs::write(Path::new(&dir).join(".gitignore"), gitignore_contents)?;

    // Files that SHOULD be ignored
    let ignored_file = Path::new(&dir).join("ignored-file");
    let ignored_dir_file = Path::new(&dir).join("ignored-dir/afile");
    let ignored_tmp = Path::new(&dir).join("dir/tmp.tmp");
    let ignored_negate_dir = Path::new(&dir).join("negate-me/file.txt");
    let anchored_file = Path::new(&dir).join("anchored-file");

    // Files that should NOT be ignored by /anchored-file rule
    let non_anchored_1 = Path::new(&dir).join("nested/anchored-file");
    let non_anchored_2 = Path::new(&dir).join("nested/deeper/anchored-file");

    // Files that should NOT be ignored (explicit negations)
    let included_file = Path::new(&dir).join("included-file");
    let included_keep_tmp = Path::new(&dir).join("dir/keep.tmp");
    let included_negate_keep = Path::new(&dir).join("negate-me/keep");

    sandbox.run(&[
        "mkdir",
        "-p",
        ignored_dir_file.parent().unwrap().to_str().unwrap(),
        included_keep_tmp.parent().unwrap().to_str().unwrap(),
        included_negate_keep.parent().unwrap().to_str().unwrap(),
        non_anchored_1.parent().unwrap().to_str().unwrap(),
        non_anchored_2.parent().unwrap().to_str().unwrap(),
    ])?;

    for path in [
        &ignored_file,
        &ignored_dir_file,
        &ignored_tmp,
        &ignored_negate_dir,
        &anchored_file,
        &non_anchored_1,
        &non_anchored_2,
        &included_file,
        &included_keep_tmp,
        &included_negate_keep,
    ] {
        sandbox.run(&["touch", path.to_str().unwrap()])?;
    }

    // 1) Run `status` WITHOUT the --ignored flag (direct command invocation)
    let output = Command::new("sudo")
        .args(["-E", &sandbox.sandbox_bin])
        .args(["-v", &format!("--name={}", &sandbox.name), "status", "/"])
        .output()?;
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Files that should be present
    for inc in [
        &included_file,
        &included_keep_tmp,
        &included_negate_keep,
        &non_anchored_1,
        &non_anchored_2,
    ] {
        assert!(stdout.contains(inc.to_str().unwrap()));
    }
    // The primary ignored file and anchored file should *not* be present
    for ign in [&ignored_file, &anchored_file] {
        assert!(!stdout.contains(ign.to_str().unwrap()));
    }

    // 2) Run `status` WITH the --ignored flag via SandboxManager (flag added automatically)
    sandbox.run(&["status", "/"])?;
    let stdout_flag = &sandbox.last_stdout;
    // Now every file should appear because we are passing --ignored implicitly
    for path in [
        &ignored_file,
        &ignored_dir_file,
        &ignored_tmp,
        &ignored_negate_dir,
        &anchored_file,
        &non_anchored_1,
        &non_anchored_2,
        &included_file,
        &included_keep_tmp,
        &included_negate_keep,
        &non_anchored_1,
        &non_anchored_2,
    ] {
        assert!(stdout_flag.contains(path.to_str().unwrap()));
    }

    Ok(())
}

/// Verify that the SANDBOX_IGNORED environment variable enables inclusion of
/// ignored files even when the --ignored CLI flag is not provided.
#[rstest]
fn test_env_flag_includes_ignored(mut sandbox: SandboxManager) -> Result<()> {
    // Prepare isolated dir
    let dir = sandbox.test_filename_no_rid("env-ignore-test");
    std::fs::create_dir_all(&dir)?;

    // Remove top-level .gitignore in sandbox
    sandbox.run(&["rm", ".gitignore"])?;

    // Simple .gitignore ignoring a single file
    std::fs::write(Path::new(&dir).join(".gitignore"), "ignored\n")?;

    let ignored_file = Path::new(&dir).join("ignored");
    let included_file = Path::new(&dir).join("included");

    // Create the files (using --ignored via helper, fine for creation phase)
    sandbox.run(&["touch", ignored_file.to_str().unwrap()])?;
    sandbox.run(&["touch", included_file.to_str().unwrap()])?;

    // Call `status` WITHOUT the flag but WITH env var set
    let output = Command::new("sudo")
        .args(["-E", &sandbox.sandbox_bin])
        .args(["-v", &format!("--name={}", &sandbox.name), "status", "/"])
        .output()?;
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    // The ignored file should not be present, the included file should be
    assert!(!stdout.contains(ignored_file.to_str().unwrap()));
    assert!(stdout.contains(included_file.to_str().unwrap()));

    // Call `status` WITHOUT the flag but WITH env var set
    let output = Command::new("sudo")
        .env("SANDBOX_IGNORED", "true")
        .args(["-E", &sandbox.sandbox_bin])
        .args(["-v", &format!("--name={}", &sandbox.name), "status", "/"])
        .output()?;
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Both files should be reported now
    assert!(stdout.contains(ignored_file.to_str().unwrap()));
    assert!(stdout.contains(included_file.to_str().unwrap()));

    Ok(())
}

#[rstest]
fn test_builtin_tmp_ignore(mut sandbox: SandboxManager) -> Result<()> {
    let ignored_file: PathBuf =
        PathBuf::from(format!("/tmp/builtin-ignore-{}", rid()));

    sandbox.run(&["touch", ignored_file.to_str().unwrap()])?;

    let output = std::process::Command::new("sudo")
        .args(["-E", &sandbox.sandbox_bin])
        .args(["-v", &format!("--name={}", &sandbox.name), "status", "/"])
        .output()?;
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(!stdout.contains(ignored_file.to_str().unwrap()));

    // implicitly --ignored
    sandbox.run(&["status", "/"])?;
    let stdout_flag = &sandbox.last_stdout;

    assert!(stdout_flag.contains(ignored_file.to_str().unwrap()));

    Ok(())
}
