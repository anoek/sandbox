use anyhow::{Result, anyhow};
use std::ffi::CStr;
use std::path::Path;

pub fn get_mounts(base: &Path) -> Result<Vec<String>> {
    let mut mounts = Vec::new();

    let system_mounts =
        unsafe { libc::setmntent(c"/proc/mounts".as_ptr(), c"r".as_ptr()) };

    #[cfg(feature = "coverage")]
    let system_mounts = if std::env::var_os("TEST_SYSTEM_MOUNTS_NULL").is_some()
    {
        std::ptr::null_mut()
    } else {
        system_mounts
    };

    if system_mounts.is_null() {
        return Err(anyhow!("Failed to open /proc/mounts".to_string(),));
    }

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

    Ok(mounts)
}
