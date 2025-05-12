use anyhow::{Result, anyhow};
use nix::unistd::{Gid, Uid, getresgid, getresuid};
use std::path::Path;

use crate::types::UidGidHome;

/**
 * We may be being run with setuid bits, or via sudo. In either case we
 * don't want to stay as root for any longer than necessary, this function
 * figures out who we really are and returns that information so we can
 * drop to the appropriate uid/gid and run as that user at the first
 * possible opportunity.
 */
pub fn resolve_uid_gid_home() -> Result<UidGidHome> {
    let sudo_uid = std::env::var("SUDO_UID");
    let sudo_gid = std::env::var("SUDO_GID");
    let home = std::env::var("SUDO_HOME")
        .or(std::env::var("HOME"))
        .unwrap_or("/tmp".to_string());

    let resuid = getresuid()?;
    let resgid = getresgid()?;

    let uid = match sudo_uid {
        Ok(uid) => match uid.parse::<u32>() {
            Ok(uid) => Uid::from_raw(uid),
            Err(_) => {
                return Err(anyhow!("Failed to parse SUDO_UID: {:?}", uid));
            }
        },
        Err(_) => resuid.real,
    };

    let gid = match sudo_gid {
        Ok(gid) => match gid.parse::<u32>() {
            Ok(gid) => Gid::from_raw(gid),
            Err(_) => {
                return Err(anyhow!("Failed to parse SUDO_GID: {:?}", gid));
            }
        },
        Err(_) => resgid.real,
    };

    let home_path = Path::new(&home);
    if !home_path.is_absolute() {
        return Err(anyhow!("Home directory is not absolute: {:?}", home));
    }

    if !home_path.exists() {
        return Err(anyhow!("Home directory does not exist: {:?}", home));
    }

    Ok(UidGidHome {
        uid,
        gid,
        home: home_path.to_path_buf(),
    })
}
