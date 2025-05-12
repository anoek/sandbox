use crate::sandbox::Sandbox;
use crate::util::get_mounts;
use anyhow::Result;
use log::{debug, error};
use std::ffi::CString;

impl Sandbox {
    pub fn unmount(&self) -> Result<()> {
        let mut mounts = get_mounts(&self.base)?;
        mounts.sort_by(|a, b| b.cmp(a));

        #[cfg(feature = "coverage")]
        if std::env::var_os("TEST_NO_MOUNTS_FOUND").is_some() {
            mounts.clear();
        }

        if mounts.is_empty() {
            debug!("No mounts found for sandbox '{}'", self.name);
        }

        for mount in mounts {
            let dir = CString::new(mount.as_bytes())?;
            debug!("Unmounting {}", mount);
            let result =
                unsafe { libc::umount2(dir.as_ptr(), libc::MNT_DETACH) };

            #[cfg(feature = "coverage")]
            let result = if std::env::var_os("TEST_UNMOUNT_RACE").is_some() {
                // Double unmount to test race condition
                unsafe { libc::umount2(dir.as_ptr(), libc::MNT_DETACH) }
            } else {
                result
            };

            if result != 0 {
                error!("Failed to unmount {}", mount);
            }
        }

        Ok(())
    }
}
