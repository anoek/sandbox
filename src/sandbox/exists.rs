use std::path::PathBuf;

use crate::sandbox::Sandbox;

impl Sandbox {
    pub fn exists(&self) -> bool {
        let pid_file = PathBuf::from(format!("{}.pid", self.base.display()));
        let lock_file = PathBuf::from(format!("{}.lock", self.base.display()));

        self.base.exists() || pid_file.exists() || lock_file.exists()
    }
}
