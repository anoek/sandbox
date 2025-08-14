use nix::unistd::Gid;
use nix::unistd::Pid;
use nix::unistd::Uid;
use std::path::PathBuf;

pub struct Sandbox {
    pub name: String,
    pub base: PathBuf,
    pub work_base: PathBuf,
    pub upper_base: PathBuf,
    pub overlay_base: PathBuf,
    pub data_storage_dir: PathBuf, // storage dir for sandbox data and nested sandboxes
    //pub root_suffix: String,
    //pub root_work: PathBuf,
    //pub root_upper: PathBuf,
    pub root_overlay: PathBuf,
    pub pid: Pid,
    pub uid: Uid,
    pub gid: Gid,
}
