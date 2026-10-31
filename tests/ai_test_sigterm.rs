mod fixtures;

use anyhow::Result;
use fixtures::*;
use rstest::*;
use std::path::Path;
use std::time::{Duration, Instant};

#[rstest]
fn ai_test_sigterm_stops_sandbox(mut sandbox: SandboxManager) -> Result<()> {
    sandbox.run(&["true"])?;

    sandbox.run(&["config", "sandbox_dir"])?;
    let pid_file = format!("{}.pid", sandbox.last_stdout.trim());
    let pid = std::fs::read_to_string(&pid_file)?.trim().to_string();
    let proc_dir = format!("/proc/{}", pid);
    assert!(Path::new(&proc_dir).exists());

    // The sandbox's PID 1 exits when asked to terminate
    let status = std::process::Command::new("sudo")
        .args(["kill", "-TERM", &pid])
        .status()?;
    assert!(status.success());
    let deadline = Instant::now() + Duration::from_secs(5);
    while Path::new(&proc_dir).exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(!Path::new(&proc_dir).exists());

    // and the sandbox starts again the next time it's used
    sandbox.run(&["echo", "restarted"])?;
    assert_eq!(sandbox.last_stdout.trim(), "restarted");
    Ok(())
}
