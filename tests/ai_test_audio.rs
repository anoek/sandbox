mod fixtures;

use anyhow::Result;
use fixtures::*;
use rstest::*;
use std::fs;
use std::path::Path;

/// Host paths that --audio should bind into the sandbox, limited to the ones
/// that exist on this machine
fn host_audio_paths() -> Vec<String> {
    let mut paths = vec![];
    if Path::new("/dev/snd").exists() {
        paths.push("/dev/snd".to_string());
    }
    if let Ok(xdg_runtime_dir) = std::env::var("XDG_RUNTIME_DIR") {
        for name in ["pipewire-0", "pulse"] {
            let path = Path::new(&xdg_runtime_dir).join(name);
            if path.exists() {
                paths.push(path.to_string_lossy().to_string());
            }
        }
    }
    paths
}

/// Writes a config file with the given contents and returns its path
fn write_config(
    sandbox: &SandboxManager,
    prefix: &str,
    contents: &str,
) -> Result<String> {
    let path = format!("{}.toml", sandbox.test_filename(prefix));
    fs::write(&path, contents)?;
    Ok(path)
}

#[rstest]
fn ai_test_audio_disabled_by_default(
    mut sandbox: SandboxManager,
) -> Result<()> {
    sandbox.run(&["--no-config", "config", "audio"])?;
    assert_eq!(sandbox.last_stdout.trim(), "false");

    sandbox.run(&["--no-config", "config", "bind"])?;
    for path in host_audio_paths() {
        assert!(!sandbox.last_stdout.contains(&path));
    }

    Ok(())
}

#[rstest]
fn ai_test_audio_flag(mut sandbox: SandboxManager) -> Result<()> {
    sandbox.run(&["--no-config", "--audio", "config", "audio"])?;
    assert_eq!(sandbox.last_stdout.trim(), "true");

    sandbox.run(&["--no-config", "--audio", "config", "bind"])?;
    for path in host_audio_paths() {
        assert!(sandbox.last_stdout.contains(&path));
    }

    Ok(())
}

#[rstest]
fn ai_test_audio_env(mut sandbox: SandboxManager) -> Result<()> {
    sandbox.run_with_env(
        &["--no-config", "config", "audio"],
        "SANDBOX_AUDIO",
        "true",
    )?;
    assert_eq!(sandbox.last_stdout.trim(), "true");

    sandbox.run_with_env(
        &["--no-config", "config", "audio"],
        "SANDBOX_AUDIO",
        "false",
    )?;
    assert_eq!(sandbox.last_stdout.trim(), "false");

    assert!(sandbox.exfail(
        &["--no-config", "config", "audio"],
        "SANDBOX_AUDIO",
        "yes"
    ));
    assert!(
        sandbox
            .last_stderr
            .contains("Invalid value for SANDBOX_AUDIO")
    );

    Ok(())
}

#[rstest]
fn ai_test_audio_config_file(mut sandbox: SandboxManager) -> Result<()> {
    let config = write_config(&sandbox, "audio", "audio = true")?;

    sandbox.run(&["--config", &config, "config", "audio"])?;
    assert_eq!(sandbox.last_stdout.trim(), "true");

    // The environment takes precedence over config files
    sandbox.run_with_env(
        &["--config", &config, "config", "audio"],
        "SANDBOX_AUDIO",
        "false",
    )?;
    assert_eq!(sandbox.last_stdout.trim(), "false");

    Ok(())
}

#[rstest]
fn ai_test_audio_with_no_default_binds(
    mut sandbox: SandboxManager,
) -> Result<()> {
    sandbox.run(&[
        "--no-config",
        "--no-default-binds",
        "--audio",
        "config",
        "bind",
    ])?;
    assert!(!sandbox.last_stdout.contains("/dev/fuse"));
    for path in host_audio_paths() {
        assert!(sandbox.last_stdout.contains(&path));
    }

    Ok(())
}

#[rstest]
fn ai_test_audio_mounted_in_sandbox(mut sandbox: SandboxManager) -> Result<()> {
    let paths = host_audio_paths();

    for path in &paths {
        assert!(sandbox.xfail(&["--no-config", "test", "-e", path]));
    }

    // Changing audio for a running sandbox requires a restart
    assert!(sandbox.xfail(&["--no-config", "--audio", "true"]));
    sandbox.run(&["stop"])?;

    for path in &paths {
        assert!(sandbox.pass(&["--no-config", "--audio", "test", "-e", path]));
    }

    // Parent directories created for the sockets mirror the host's ownership
    // so that clients like pactl will trust XDG_RUNTIME_DIR
    if let Ok(xdg_runtime_dir) = std::env::var("XDG_RUNTIME_DIR") {
        if Path::new(&xdg_runtime_dir).join("pulse").exists() {
            sandbox.run(&[
                "--no-config",
                "--audio",
                "stat",
                "-c",
                "%u %a",
                &xdg_runtime_dir,
            ])?;
            let host = fs::metadata(&xdg_runtime_dir)?;
            use std::os::unix::fs::MetadataExt;
            assert_eq!(
                sandbox.last_stdout.trim(),
                format!("{} {:o}", host.uid(), host.mode() & 0o7777)
            );
        }
    }

    Ok(())
}

#[rstest]
fn ai_test_audio_without_xdg_runtime_dir(
    mut sandbox: SandboxManager,
) -> Result<()> {
    // Without XDG_RUNTIME_DIR we can't find the sockets, but /dev/snd is
    // still bound if it exists
    sandbox.run_with_env(
        &["--no-config", "--audio", "config", "bind"],
        "XDG_RUNTIME_DIR",
        "",
    )?;
    assert!(!sandbox.last_stdout.contains("pipewire-0"));
    assert!(!sandbox.last_stdout.contains("/pulse"));
    assert_eq!(
        sandbox.last_stdout.contains("/dev/snd"),
        Path::new("/dev/snd").exists()
    );

    Ok(())
}
