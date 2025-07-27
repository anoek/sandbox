use crate::util::resolve_uid_gid_home;
use anyhow::Result;
use std::path::{Path, PathBuf};

/// Expands a path that starts with ~ to use the user's home directory
pub fn expand_tilde_path(path: &Path) -> Result<PathBuf> {
    let path_str = path.to_str().expect("path is not a valid string");
    if path_str.starts_with('~') {
        let uid_gid_home = resolve_uid_gid_home()?;
        if path_str == "~" {
            return Ok(uid_gid_home.home);
        } else if let Some(rest) = path_str.strip_prefix("~/") {
            return Ok(uid_gid_home.home.join(rest));
        }
    }
    Ok(path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expand_tilde_path() {
        let uid_gid_home = resolve_uid_gid_home().unwrap();
        assert_eq!(
            expand_tilde_path(Path::new("~")).unwrap(),
            uid_gid_home.home
        );
        assert_eq!(
            expand_tilde_path(Path::new("~/test")).unwrap(),
            uid_gid_home.home.join("test")
        );
        assert_eq!(
            expand_tilde_path(Path::new("~/test/test2")).unwrap(),
            uid_gid_home.home.join("test").join("test2")
        );
    }

    #[test]
    fn test_expand_tilde_path_error() {
        assert_eq!(
            expand_tilde_path(Path::new("/test")).unwrap(),
            PathBuf::from("/test")
        );
        assert_eq!(
            expand_tilde_path(Path::new("~test")).unwrap(),
            PathBuf::from("~test")
        );
    }
}
