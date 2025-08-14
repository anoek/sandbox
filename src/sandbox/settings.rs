use crate::config::{BindMount, Config};
use crate::sandbox::mount_overlays::MountHash;
use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SandboxSettings {
    pub version: String,
    pub mounts: Vec<MountHash>,
    pub network: crate::config::Network,
    pub bind_mounts: Vec<BindMount>,
}

impl SandboxSettings {
    pub fn from_config(config: &Config, mounts: &[MountHash]) -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION").to_string(),
            mounts: mounts.to_vec(),
            network: config.net.clone(),
            bind_mounts: config.bind_mounts.clone(),
        }
    }

    /// Filter out implicit system bind mounts (like "data") for comparison purposes
    /// Returns a sorted vector to ensure order-independent comparison
    fn user_bind_mounts(&self) -> Vec<&BindMount> {
        let mut binds: Vec<&BindMount> = self
            .bind_mounts
            .iter()
            .filter(|bind| bind.argument != "data")
            .collect();
        // Sort by argument string to ensure consistent ordering
        binds.sort_by(|a, b| a.argument.cmp(&b.argument));
        binds
    }

    pub fn save_to_file(&self, path: &Path) -> Result<()> {
        let settings_json = serde_json::to_string_pretty(self)
            .context("Failed to serialize sandbox settings to JSON")?;
        std::fs::write(path, settings_json).context(format!(
            "Failed to write settings.json to {}",
            path.display()
        ))?;
        Ok(())
    }

    pub fn load_from_file(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Err(anyhow!(
                "Settings file does not exist: {}",
                path.display()
            ));
        }
        let settings_json = std::fs::read_to_string(path).context(format!(
            "Failed to read settings.json from {}",
            path.display()
        ))?;
        let settings: SandboxSettings = serde_json::from_str(&settings_json)
            .context("Failed to parse settings.json")?;
        Ok(settings)
    }

    pub fn validate_against_config(
        &self,
        config: &Config,
        mounts: &[MountHash],
    ) -> Result<()> {
        let current_settings = Self::from_config(config, mounts);

        if self.network != current_settings.network {
            return Err(anyhow!(
                "Network configuration has changed. Existing: {:?}, Current: {:?}. Please stop existing sandbox to change network settings.",
                self.network,
                current_settings.network
            ));
        }

        if self.mounts != current_settings.mounts {
            let mut error_msg =
                String::from("Mount configuration has changed:\n");

            // Find mounts that were removed
            let removed_mounts: Vec<_> = self
                .mounts
                .iter()
                .filter(|existing| !current_settings.mounts.contains(existing))
                .collect();
            if !removed_mounts.is_empty() {
                error_msg.push_str("  Removed mounts:\n");
                for mount in removed_mounts {
                    error_msg.push_str(&format!("    - {}\n", mount.dir));
                }
            }

            // Find mounts that were added
            let added_mounts: Vec<_> = current_settings
                .mounts
                .iter()
                .filter(|current| !self.mounts.contains(current))
                .collect();
            if !added_mounts.is_empty() {
                error_msg.push_str("  Added mounts:\n");
                for mount in added_mounts {
                    error_msg.push_str(&format!("    - {}\n", mount.dir));
                }
            }

            error_msg.push_str(
                "Please stop the existing sandbox to change mount settings.",
            );
            return Err(anyhow!(error_msg));
        }

        // Compare only user-specified bind mounts (excluding implicit system mounts like "data")
        let existing_user_binds = self.user_bind_mounts();
        let current_user_binds = current_settings.user_bind_mounts();

        if existing_user_binds != current_user_binds {
            let mut error_msg =
                String::from("Bind mount configuration has changed:\n");

            // Find bind mounts that were removed
            let removed_binds: Vec<_> = existing_user_binds
                .iter()
                .filter(|existing| !current_user_binds.contains(existing))
                .collect();
            if !removed_binds.is_empty() {
                error_msg.push_str("  Removed bind mounts:\n");
                for bind in &removed_binds {
                    error_msg.push_str(&format!("    - {}\n", bind.argument));
                }
            }

            // Find bind mounts that were added
            let added_binds: Vec<_> = current_user_binds
                .iter()
                .filter(|current| !existing_user_binds.contains(current))
                .collect();
            if !added_binds.is_empty() {
                error_msg.push_str("  Added bind mounts:\n");
                for bind in &added_binds {
                    error_msg.push_str(&format!("    - {}\n", bind.argument));
                }
            }

            error_msg.push_str("Please stop the existing sandbox to change bind mount settings.");
            return Err(anyhow!(error_msg));
        }

        Ok(())
    }
}
