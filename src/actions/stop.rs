use std::path::Path;

use fast_glob::glob_match;
use log::{trace, warn};
use nix::unistd::{Gid, Uid};

use crate::sandbox::Sandbox;
use anyhow::Result;

pub fn stop(
    sandboxes_storage_dir: &Path,
    sandbox_name: &str,
    uid: Uid,
    gid: Gid,
) -> Result<()> {
    trace!("Stopping sandbox {}", sandbox_name);
    let (sandbox, _lock) =
        Sandbox::get(sandboxes_storage_dir, sandbox_name, uid, gid, None)?;

    let sandbox = match sandbox {
        Some(sandbox) => sandbox,
        None => Sandbox::from_location(
            sandboxes_storage_dir,
            sandbox_name,
            uid,
            gid,
        ),
    };

    sandbox.stop()?;
    sandbox.unmount()?;

    Ok(())
}

pub fn stop_all(
    sandboxes_storage_dir: &Path,
    uid: Uid,
    gid: Gid,
    patterns: &[String],
) -> Result<()> {
    trace!(
        "Stopping all sandboxes in {}",
        sandboxes_storage_dir.display()
    );

    let sandboxes = sandboxes_storage_dir.read_dir()?;
    for sandbox in sandboxes {
        let entry = sandbox?;
        let path = entry.path();
        let filename = path
            .components()
            .next_back()
            .expect("Failed to get filename")
            .as_os_str();
        if filename.to_string_lossy().ends_with(".lock") {
            let sandbox_name =
                filename.to_string_lossy()[..filename.len() - 5].to_string();

            let lock_file = format!(
                "{}/{}.lock",
                sandboxes_storage_dir.display(),
                sandbox_name
            );
            let pid_file = format!(
                "{}/{}.pid",
                sandboxes_storage_dir.display(),
                sandbox_name
            );
            let storage_dir =
                format!("{}/{}", sandboxes_storage_dir.display(), sandbox_name);

            if !Path::new(&lock_file).exists()
                || !Path::new(&pid_file).exists()
                || !Path::new(&storage_dir).exists()
            {
                continue;
            }

            if patterns.is_empty()
                || patterns.iter().any(|pattern| {
                    let mut pattern = pattern.clone();
                    pattern = format!("*{pattern}*");
                    glob_match(&pattern, &sandbox_name)
                })
            {
                match stop(sandboxes_storage_dir, &sandbox_name, uid, gid) {
                    Ok(_) => {}
                    Err(e) => {
                        warn!("Failed to kill sandbox {}: {}", sandbox_name, e);
                    }
                }
            }
        }
    }

    Ok(())
}
