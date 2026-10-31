mod fixtures;

use anyhow::Result;
use fixtures::*;
use rstest::*;
use std::fs;
use std::path::PathBuf;

const UNAVAILABLE: &str = "Failed to set up resource limits";

fn available_cpus() -> usize {
    std::thread::available_parallelism().unwrap().get()
}

fn write_config(
    sandbox: &SandboxManager,
    prefix: &str,
    contents: &str,
) -> Result<String> {
    let path = format!("{}.toml", sandbox.test_filename(prefix));
    fs::write(&path, contents)?;
    Ok(path)
}

/// Starts the sandbox with a CPU limit to find out if this machine lets us
/// manage cgroups. When it doesn't, e.g. because we're running inside a
/// container or another sandbox, we check that we fail with a helpful error.
fn limits_available(sandbox: &mut SandboxManager) -> bool {
    if sandbox.pass(&["--cpus=1", "true"]) {
        return true;
    }
    assert!(
        sandbox.last_stderr.contains(UNAVAILABLE),
        "{}",
        sandbox.last_stderr
    );
    eprintln!("cgroup resource limits are not available, skipping");
    false
}

/// Finds our sandbox's cgroup on the host, if it has one
fn host_cgroup(sandbox: &SandboxManager) -> Option<PathBuf> {
    let suffix = format!("-{}", sandbox.name);
    fs::read_dir("/sys/fs/cgroup/sandbox")
        .ok()?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .find(|path| path.to_string_lossy().ends_with(&suffix))
}

/*
 * Configuration
 */

#[rstest]
fn ai_test_limits_unlimited_by_default(
    mut sandbox: SandboxManager,
) -> Result<()> {
    // Skip config files, which may set limits
    sandbox.run(&["--no-config", "config", "cpus", "memory"])?;
    assert!(sandbox.last_stdout.contains("cpus=unlimited"));
    assert!(sandbox.last_stdout.contains("memory=unlimited"));

    // Both keys are part of the full config listing
    sandbox.run(&["--no-config", "config"])?;
    assert!(sandbox.last_stdout.contains("cpus=unlimited"));
    assert!(sandbox.last_stdout.contains("memory=unlimited"));
    Ok(())
}

#[rstest]
fn ai_test_limits_cli(mut sandbox: SandboxManager) -> Result<()> {
    sandbox.run(&["--cpus=2", "config", "cpus"])?;
    assert_eq!(sandbox.last_stdout.trim(), "2");

    sandbox.run(&["--cpus=1.5", "config", "cpus"])?;
    assert_eq!(sandbox.last_stdout.trim(), "1.5");

    // Negative values are relative to the number of available CPUs, and
    // work with or without an equals sign
    let expected = (available_cpus() - 1).to_string();
    sandbox.run(&["--cpus=-1", "config", "cpus"])?;
    assert_eq!(sandbox.last_stdout.trim(), expected);
    sandbox.run(&["--cpus", "-1", "config", "cpus"])?;
    assert_eq!(sandbox.last_stdout.trim(), expected);

    sandbox.run(&["--memory=512M", "config", "memory"])?;
    assert_eq!(sandbox.last_stdout.trim(), (512u64 << 20).to_string());

    sandbox.run(&["--memory", "1.5G", "config", "memory"])?;
    assert_eq!(sandbox.last_stdout.trim(), (3u64 << 29).to_string());
    Ok(())
}

#[rstest]
fn ai_test_limits_env(mut sandbox: SandboxManager) -> Result<()> {
    // Config files are skipped so they can't set limits of their own
    let args = ["--no-config", "config"];
    sandbox.run_with_env(
        &[&args[..], &["cpus"]].concat(),
        "SANDBOX_CPUS",
        "3",
    )?;
    assert_eq!(sandbox.last_stdout.trim(), "3");

    sandbox.run_with_env(
        &[&args[..], &["memory"]].concat(),
        "SANDBOX_MEMORY",
        "2G",
    )?;
    assert_eq!(sandbox.last_stdout.trim(), (2u64 << 30).to_string());

    // Empty values are ignored
    sandbox.run_with_env(
        &[&args[..], &["cpus"]].concat(),
        "SANDBOX_CPUS",
        "",
    )?;
    assert_eq!(sandbox.last_stdout.trim(), "unlimited");

    // The CLI takes precedence over the environment
    sandbox.run_with_env(
        &["--no-config", "--cpus=1", "config", "cpus"],
        "SANDBOX_CPUS",
        "3",
    )?;
    assert_eq!(sandbox.last_stdout.trim(), "1");
    Ok(())
}

#[rstest]
fn ai_test_limits_config_file(mut sandbox: SandboxManager) -> Result<()> {
    let config =
        write_config(&sandbox, "limits", "cpus = -1\nmemory = \"4G\"\n")?;
    sandbox.run(&["--config", &config, "config", "cpus", "memory"])?;
    assert!(
        sandbox
            .last_stdout
            .contains(&format!("cpus={}", available_cpus() - 1))
    );
    assert!(
        sandbox
            .last_stdout
            .contains(&format!("memory={}", 4u64 << 30))
    );

    // Numbers and strings are both accepted
    let config = write_config(
        &sandbox,
        "limits-types",
        "cpus = \"0.5\"\nmemory = 1073741824\n",
    )?;
    sandbox.run(&["--config", &config, "config", "cpus", "memory"])?;
    assert!(sandbox.last_stdout.contains("cpus=0.5"));
    assert!(sandbox.last_stdout.contains("memory=1073741824"));

    // The environment takes precedence over config files
    sandbox.run_with_env(
        &["--config", &config, "config", "cpus"],
        "SANDBOX_CPUS",
        "2",
    )?;
    assert_eq!(sandbox.last_stdout.trim(), "2");
    Ok(())
}

#[rstest]
fn ai_test_limits_invalid(mut sandbox: SandboxManager) -> Result<()> {
    for cpus in ["--cpus=0", "--cpus=abc", "--cpus=0.001"] {
        assert!(sandbox.xfail(&[cpus, "config", "cpus"]));
        assert!(
            sandbox.last_stderr.contains("Invalid cpus value"),
            "{}",
            sandbox.last_stderr
        );
    }

    let all_cpus = format!("--cpus=-{}", available_cpus());
    assert!(sandbox.xfail(&[&all_cpus, "config", "cpus"]));
    assert!(sandbox.last_stderr.contains("leaves 0 of the"));

    assert!(sandbox.xfail(&["--memory=lots", "config", "memory"]));
    assert!(sandbox.last_stderr.contains("Invalid memory value 'lots'"));

    let config = write_config(&sandbox, "bad-type", "cpus = true\n")?;
    assert!(sandbox.xfail(&["--config", &config, "config", "cpus"]));
    Ok(())
}

/*
 * Enforcement
 */

#[rstest]
fn ai_test_limits_applied(mut sandbox: SandboxManager) -> Result<()> {
    if !limits_available(&mut sandbox) {
        return Ok(());
    }
    sandbox.run(&["stop"])?;

    let args = ["--cpus=2", "--memory=256M"];
    let run = |sandbox: &mut SandboxManager, command: &[&str]| {
        let mut full = args.to_vec();
        full.extend_from_slice(command);
        sandbox
            .run(&full)
            .map(|_| sandbox.last_stdout.trim().to_string())
    };

    // Our limits are visible inside the sandbox, whose cgroup namespace is
    // rooted at the sandbox's cgroup
    assert_eq!(
        run(&mut sandbox, &["cat", "/sys/fs/cgroup/cpu.max"])?,
        "200000 100000"
    );
    assert_eq!(
        run(&mut sandbox, &["cat", "/sys/fs/cgroup/memory.max"])?,
        (256u64 << 20).to_string()
    );
    assert_eq!(run(&mut sandbox, &["cat", "/proc/self/cgroup"])?, "0::/");

    // Processes that join the running sandbox are in its cgroup too
    let cgroup = host_cgroup(&sandbox).expect("sandbox cgroup should exist");
    let mut child =
        sandbox.run_in_background(&[args[0], args[1], "sleep", "30"])?;
    let mut joined = false;
    for _ in 0..100 {
        let procs = fs::read_to_string(cgroup.join("cgroup.procs"))?;
        if procs.lines().count() > 1 {
            joined = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert!(joined, "sleep should have joined the sandbox cgroup");

    // Stopping the sandbox removes its cgroup
    sandbox.run(&["stop"])?;
    let _ = child.wait();
    assert!(!cgroup.exists());
    Ok(())
}

#[rstest]
fn ai_test_limits_negative_cpus(mut sandbox: SandboxManager) -> Result<()> {
    if !limits_available(&mut sandbox) {
        return Ok(());
    }
    sandbox.run(&["stop"])?;

    sandbox.run(&["--cpus=-1", "cat", "/sys/fs/cgroup/cpu.max"])?;
    assert_eq!(
        sandbox.last_stdout.trim(),
        format!("{} 100000", (available_cpus() - 1) * 100000)
    );
    Ok(())
}

#[rstest]
fn ai_test_limits_memory_enforced(mut sandbox: SandboxManager) -> Result<()> {
    if !limits_available(&mut sandbox) {
        return Ok(());
    }
    sandbox.run(&["stop"])?;

    // tail has to buffer the whole newline-free input, which won't fit
    assert!(sandbox.xfail(&[
        "--memory=32M",
        "sh",
        "-c",
        "head -c 256M /dev/zero | tail > /dev/null",
    ]));

    sandbox.run(&["--memory=32M", "cat", "/sys/fs/cgroup/memory.events"])?;
    let oom_kills: u64 = sandbox
        .last_stdout
        .lines()
        .find_map(|line| line.strip_prefix("oom_kill "))
        .expect("memory.events should have an oom_kill entry")
        .parse()?;
    assert!(oom_kills >= 1);
    Ok(())
}

#[rstest]
fn ai_test_limits_changed(mut sandbox: SandboxManager) -> Result<()> {
    if !limits_available(&mut sandbox) {
        return Ok(());
    }

    // Restart without config files, which may set limits of their own
    sandbox.run(&["stop"])?;
    sandbox.run(&["--no-config", "--cpus=1", "true"])?;

    // The sandbox is running with --cpus=1, so other limits are rejected
    assert!(sandbox.xfail(&["--no-config", "true"]));
    assert!(sandbox.last_stderr.contains("Resource limits have changed"));
    assert!(
        sandbox
            .last_stderr
            .contains("Existing: cpus=1 memory=unlimited")
    );
    assert!(
        sandbox
            .last_stderr
            .contains("Current: cpus=unlimited memory=unlimited")
    );

    assert!(sandbox.xfail(&["--no-config", "--cpus=1", "--memory=1G", "true"]));
    assert!(
        sandbox
            .last_stderr
            .contains(&format!("Current: cpus=1 memory={} bytes", 1u64 << 30))
    );

    // The same limits are fine, and stopping lets us change them
    sandbox.run(&["--no-config", "--cpus=1", "true"])?;
    sandbox.run(&["stop"])?;
    sandbox.run(&["--no-config", "--cpus=2", "true"])?;
    Ok(())
}

#[rstest]
fn ai_test_limits_stale_cgroup(mut sandbox: SandboxManager) -> Result<()> {
    if !limits_available(&mut sandbox) {
        return Ok(());
    }

    // Kill the sandbox without cleaning up, leaving an empty cgroup behind
    let cgroup = host_cgroup(&sandbox).expect("sandbox cgroup should exist");
    sandbox.run(&["config", "sandbox_dir"])?;
    let pid_file = format!("{}.pid", sandbox.last_stdout.trim());
    let pid = fs::read_to_string(&pid_file)?;
    std::process::Command::new("sudo")
        .args(["kill", "-9", pid.trim()])
        .status()?;
    for _ in 0..100 {
        if fs::read_to_string(cgroup.join("cgroup.procs"))?.is_empty() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert!(cgroup.exists());

    // Starting again replaces the stale cgroup
    sandbox.run(&["--cpus=2", "cat", "/sys/fs/cgroup/cpu.max"])?;
    assert_eq!(sandbox.last_stdout.trim(), "200000 100000");
    Ok(())
}

#[rstest]
fn ai_test_limits_cgroup_in_use(mut sandbox: SandboxManager) -> Result<()> {
    if !limits_available(&mut sandbox) {
        return Ok(());
    }

    // A sandbox with the same name in another storage directory would
    // need the same cgroup, which is in use
    let storage_dir =
        std::env::current_dir()?.join(sandbox.test_filename("storage"));
    fs::create_dir_all(&storage_dir)?;
    let storage_arg = format!("--storage-dir={}", storage_dir.display());
    assert!(sandbox.xfail(&[&storage_arg, "--cpus=1", "true"]));
    assert!(
        sandbox
            .last_stderr
            .contains("from a different storage directory")
    );
    Ok(())
}

#[rstest]
fn ai_test_limits_setup_failures(mut sandbox: SandboxManager) -> Result<()> {
    assert!(sandbox.exfail(&["--cpus=1", "true"], "TEST_NO_CGROUP2", "1"));
    assert!(sandbox.last_stderr.contains(UNAVAILABLE));
    assert!(
        sandbox
            .last_stderr
            .contains("requires cgroup v2, which is not mounted")
    );

    if !limits_available(&mut sandbox) {
        return Ok(());
    }
    sandbox.run(&["stop"])?;

    // Failing to apply limits cleans up the cgroup
    for (file, args) in [
        ("cpu.max", vec!["--cpus=1", "true"]),
        ("memory.max", vec!["--memory=1G", "true"]),
    ] {
        assert!(sandbox.exfail(&args, "TEST_CGROUP_WRITE_FAILURE", file));
        assert!(sandbox.last_stderr.contains(UNAVAILABLE));
        assert!(sandbox.last_stderr.contains("Failed to write"));
        assert!(host_cgroup(&sandbox).is_none());
    }

    // As does failing to start the sandbox
    assert!(sandbox.exfail(
        &["--cpus=1", "true"],
        "TEST_START_SANDBOX_FAILURE",
        "1"
    ));
    assert!(sandbox.last_stderr.contains("Failed to start sandbox"));
    assert!(host_cgroup(&sandbox).is_none());
    Ok(())
}

#[rstest]
fn ai_test_limits_new_parent_cgroup(mut sandbox: SandboxManager) -> Result<()> {
    if !limits_available(&mut sandbox) {
        return Ok(());
    }
    sandbox.run(&["stop"])?;

    // The first sandbox with limits creates the parent cgroup and enables
    // the controllers we need in it
    let parent = format!("{}-parent", sandbox.name);
    let parent_path = PathBuf::from("/sys/fs/cgroup").join(&parent);
    assert!(!parent_path.exists());
    sandbox.run_with_env(
        &["--cpus=1", "--memory=64M", "cat", "/sys/fs/cgroup/cpu.max"],
        "TEST_CGROUP_PARENT",
        &parent,
    )?;
    assert_eq!(sandbox.last_stdout.trim(), "100000 100000");
    let enabled =
        fs::read_to_string(parent_path.join("cgroup.subtree_control"))?;
    assert!(enabled.split_whitespace().any(|c| c == "cpu"));
    assert!(enabled.split_whitespace().any(|c| c == "memory"));

    sandbox.run_with_env(&["stop"], "TEST_CGROUP_PARENT", &parent)?;
    assert_eq!(
        fs::read_dir(&parent_path)?
            .filter(|e| { e.as_ref().is_ok_and(|e| e.path().is_dir()) })
            .count(),
        0
    );
    std::process::Command::new("sudo")
        .args(["rmdir", &parent_path.to_string_lossy()])
        .status()?;
    Ok(())
}

#[rstest]
fn ai_test_limits_no_swap_accounting(
    mut sandbox: SandboxManager,
) -> Result<()> {
    if !limits_available(&mut sandbox) {
        return Ok(());
    }
    sandbox.run(&["stop"])?;

    // Without swap accounting we can only limit memory, not swap
    sandbox.run_with_env(
        &["--memory=64M", "cat", "/sys/fs/cgroup/memory.max"],
        "TEST_NO_SWAP_ACCOUNTING",
        "1",
    )?;
    assert_eq!(sandbox.last_stdout.trim(), (64u64 << 20).to_string());
    sandbox.run(&["--memory=64M", "cat", "/sys/fs/cgroup/memory.swap.max"])?;
    assert_eq!(sandbox.last_stdout.trim(), "max");
    sandbox.run(&["stop"])?;

    // Normally swap is disabled
    sandbox.run(&["--memory=64M", "cat", "/sys/fs/cgroup/memory.swap.max"])?;
    assert_eq!(sandbox.last_stdout.trim(), "0");
    Ok(())
}

#[rstest]
fn ai_test_limits_join_failure(mut sandbox: SandboxManager) -> Result<()> {
    if !limits_available(&mut sandbox) {
        return Ok(());
    }

    // The sandbox is running with --cpus=1, joining it requires moving
    // into its cgroup
    assert!(sandbox.exfail(
        &["--cpus=1", "true"],
        "TEST_CGROUP_WRITE_FAILURE",
        "cgroup.procs"
    ));
    assert!(sandbox.last_stderr.contains("Failed to write"));
    assert!(sandbox.last_stderr.contains("cgroup.procs"));
    Ok(())
}

#[rstest]
fn ai_test_limits_stop_cgroup_busy(mut sandbox: SandboxManager) -> Result<()> {
    if !limits_available(&mut sandbox) {
        return Ok(());
    }

    // A child cgroup keeps the sandbox's cgroup from being removed, so stop
    // retries for a while and then gives up without failing
    let cgroup = host_cgroup(&sandbox).expect("sandbox cgroup should exist");
    let child = cgroup.join("busy");
    let sudo =
        |args: &[&str]| std::process::Command::new("sudo").args(args).status();
    sudo(&["mkdir", &child.to_string_lossy()])?;

    let started = std::time::Instant::now();
    sandbox.run(&["stop"])?;
    assert!(started.elapsed() >= std::time::Duration::from_secs(5));
    assert!(cgroup.exists());

    sudo(&["rmdir", &child.to_string_lossy()])?;
    sudo(&["rmdir", &cgroup.to_string_lossy()])?;
    assert!(!cgroup.exists());
    Ok(())
}
