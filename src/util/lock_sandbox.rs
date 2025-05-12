use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};

use log::trace;
use nix::fcntl::Flock;

use anyhow::{Result, anyhow};

pub struct Lock {
    path: PathBuf,
    #[allow(dead_code)]
    lock: Flock<File>,
}

impl Lock {
    /** Acquire a lock on the sandbox storage directory. */
    pub fn sandbox(
        sandboxes_storage_dir: &Path,
        sandbox_name: &str,
    ) -> Result<Box<Lock>> {
        let lock_file =
            sandboxes_storage_dir.join(format!("{}.lock", sandbox_name));
        trace!("Acquiring lock {}", lock_file.display());
        let file = match OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(&lock_file)
        {
            Ok(file) => file,
            Err(e) => {
                return Err(anyhow!(
                    "Failed to open lock file for sandbox {}: {}",
                    sandbox_name,
                    e
                ));
            }
        };

        let lock =
            nix::fcntl::Flock::lock(file, nix::fcntl::FlockArg::LockExclusive)
                .map_err(|(_, e)| anyhow!("Failed to acquire lock: {}", e))?;

        trace!("Acquired lock {}", lock_file.display());
        Ok(Box::new(Lock {
            path: lock_file,
            lock,
        }))
    }
}

impl Drop for Lock {
    fn drop(&mut self) {
        trace!("Unlocking lock {}", self.path.display());
    }
}
