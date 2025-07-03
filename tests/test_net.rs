mod fixtures;

use anyhow::Result;
use fixtures::*;
use rstest::*;

#[rstest]
fn test_no_network(mut sandbox: SandboxManager) -> Result<()> {
    assert!(sandbox.xfail(&[
        "--net=none",
        "ping",
        "-c",
        "1",
        "-W",
        "1",
        "127.0.0.1"
    ]));
    assert!(
        sandbox
            .last_stderr
            .contains("Creating new network namespace")
    );
    Ok(())
}

#[rstest]
fn test_network(mut sandbox: SandboxManager) -> Result<()> {
    let status = sandbox.run(&[
        "--net=host",
        "ping",
        "-c",
        "1",
        "-W",
        "1",
        "127.0.0.1",
    ])?;
    assert!(sandbox.last_stderr.contains("Using host network"));
    assert!(status.status.success());
    Ok(())
}
