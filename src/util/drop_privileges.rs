use anyhow::Result;
use nix::unistd::{Gid, Uid, setgid, setuid};

pub fn drop_privileges(uid: Uid, gid: Gid) -> Result<()> {
    setgid(gid)?;
    setuid(uid)?;
    Ok(())
}
