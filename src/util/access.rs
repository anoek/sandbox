use anyhow::{Context, Result, anyhow};
use log::trace;
use nix::unistd::{Gid, Uid, eaccess, getegid, geteuid, setegid, seteuid};
use std::path::Path;

pub fn can_access(
    path: &Path,
    uid: Uid,
    gid: Gid,
    mode: nix::unistd::AccessFlags,
) -> Result<()> {
    let current_uid = geteuid();
    let current_gid = getegid();

    trace!(
        "current_uid: {}, current_gid: {}, uid: {}, gid: {}, mode: {:?}",
        current_uid, current_gid, uid, gid, mode
    );

    setegid(gid)?;
    seteuid(uid)?;

    let res = eaccess(path, mode);
    trace!(
        "can_access({}, {}, {}) = {:?}",
        path.display(),
        uid,
        gid,
        res
    );

    seteuid(current_uid)?;
    setegid(current_gid)?;

    match res {
        Ok(_) => Ok(()),
        Err(e) => Err(anyhow!("Failed to access {}: {}", path.display(), e)),
    }
}

/* Finds the first ancestor that exists and checks to see if we have write access to it. */
pub fn can_mkdir(path: &Path, uid: Uid, gid: Gid) -> Result<()> {
    let mut current_path = path.to_path_buf();
    loop {
        let existing_base = current_path.parent().expect("path has no parent");
        if existing_base.exists() {
            can_access(existing_base, uid, gid, nix::unistd::AccessFlags::W_OK)
                .context(format!(
                    "Insufficient access to {} to create directory {}",
                    existing_base.display(),
                    path.display()
                ))?;
            break;
        }
        current_path = existing_base.to_path_buf();
    }

    Ok(())
}
