use anyhow::{Result, anyhow};
use nix::{
    sys::stat::SFlag,
    unistd::{Gid, Uid, chown},
};
use std::path::PathBuf;

/* Makes the directory if it doesn't exist and sets the owner and group.
 * Will throw an error if the directory already exists and the owner and group
 * do not match the given uid and gid, or if the path is not a directory.
*/
pub fn mkdir(path: &PathBuf, uid: Uid, gid: Gid) -> Result<()> {
    if path.exists() {
        let metadata = nix::sys::stat::lstat(path)?;
        if metadata.st_mode & SFlag::S_IFMT.bits() != SFlag::S_IFDIR.bits() {
            return Err(anyhow!(
                "Directory {} already exists but is not a directory",
                path.display()
            ));
        }

        if metadata.st_uid != uid.as_raw() || metadata.st_gid != gid.as_raw() {
            return Err(anyhow!(
                "Directory {} already exists with different owner or group",
                path.display()
            ));
        }
    } else {
        // Create the directory
        match std::fs::create_dir_all(path) {
            Ok(_) => (),
            Err(e) => {
                return Err(anyhow!(
                    "Failed to create directory {}: {}",
                    path.display(),
                    e
                ));
            }
        }
    }

    match chown(path, Some(uid), Some(gid)) {
        Ok(_) => (),
        Err(e) => {
            return Err(anyhow!(
                "Failed to chown directory {}: {}",
                path.display(),
                e
            ));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use nix::unistd::{getgid, getuid};
    use std::path::PathBuf;

    fn cleanup(path: &PathBuf) -> Result<()> {
        if path.exists() {
            if path.is_dir() {
                std::fs::remove_dir(path)?;
            } else {
                std::fs::remove_file(path)?;
            }
        }
        Ok(())
    }

    #[test]
    fn test_mkdir_failure_paths() -> Result<()> {
        let uid = getuid();
        let gid = getgid();
        let path = PathBuf::from(format!(
            "/tmp/sandbox-coverage-tests-mkdir-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs()
        ));
        cleanup(&path)?;

        std::fs::write(&path, "test")?;
        assert!(mkdir(&path, uid, gid).is_err());
        cleanup(&path)?;

        assert!(mkdir(&path, uid, gid).is_ok());
        // fails on different owner check, instead of the next one which will fail
        // on chown check
        assert!(mkdir(&path, Uid::from_raw(123456), gid).is_err());
        cleanup(&path)?;

        // can't chown since we don't run the actual test binary as root
        assert!(mkdir(&path, Uid::from_raw(123456), gid).is_err());
        cleanup(&path)?;

        // can't mkdir in root since we don't run the actual test binary as root
        assert!(
            mkdir(
                &PathBuf::from(format!(
                    "/sandbox-coverage-tests-mkdir-{}",
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs()
                )),
                uid,
                gid
            )
            .is_err()
        );
        cleanup(&path)?;

        Ok(())
    }
}
