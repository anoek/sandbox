use super::PartialConfig;
use crate::config::Network;
use crate::util::{can_access, can_mkdir, find_mount_point, mkdir};
use crate::{config::Config, util::resolve_uid_gid_home};
use anyhow::{Context, Result};
use log::trace;
use nix::unistd::{Gid, Uid};
use std::collections::HashMap;
use std::{env, str::FromStr};
use std::{ffi::CStr, path::PathBuf};

use super::cli::Args;

pub fn resolve_config(cli: Args) -> Result<Config> {
    let uid_gid_home = resolve_uid_gid_home()?;
    let (mut partial_config, mut sources) = load_partial(cli.no_config)?;

    // Override with environment variables if set
    if let Ok(log_level) = env::var("SANDBOX_LOG_LEVEL") {
        if let Ok(log_level) = log::LevelFilter::from_str(&log_level) {
            partial_config.log_level = Some(log_level);
            sources.insert("log_level".into(), "environment".into());
        } else {
            return Err(anyhow::anyhow!("Invalid log level: {}", log_level));
        }
    }
    if let Ok(name) = env::var("SANDBOX_NAME") {
        if !name.is_empty() {
            partial_config.name = Some(name);
            sources.insert("name".into(), "environment".into());
        }
    }
    if let Ok(storage_dir) = env::var("SANDBOX_STORAGE_DIR") {
        if !storage_dir.is_empty() {
            partial_config.storage_dir = Some(storage_dir);
            sources.insert("storage_dir".into(), "environment".into());
        }
    }

    if let Ok(bind_fuse) = env::var("SANDBOX_BIND_FUSE") {
        if !bind_fuse.is_empty() {
            if let Ok(bind_fuse) = bool::from_str(&bind_fuse) {
                partial_config.bind_fuse = Some(bind_fuse);
                sources.insert("bind_fuse".into(), "environment".into());
            } else {
                return Err(anyhow::anyhow!(
                    "Invalid bind_fuse: {}",
                    bind_fuse
                ));
            }
        }
    }

    if let Ok(net) = env::var("SANDBOX_NET") {
        if !net.is_empty() {
            if let Ok(net) = Network::from_str(&net) {
                partial_config.net = Some(net);
                sources.insert("net".into(), "environment".into());
            } else {
                return Err(anyhow::anyhow!("Invalid network type: {}", net));
            }
        }
    }

    if let Ok(ignored_env) = env::var("SANDBOX_IGNORED") {
        if !ignored_env.is_empty() {
            if let Ok(ignored_val) = bool::from_str(&ignored_env) {
                partial_config.ignored = Some(ignored_val);
                sources.insert("ignored".into(), "environment".into());
            } else {
                return Err(anyhow::anyhow!(
                    "Invalid value for SANDBOX_IGNORED: {}",
                    ignored_env
                ));
            }
        }
    }

    // Override with CLI args if provided (highest precedence)
    if let Some(log_level) = cli.log_level {
        partial_config.log_level = Some(log_level);
        sources.insert("log_level".into(), "cli".into());
    }
    if let Some(name) = cli.name {
        partial_config.name = Some(name);
        sources.insert("name".into(), "cli".into());
    }
    if let Some(storage_dir) = cli.storage_dir {
        partial_config.storage_dir = Some(storage_dir);
        sources.insert("storage_dir".into(), "cli".into());
    }

    if let Some(net) = cli.net {
        partial_config.net = Some(net);
        sources.insert("net".into(), "cli".into());
    }

    if let Some(bind_fuse) = cli.bind_fuse {
        partial_config.bind_fuse = Some(bind_fuse);
        sources.insert("bind_fuse".into(), "cli".into());
    }

    if cli.ignored {
        partial_config.ignored = Some(true);
        sources.insert("ignored".into(), "cli".into());
    }

    // If nothing else, fill in with some default values
    let name = partial_config.name.unwrap_or("sandbox".to_string());
    if !sources.contains_key("name") {
        sources.insert("name".into(), "default".into());
    }

    let net = partial_config.net.unwrap_or(Network::None);
    if !sources.contains_key("net") {
        sources.insert("net".into(), "default".into());
    }

    let bind_fuse = partial_config.bind_fuse.unwrap_or(true);
    if !sources.contains_key("bind_fuse") {
        sources.insert("bind_fuse".into(), "default".into());
    }

    let ignored = partial_config.ignored.unwrap_or(false);
    if !sources.contains_key("ignored") {
        sources.insert("ignored".into(), "default".into());
    }

    let storage_dir = resolve_sandbox_storage_dir(
        partial_config.storage_dir,
        uid_gid_home.uid,
        uid_gid_home.gid,
    )?;
    if !sources.contains_key("storage_dir") {
        sources.insert("storage_dir".into(), "default".into());
    }

    let sandbox_dir = storage_dir.join(name.clone());
    if !sources.contains_key("sandbox_dir") {
        sources.insert("sandbox_dir".into(), "derived from storage_dir".into());
    }

    let cwd = std::env::current_dir()?;
    let mp = find_mount_point(cwd.clone())?;
    let hash =
        data_encoding::BASE32_NOPAD.encode(mp.to_string_lossy().as_bytes());
    let upper_cwd = storage_dir
        .join(name.clone())
        .join("upper")
        .join(&hash)
        .join(cwd.strip_prefix(&mp)?);
    sources.insert("upper_cwd".into(), "derived from storage_dir".into());

    let overlay_cwd = storage_dir
        .join(name.clone())
        .join("overlay")
        .join(&hash)
        .join(cwd.strip_prefix(&mp)?);
    sources.insert("overlay_cwd".into(), "derived from storage_dir".into());

    let config = Config {
        name,
        net,
        bind_fuse,
        storage_dir,
        sandbox_dir,
        upper_cwd,
        overlay_cwd,
        log_level: partial_config.log_level.unwrap_or(log::LevelFilter::Info),
        ignored,
        sources,
    };

    trace!("Storage dir: {:?}", config.storage_dir);
    trace!("Sandbox: {:?}", config.name);

    Ok(config)
}

pub fn resolve_sandbox_storage_dir(
    storage_dir: Option<String>,
    uid: Uid,
    gid: Gid,
) -> Result<PathBuf> {
    /* Notably, NFS mounts are not acceptable for storing upper/work directories according to
     * overlayfs. */
    let acceptable_mount_types: [&str; 5] =
        ["btrfs", "ext4", "tmpfs", "xfs", "zfs"];

    #[cfg(feature = "coverage")]
    let acceptable_mount_types: [&str; 5] =
        if std::env::var_os("TEST_UNACCEPTABLE_MOUNT_TYPE").is_some() {
            ["", "", "", "", ""]
        } else {
            acceptable_mount_types.clone()
        };

    let proposed_storage_dirs = match storage_dir {
        Some(storage_dir) => vec![PathBuf::from(storage_dir)],
        None => {
            let ugh = resolve_uid_gid_home()?;
            vec![
                ugh.home.join(".sandboxes"),
                PathBuf::from(format!("/tmp/sandboxes-{}", ugh.uid)),
            ]
        }
    };

    for proposed_storage_dir in proposed_storage_dirs {
        let mount_point = find_mount_point(proposed_storage_dir.clone())?;

        // Open /proc/mounts
        let mounts =
            unsafe { libc::setmntent(c"/proc/mounts".as_ptr(), c"r".as_ptr()) };

        #[cfg(feature = "coverage")]
        let mounts =
            if std::env::var_os("TEST_UNABLE_TO_OPEN_PROC_MOUNTS").is_some() {
                std::ptr::null_mut()
            } else {
                mounts
            };

        if mounts.is_null() {
            return Err(anyhow::anyhow!("Failed to open /proc/mounts"));
        }

        // Search for our mount point
        let mut mnt_type = None;
        loop {
            let mnt = unsafe { libc::getmntent(mounts) };

            #[cfg(feature = "coverage")]
            let mnt = if std::env::var_os("TEST_UNABLE_TO_FIND_MOUNT_POINT")
                .is_some()
            {
                std::ptr::null_mut()
            } else {
                mnt
            };

            if mnt.is_null() {
                break;
            }

            let mnt_dir =
                unsafe { CStr::from_ptr((*mnt).mnt_dir).to_string_lossy() };
            if mnt_dir == mount_point.to_string_lossy() {
                mnt_type = Some(unsafe {
                    CStr::from_ptr((*mnt).mnt_type)
                        .to_string_lossy()
                        .into_owned()
                });
                break;
            }
        }

        unsafe { libc::endmntent(mounts) };

        if let Some(mnt_type) = mnt_type {
            /* Default to storing stuff in /tmp we're in an environment where home isn't
             * an acceptable place to store things, such as an an NFS mount. */
            if !acceptable_mount_types.contains(&mnt_type.as_str()) {
                trace!(
                    "Storage dir {:?} is on an unacceptable mount type: {:?}",
                    proposed_storage_dir, mnt_type
                );
                continue;
            } else {
                can_mkdir(&proposed_storage_dir, uid, gid).context(format!(
                    "Check to ensure we can create the storage directory {:?} failed",
                    proposed_storage_dir
                ))?;
                mkdir(&proposed_storage_dir, uid, gid).context(format!(
                    "Failed to create storage directory {:?}",
                    proposed_storage_dir
                ))?;
                trace!("Using storage directory: {:?}", proposed_storage_dir);
                return Ok(proposed_storage_dir);
            }
        }
    }

    Err(anyhow::anyhow!(
        "Failed to find a suitable storage directory. Please make sure the storage directory is set to a mount point that is one of the following types: {}",
        acceptable_mount_types.join(", ")
    ))
}

pub fn load_partial(
    no_config: bool,
) -> Result<(PartialConfig, HashMap<String, String>)> {
    let config_paths = if no_config {
        vec![]
    } else {
        find_config_files()?
    };
    let mut sources = HashMap::new();
    if config_paths.is_empty() {
        trace!("No config files found, using default config");
        return Ok((PartialConfig::default(), sources));
    }

    let mut merged_config = PartialConfig::default();
    for path in config_paths.iter() {
        let config_str = std::fs::read_to_string(path).context(format!(
            "Failed to read config file {}",
            path.display()
        ))?;

        let config: PartialConfig = toml::from_str(&config_str).context(
            format!("Failed to parse config file {}", path.display()),
        )?;

        merge_configs(
            &mut merged_config,
            &mut sources,
            config,
            path.to_str()
                .context("Failed to convert config path to str")?,
        );
        trace!("Loaded config file: {}", path.display());
    }

    Ok((merged_config, sources))
}

/** Returns a vec of all config files found */
fn find_config_files() -> Result<Vec<PathBuf>> {
    let uid_gid_home = resolve_uid_gid_home()?;
    let mut paths_to_check = Vec::new();

    // Any project specific files
    let mut current_dir = std::env::current_dir()?;
    loop {
        paths_to_check.push(current_dir.join(".sandbox.conf"));
        paths_to_check.push(current_dir.join(".sandbox.toml"));
        if current_dir == uid_gid_home.home || !current_dir.pop() {
            break;
        }
    }

    // ~/.config/sandbox/config.toml
    paths_to_check.push(uid_gid_home.home.join(".config/sandbox/config.conf"));
    paths_to_check.push(uid_gid_home.home.join(".config/sandbox/config.toml"));

    // /etc/sandbox.conf
    paths_to_check.push(PathBuf::from("/etc/sandbox.conf"));
    paths_to_check.push(PathBuf::from("/etc/sandbox.toml"));

    // Finally reverse them so we can process them in order nicely
    paths_to_check.reverse();

    Ok(paths_to_check
        .iter()
        .filter(|path| {
            path.exists()
                && can_access(
                    path,
                    uid_gid_home.uid,
                    uid_gid_home.gid,
                    nix::unistd::AccessFlags::R_OK,
                )
                .is_ok()
        })
        .cloned()
        .collect())
}

fn merge_configs(
    base: &mut PartialConfig,
    sources: &mut HashMap<String, String>,
    override_config: PartialConfig,
    source: &str,
) {
    if let Some(log_level) = override_config.log_level {
        base.log_level = Some(log_level);
        sources.insert("log_level".into(), source.into());
    }
    if let Some(name) = override_config.name {
        base.name = Some(name);
        sources.insert("name".into(), source.into());
    }
    if let Some(storage_dir) = override_config.storage_dir {
        base.storage_dir = Some(storage_dir);
        sources.insert("storage_dir".into(), source.into());
    }
    if let Some(net) = override_config.net {
        base.net = Some(net);
        sources.insert("net".into(), source.into());
    }
    if let Some(ignored) = override_config.ignored {
        base.ignored = Some(ignored);
        sources.insert("ignored".into(), source.into());
    }
}
