mod fixtures;

use anyhow::Result;
use fixtures::*;
use rstest::*;
use std::{process::Command, thread::sleep, time::Duration};

/* This test needs to be the only test in this file since it
 * kills all running sandboxes, which would interfere with other
 * tests that were running in parallel. */

#[rstest]
fn test_stop_pattern() -> Result<()> {
    let mut sandbox1 = sandbox();
    let mut sandbox2 = sandbox();
    let mut c1 = sandbox1.run_in_background(&["sleep", "9897135"])?;
    let mut c2 = sandbox2.run_in_background(&["sleep", "9897135"])?;

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

    assert!(sandbox1.pass(&["stop", sandbox2.name.as_str()]));
    c2.wait()?;

    let ps = Command::new("ps").arg("aux").output()?;
    let ps_output = String::from_utf8(ps.stdout)?;
    let lines: Vec<&str> = ps_output
        .lines()
        .filter(|line| {
            line.contains("sleep 9897135") && !line.contains("--name")
        })
        .collect();
    assert!(lines.len() == 1);

    sandbox1.run(&["stop"])?;

    c1.wait()?; // if this doesn't hang, then the sandbox was killed!
    Ok(())
}
