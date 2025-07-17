use crate::sandbox::Sandbox;
use crate::util::{can_mkdir, mkdir};
use anyhow::{Context, Result, anyhow};
use log::debug;
use nix::{
    mount::MsFlags,
    unistd::{Gid, Uid},
};
use std::{ffi::CStr, path::Path};

const ACCEPTABLE_MOUNT_TYPES: [&str; 7] =
    ["xfs", "nfs4", "ext2", "ext3", "ext4", "zfs", "btrfs"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MountHash {
    pub hash: String,
    pub dir: String,
}

impl Sandbox {
    pub fn mount_overlays(&self) -> Result<Vec<MountHash>> {
        let system_mounts =
            unsafe { libc::setmntent(c"/proc/mounts".as_ptr(), c"r".as_ptr()) };

        #[cfg(feature = "coverage")]
        let system_mounts =
            if std::env::var_os("TEST_SYSTEM_MOUNTS_NULL").is_some() {
                std::ptr::null_mut()
            } else {
                system_mounts
            };

        if system_mounts.is_null() {
            return Err(anyhow!("Failed to open /proc/mounts".to_string(),));
        }

        let mut mounts: Vec<MountHash> = Vec::new();

        loop {
            let mnt = unsafe { libc::getmntent(system_mounts) };
            if mnt.is_null() {
                break;
            }

            let mnt_dir = String::from(unsafe {
                CStr::from_ptr((*mnt).mnt_dir).to_string_lossy()
            });
            let mnt_type = String::from(unsafe {
                CStr::from_ptr((*mnt).mnt_type).to_string_lossy()
            });

            /* Mount / regardless of type, in docker or another sandbox it is expected
             * to be an overlayfs. For all other file systems though, skip them if
             * they don't seem to be real file systems. */
            if mnt_dir != "/"
                && !ACCEPTABLE_MOUNT_TYPES.contains(&mnt_type.as_str())
            {
                continue;
            }

            if !Path::new(&mnt_dir).starts_with(&self.base) {
                mounts.push(MountHash {
                    hash: data_encoding::BASE32_NOPAD
                        .encode(mnt_dir.as_bytes()),
                    dir: mnt_dir,
                });
            }
        }

        unsafe { libc::endmntent(system_mounts) };

        mounts.sort_by(|a, b| a.dir.cmp(&b.dir));

        #[cfg(feature = "coverage")]
        if std::env::var_os("TEST_NO_MOUNTS_FOUND").is_some() {
            mounts.clear();
        };

        if mounts.is_empty() {
            return Err(anyhow!(
                "No suitable mounts found for sandbox, something is very wrong"
                    .to_string(),
            ));
        }

        for e in mounts.clone() {
            let hash = e.hash;
            let dir = e.dir;

            debug!("Mounting {} to {}", dir, hash);

            /* Create our directories if needed */
            can_mkdir(&self.base, self.uid, self.gid).context(format!(
                "Failed to check if we can create the base directory {:?} for the sandbox failed",
                self.base
            ))?;
            mkdir(&self.base, self.uid, self.gid)?;

            // Get the uid/gid of the source directory
            let source_stat = nix::sys::stat::lstat(dir.as_str())?;
            let source_uid = Uid::from_raw(source_stat.st_uid);
            let source_gid = Gid::from_raw(source_stat.st_gid);

            mkdir(&self.work_base.join(&hash), source_uid, source_gid)?;
            mkdir(&self.upper_base.join(&hash), source_uid, source_gid)?;
            mkdir(&self.overlay_base.join(&hash), source_uid, source_gid)?;

            /*
             * index=off
             *
             *   Unknown if we need this option or not. Turning it on has benefits to hard links.
             *
             * redirect_dir=on
             *
             *   Allows us to rename directories without having to copy the contents.
             *
             *   Note: This poses a problem for online changes to the directory names
             *   in the under fs, but docs say it won't result in a crash or deadlock.
             *
             *
             * metacopy=off
             *
             *   From https://docs.kernel.org/filesystems/overlayfs.html:
             *   Do not use metacopy=on with untrusted upper/lower directories. Otherwise it is
             *   possible that an attacker can create a handcrafted file with appropriate REDIRECT
             *   and METACOPY xattrs, and gain access to file on lower pointed by REDIRECT.
             *
             *   Note: They go on to say
             *
             *      This should not be possible on local system as setting “trusted.” xattrs will
             *      require CAP_SYS_ADMIN. But it should be possible for untrusted layers like from
             *      a pen drive."
             *
             *   I don't think this is a very realistic risk, never the less it's off for now. The
             *   only bad part about turning this off is moved files and files that just have
             *   attributes modified will be entirely copied to the overlayfs. This is potentially
             *   notably slower if dealing with very large files where only attributes are
             *   changing, but that use case doesn't seem worth the added complexity and edge case
             *   risk vector.
             */
            let overlayfs_options = format!(
                "lowerdir={},upperdir={},workdir={},index=off,redirect_dir=on,metacopy=off",
                dir,
                self.upper_base.join(&hash).display(),
                self.work_base.join(&hash).display()
            );

            /* Mount overlayfs */
            crate::util::mount(
                Some("overlay"),
                self.overlay_base
                    .join(&hash)
                    .to_str()
                    .expect("Failed to convert path to str"),
                Some("overlay"),
                MsFlags::empty(),
                Some(overlayfs_options),
            )?;
        }

        Ok(mounts)
    }
}
