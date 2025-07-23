use std::path::{Path, PathBuf};
use crate::util::resolve_uid_gid_home;
use anyhow::Result;

/// Expands a path that starts with ~ to use the user's home directory
pub fn expand_tilde_path(path: &Path) -> Result<PathBuf> {
    if let Some(path_str) = path.to_str() {
        if path_str.starts_with('~') {
            let uid_gid_home = resolve_uid_gid_home()?;
            if path_str == "~" {
                return Ok(uid_gid_home.home);
            } else if let Some(rest) = path_str.strip_prefix("~/") {
                return Ok(uid_gid_home.home.join(rest));
            }
        }
    }
    Ok(path.to_path_buf())
}