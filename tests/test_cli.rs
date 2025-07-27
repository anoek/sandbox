mod fixtures;

use anyhow::Result;
use fixtures::*;
use rstest::*;

#[rstest]
fn test_cli(mut sandbox: SandboxManager) -> Result<()> {
    assert!(sandbox.pass(&["--version"]));
    assert!(sandbox.pass(&["--", "ls"]));
    assert!(sandbox.xfail(&["--bad-option"]));

    assert!(sandbox.pass(&["--net", "config"]));
    assert!(sandbox.last_stdout.contains("net=host"));
    assert!(sandbox.pass(&["--net=none", "config"]));
    assert!(sandbox.last_stdout.contains("net=none"));
    assert!(sandbox.pass(&["--net=host", "config"]));
    assert!(sandbox.last_stdout.contains("net=host"));
    assert!(sandbox.xfail(&["--net=foobar", "config"]));

    assert!(sandbox.pass(&["--name", "test-cli", "config"]));
    assert!(sandbox.last_stdout.contains("name=test-cli"));
    assert!(sandbox.pass(&["--name=test-cli", "config"]));
    assert!(sandbox.last_stdout.contains("name=test-cli"));

    assert!(sandbox.pass(&["--storage-dir=/tmp/sandbox", "config"]));
    assert!(sandbox.last_stdout.contains("storage_dir=/tmp/sandbox"));

    assert!(sandbox.pass(&["--storage-dir=/tmp/sandbox", "config"]));
    assert!(sandbox.last_stdout.contains("storage_dir=/tmp/sandbox"));

    // default values check
    sandbox.no_default_options = true;
    assert!(sandbox.pass(&["--no-config", "config"]));
    assert!(sandbox.last_stdout.contains("name=sandbox"));
    assert!(sandbox.last_stdout.contains("net=none"));

    // Test config_files display
    assert!(sandbox.pass(&["--no-config", "config", "config_files"]));
    assert!(sandbox.last_stdout == "\n"); // Should be empty with --no-config

    Ok(())
}

#[rstest]
fn test_cli_with_config(mut sandbox: SandboxManager) -> Result<()> {
    let parent_config = r#"
    net = "none"
    name = "parent"
    log_level = "trace"
    storage_dir = "/tmp/sandbox1"
    bind = [".cargo"]
    "#;
    let config = r#"
    net = "host"
    name = "test-cli-with-config"
    log_level = "info"
    storage_dir = "/tmp/sandbox2"
    bind = [".cargo"]
    "#;

    let _ = std::fs::remove_file(".sandbox.toml");
    let _ = std::fs::remove_file("../.sandbox.conf");

    std::fs::write("../.sandbox.conf", parent_config)?;
    sandbox.no_default_options = true;
    assert!(sandbox.epass(&["config"], "SANDBOX_BIND", ".cargo"));
    assert!(sandbox.last_stdout.contains("name=parent"));
    assert!(sandbox.last_stdout.contains("net=none"));
    assert!(sandbox.last_stdout.contains("log_level=TRACE"));
    assert!(sandbox.last_stdout.contains("storage_dir=/tmp/sandbox1"));
    assert!(sandbox.last_stdout.contains(".cargo"));

    std::fs::write(".sandbox.toml", config)?;
    sandbox.no_default_options = true;
    assert!(sandbox.pass(&["config", "--bind=.cargo"]));
    assert!(sandbox.last_stdout.contains("name=test-cli-with-config"));
    assert!(sandbox.last_stdout.contains("net=host"));
    println!("{}", sandbox.last_stdout);
    assert!(sandbox.last_stdout.contains("log_level=INFO"));
    assert!(sandbox.last_stdout.contains("storage_dir=/tmp/sandbox2"));

    std::fs::remove_file(".sandbox.toml")?;
    std::fs::remove_file("../.sandbox.conf")?;

    Ok(())
}

#[rstest]
fn test_cli_with_env(mut sandbox: SandboxManager) -> Result<()> {
    assert!(sandbox.epass(&["config"], "SANDBOX_NET", ""));
    assert!(sandbox.epass(&["config"], "SANDBOX_NET", "host"));
    assert!(sandbox.last_stdout.contains("net=host"));
    assert!(sandbox.epass(&["config"], "SANDBOX_NET", "none"));
    assert!(sandbox.last_stdout.contains("net=none"));
    assert!(sandbox.exfail(&["config"], "SANDBOX_NET", "foobar"));
    assert!(sandbox.last_stderr.contains("Invalid network type: foobar"));

    assert!(sandbox.epass(&["config"], "SANDBOX_LOG_LEVEL", "trace"));
    assert!(sandbox.last_stdout.contains("log_level=TRACE"));

    assert!(sandbox.exfail(&["config"], "SANDBOX_LOG_LEVEL", "foobar"));
    assert!(sandbox.last_stderr.contains("Invalid log level: foobar"));

    assert!(sandbox.epass(&["config"], "SANDBOX_NAME", ""));
    assert!(sandbox.epass(&["config"], "SANDBOX_NAME", "test-cli-with-env"));

    assert!(sandbox.epass(&["config"], "SANDBOX_STORAGE_DIR", ""));
    assert!(sandbox.epass(&["config"], "SANDBOX_STORAGE_DIR", "/tmp/sandbox"));
    assert!(sandbox.last_stdout.contains("storage_dir=/tmp/sandbox"));

    Ok(())
}

#[rstest]
fn test_cli_resolve_failures(mut sandbox: SandboxManager) -> Result<()> {
    assert!(sandbox.exfail(&["config"], "TEST_UNABLE_TO_OPEN_PROC_MOUNTS", ""));
    assert!(sandbox.exfail(&["config"], "TEST_UNABLE_TO_FIND_MOUNT_POINT", ""));
    assert!(sandbox.exfail(&["config"], "TEST_UNACCEPTABLE_MOUNT_TYPE", ""));
    assert!(sandbox.xfail(&["--storage-dir=/bad/path", "config"]));
    assert!(sandbox.last_stderr.contains("Insufficient access"));

    Ok(())
}
