mod fixtures;

use anyhow::Result;
use fixtures::*;
use rstest::*;
use std::process::Command;
use std::thread::sleep;
use std::time::Duration;

#[rstest]
fn test_stop_stop_children(mut sandbox: SandboxManager) -> Result<()> {
    let mut c1 = sandbox.run_in_background(&["sleep", "9897135"])?;
    let mut c2 = sandbox.run_in_background(&["sleep", "9897135"])?;

    // ensure they start up
    for _ in 0..1000 {
        sleep(Duration::from_millis(10));
        let ps = Command::new("ps").arg("aux").output()?;
        let ps_output = String::from_utf8(ps.stdout)?;
        let lines: Vec<&str> = ps_output
            .lines()
            .filter(|line| {
                line.contains("sleep 9897135") && !line.contains("--name")
            })
            .collect();
        if lines.len() == 2 {
            break;
        }
    }

    // ensure they're actually there
    let ps = Command::new("ps").arg("aux").output()?;
    let ps_output = String::from_utf8(ps.stdout)?;
    let lines: Vec<&str> = ps_output
        .lines()
        .filter(|line| {
            line.contains("sleep 9897135") && !line.contains("--name")
        })
        .collect();
    assert!(lines.len() == 2);

    // kill them
    assert!(sandbox.pass(&["stop"]));

    // wait for that to happen
    c1.wait()?;
    c2.wait()?;

    // ensure ps agrees they're dead
    let ps = Command::new("ps").arg("aux").output()?;
    let ps_output = String::from_utf8(ps.stdout)?;
    let lines: Vec<&str> = ps_output
        .lines()
        .filter(|line| {
            line.contains("sleep 9897135") && !line.contains("--name")
        })
        .collect();
    assert!(lines.is_empty());

    Ok(())
}
