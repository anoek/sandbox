use anyhow::Result;
use std::path::PathBuf;

pub fn find_mount_point(mut path: PathBuf) -> Result<PathBuf> {
    while !path.exists() {
        let parent = path
            .parent()
            .expect("Failed to get parent of path")
            .to_path_buf();
        path = parent.to_path_buf();
    }

    let metadata = nix::sys::stat::lstat(&path)?;
    let device = metadata.st_dev;

    while let Some(parent) = path.parent() {
        let parent_metadata = nix::sys::stat::lstat(parent)?;

        if parent_metadata.st_dev != device {
            break;
        }
        path = parent.to_path_buf();
    }
    Ok(path)
}
