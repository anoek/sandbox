mod fixtures;

use std::path::PathBuf;

use anyhow::Result;
use fixtures::*;
use rstest::*;

#[rstest]
fn test_list() -> Result<()> {
    let mut sandbox1 = sandbox();
    let mut sandbox2 = sandbox();

    sandbox1.run(&["true"])?;
    sandbox2.run(&["true"])?;

    let name1 = sandbox1.name.clone();
    let name2 = sandbox2.name.clone();

    sandbox1.run(&["list"])?;
    let running = sandbox1.last_stdout.clone();
    println!("looking for {} in {}", name1, running);
    assert!(running.contains(&name1));
    assert!(running.contains(&name2));

    sandbox1.run(&["list", &name1])?;
    let running = sandbox1.last_stdout.clone();
    println!("looking for {} and {} in {}", name1, name2, running);
    assert!(running.contains(&name1));
    assert!(!running.contains(&name2));

    sandbox1.run(&["list", &name1, &name2])?;
    let running = sandbox1.last_stdout.clone();
    assert!(running.contains(&name1));
    assert!(running.contains(&name2));

    sandbox1.run(&["list", "non existent"])?;
    let running = sandbox1.last_stdout.clone();
    assert!(!running.contains(&name1));
    assert!(!running.contains(&name2));
    Ok(())
}

#[rstest]
fn test_list_ignore_bad_pid_file(mut sandbox: SandboxManager) -> Result<()> {
    sandbox.run(&["config", "sandbox_dir"])?;
    let mut t = PathBuf::from(sandbox.last_stdout.clone());
    println!("t: {}", t.display());
    t.pop();
    println!("t: {}", t.display());
    let bad_sandbox_base =
        format!("{}/{}", t.to_string_lossy(), "coverage_test_bad_pid_file");
    println!("bad_sandbox_base: {}", bad_sandbox_base);

    let pid_file_name = format!("{}.pid", bad_sandbox_base);
    let lock_file_name = format!("{}.lock", bad_sandbox_base);

    std::fs::write(&lock_file_name, "1")?;
    std::fs::write(&pid_file_name, "not a number")?;

    println!("pid_file_name: {}", pid_file_name);
    let pid_file = std::fs::File::open(&pid_file_name)?;
    let pid_file_content = std::io::read_to_string(pid_file)?;
    println!("pid_file_content: {}", pid_file_content);
    assert!(pid_file_content.parse::<i32>().is_err());

    assert!(sandbox.run(&["list"]).is_ok());
    // still expected to be listed
    assert!(sandbox.all_stdout.contains("coverage_test_bad_pid_file"));

    std::fs::remove_file(&pid_file_name)?;
    std::fs::remove_file(&lock_file_name)?;

    Ok(())
}

#[rstest]
fn test_list_stale_pid_file(mut sandbox: SandboxManager) -> Result<()> {
    sandbox.run(&["config", "sandbox_dir"])?;
    let mut t = PathBuf::from(sandbox.last_stdout.clone());
    t.pop();
    let stale_sandbox_base =
        format!("{}/{}", t.to_string_lossy(), sandbox.name.clone());

    let pid_file_name = format!("{}.pid", stale_sandbox_base);

    sandbox.run(&["true"])?;
    let pid = std::fs::read_to_string(&pid_file_name)?;
    sandbox.run(&["stop"])?;

    // stop cleans up our pid, but let's restore it so we have a stale pid
    // file that shouldn't conflict with any existing processes on the system
    // at this point.
    std::fs::write(&pid_file_name, pid)?;

    println!("pid_file_name: {}", pid_file_name);
    let pid_file = std::fs::File::open(&pid_file_name)?;
    let pid_file_content = std::io::read_to_string(pid_file)?;
    println!("pid_file_content: {}", pid_file_content);
    assert!(pid_file_content.parse::<i32>().is_ok());

    assert!(sandbox.run(&["list", &sandbox.name.clone()]).is_ok());
    eprintln!(">>>> {} <<<<", sandbox.last_stdout);
    eprintln!(">>>> {} <<<<", sandbox.last_stderr);
    // still expected to be listed
    assert!(sandbox.all_stdout.contains("Stopped"));
    assert!(sandbox.all_stdout.contains(&sandbox.name));

    Ok(())
}
