mod fixtures;

use anyhow::Result;
use fixtures::*;
use nix::unistd::chdir;
use rstest::*;
use std::{path::Path, process::Command};

/* This test needs to be the only test in this file since we chdir
 * and tests are run in parallel per file.
 *
 *
 * This test assumes where we are running our tests from is a different mount point from that of
 * root. If not, it will still pass, but it's not testing much at that point. This is the
 * desired behavior as test VMs should have different mount topologies and they should all
 * work.
 */

#[rstest]
fn test_accept_rename_across_mount(mut sandbox: SandboxManager) -> Result<()> {
    let dest_host_dir = "/delete-me-this-is-a-sandbox-coverage-test-dir-for-rename-across-mount";
    let dir_to_move = sandbox.test_filename("dir_to_move");
    Command::new("sudo")
        .arg("mkdir")
        .arg(dest_host_dir)
        .output()?;
    Command::new("sudo")
        .arg("chmod")
        .arg("777")
        .arg(dest_host_dir)
        .output()?;

    Command::new("sudo")
        .arg("mkdir")
        .arg(&dir_to_move)
        .output()?;
    Command::new("sudo")
        .arg("chmod")
        .arg("777")
        .arg(&dir_to_move)
        .output()?;

    let path = Path::new(&dir_to_move);
    let dest_path = Path::new(&dest_host_dir);
    sandbox.run(&["mv", &dir_to_move, dest_host_dir])?;
    assert!(path.exists());
    assert!(dest_path.exists());

    // The first accept will match the file, but since it's not a
    // relative change it will not accept it
    let cwd = std::env::current_dir()?;
    chdir("/")?;
    assert!(sandbox.pass(&["accept", &dir_to_move]));
    chdir(&cwd)?; // restore for cleanup

    Command::new("sudo")
        .arg("rmdir")
        .arg(format!("{}/{}", dest_host_dir, dir_to_move))
        .output()?;
    Command::new("sudo")
        .arg("rmdir")
        .arg(dest_host_dir)
        .output()?;
    Ok(())
}
