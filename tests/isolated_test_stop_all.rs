mod fixtures;

use anyhow::Result;
use fixtures::*;
use rstest::*;
use std::{io::Write, process::Command, thread::sleep, time::Duration};

/* This test needs to be the only test in this file since it
 * kills all running sandboxes, which would interfere with other
 * tests that were running in parallel. */

#[rstest]
fn test_stop_all() -> Result<()> {
    let mut sandbox1 = sandbox();
    let mut sandbox2 = sandbox();
    let mut c1 = sandbox1.run_in_background(&["sleep", "9897135"])?;
    let mut c2 = sandbox2.run_in_background(&["sleep", "9897135"])?;

    // ensure they start up
    let mut ok = false;
    for _ in 0..1000 {
        sleep(Duration::from_millis(10));
        let ps = Command::new("ps").arg("aux").output()?;
        let ps_output = String::from_utf8(ps.stdout)?;
        let lines: Vec<&str> = ps_output
            .lines()
            .filter(|line| {
                // excluding --name excludes the sandbox binary call lines
                line.contains("sleep 9897135") && !line.contains("--name")
            })
            .collect();
        if lines.len() == 2 {
            ok = true;
            break;
        }
    }

    if !ok {
        return Err(anyhow::anyhow!(
            "Sandboxes did not start within 10 seconds"
        ));
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
    println!("lines.len(): {:?}", lines.len());
    println!("lines: {:?}", lines);

    // flush stdout
    std::io::stdout().flush()?;

    assert!(lines.len() == 2);

    assert!(sandbox1.pass(&["stop", "--all"]));
    c1.wait()?;

    let ps = Command::new("ps").arg("aux").output()?;
    let ps_output = String::from_utf8(ps.stdout)?;
    let lines: Vec<&str> = ps_output
        .lines()
        .filter(|line| {
            line.contains("sleep 9897135") && !line.contains("--name")
        })
        .collect();
    assert!(lines.is_empty());

    c2.wait()?; // if this doesn't hang, then the sandbox was killed!
    Ok(())
}
