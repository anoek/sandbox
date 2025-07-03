use crate::{
    config::Config,
    outln,
    sandbox::{Sandbox, changes::changes::by_staged_descending},
    util::sync_and_drop_caches,
};
use anyhow::{Context, Result};
use log::{debug, trace};

pub fn reject(
    config: &Config,
    sandbox: &Sandbox,
    patterns: &[String],
) -> Result<()> {
    trace!("Rejecting changes from sandbox {}", sandbox.name);

    let cwd = std::env::current_dir()?;
    let changes = sandbox.changes(config)?;

    let mut changes = changes.matching(&cwd, patterns);
    changes.sort_by(by_staged_descending);

    let mut last_staged_path = None;

    for change in changes.iter_mut() {
        if let Some(staged) = &change.staged {
            if let Some(last_staged_path) = &last_staged_path {
                if *last_staged_path == staged.path {
                    /* We'll see duplicate staged paths when doing remove/adds */
                    continue;
                }
            }

            last_staged_path = Some(staged.path.clone());

            debug!("Rejecting {}", staged.path.display());
            if staged.is_dir() {
                std::fs::remove_dir(&staged.path).context(format!(
                    "Failed to remove directory {}",
                    staged.path.display()
                ))?;
            } else {
                std::fs::remove_file(&staged.path).context(format!(
                    "Failed to remove file {}",
                    staged.path.display()
                ))?;
            }
        }
    }

    sync_and_drop_caches()?;

    outln!("Rejected {} changes", changes.len());

    Ok(())
}
