mod fixtures;

use anyhow::Result;
use fixtures::*;
use rstest::*;
use std::ffi::CStr;
use std::path::Path;

#[rstest]
fn test_fail_to_read_mounts(mut sandbox: SandboxManager) -> Result<()> {
    assert!(sandbox.exfail(&["env"], "TEST_SYSTEM_MOUNTS_NULL", "1"));
    assert!(sandbox.pass(&["true"]));
    assert!(sandbox.exfail(&["stop"], "TEST_SYSTEM_MOUNTS_NULL", "1"));
    assert!(sandbox.pass(&["stop"]));
    Ok(())
}

#[rstest]
fn test_no_mounts_found(mut sandbox: SandboxManager) -> Result<()> {
    assert!(sandbox.exfail(&["env"], "TEST_NO_MOUNTS_FOUND", "1"));
    assert!(sandbox.pass(&["true"]));
    assert!(sandbox.epass(&["stop"], "TEST_NO_MOUNTS_FOUND", "1"));
    assert!(sandbox.last_stderr.contains("No mounts found for sandbox"));
    Ok(())
}

#[rstest]
fn test_unmount_race(mut sandbox: SandboxManager) -> Result<()> {
    assert!(sandbox.pass(&["true"]));
    assert!(sandbox.epass(&["stop"], "TEST_UNMOUNT_RACE", "1"));
    assert!(sandbox.last_stderr.contains("Failed to unmount"));
    Ok(())
}

#[rstest]
fn test_kill_not_running(mut sandbox: SandboxManager) -> Result<()> {
    assert!(sandbox.pass(&["stop"]));
    assert!(sandbox.last_stderr.contains("doesn't appear to be running"));

    assert!(sandbox.pass(&["true"]));
    assert!(sandbox.epass(&["stop"], "TEST_STOP_RACE", "1"));
    assert!(sandbox.last_stderr.contains("already gone"));

    assert!(sandbox.pass(&["true"]));
    assert!(sandbox.epass(&["stop"], "TEST_STOP_RACE2", "1"));
    assert!(sandbox.last_stderr.contains("Failed to kill process"));
    Ok(())
}

#[rstest]
fn test_kill_unmounts(mut sandbox: SandboxManager) -> Result<()> {
    assert!(sandbox.pass(&["true"]));

    let mounts = get_mounts(&sandbox.dir()?);
    assert!(!mounts.is_empty());

    assert!(sandbox.pass(&["stop"]));
    let mounts = get_mounts(&sandbox.dir()?);
    assert!(mounts.is_empty());

    Ok(())
}

/* Utility function for getting mounts */
pub fn get_mounts(base: &Path) -> Vec<String> {
    let mut mounts = Vec::new();

    let system_mounts =
        unsafe { libc::setmntent(c"/proc/mounts".as_ptr(), c"r".as_ptr()) };

    assert!(!system_mounts.is_null());

    loop {
        let mnt = unsafe { libc::getmntent(system_mounts) };
        if mnt.is_null() {
            break;
        }

        let mnt_dir = String::from(unsafe {
            CStr::from_ptr((*mnt).mnt_dir).to_string_lossy()
        });

        if Path::new(&mnt_dir).starts_with(base) {
            mounts.push(mnt_dir);
        }
    }

    unsafe { libc::endmntent(system_mounts) };

    mounts.sort_by(|a, b| b.cmp(a));

    mounts
}
