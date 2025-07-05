use std::path::PathBuf;

use crate::{actions::rmdir_recursive, sandbox::Sandbox};
use anyhow::{Context, Result};
use log::{debug, trace};

impl Sandbox {
    pub fn delete(&self) -> Result<()> {
        self.stop().context("failed to stop sandbox")?;
        self.unmount().context("failed to unmount sandbox")?;

        trace!("Removing sandbox directory: {}", self.base.display());
        if self.base.exists() {
            rmdir_recursive(&self.base)?;
        } else {
            debug!("Sandbox directory does not exist: {}", self.base.display());
        }

        let lock_file = PathBuf::from(format!("{}.lock", self.base.display()));
        trace!("Cleaning up lock file: {}", lock_file.display());
        std::fs::remove_file(&lock_file).context(format!(
            "failed to remove lock file {}",
            lock_file.display()
        ))?;

        #[cfg(feature = "coverage")]
        if std::env::var_os("TEST_DELETE_FAILURE").is_some() {
            return Err(anyhow::anyhow!("Test delete failure"));
        };

        Ok(())
    }
}
