use nix::unistd::Gid;
use nix::unistd::Uid;
use std::path::PathBuf;

pub struct UidGidHome {
    pub uid: Uid,
    pub gid: Gid,
    pub home: PathBuf,
}
