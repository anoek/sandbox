use super::PartialConfig;
use crate::config::Network;
use crate::util::{can_access, can_mkdir, find_mount_point, mkdir};
use crate::{config::Config, types::UidGidHome, util::resolve_uid_gid_home};
use anyhow::{Context, Result};
use chrono::Local;
use log::trace;
use nix::unistd::{Gid, Uid};
use std::collections::{HashMap, HashSet};
use std::time::{Duration, SystemTime};
use std::{env, str::FromStr};
use std::{
    ffi::CStr,
    path::{Path, PathBuf},
};

use super::cli::Args;

pub fn resolve_config(cli: Args) -> Result<Config> {
    let uid_gid_home = resolve_uid_gid_home()?;
    let (mut partial_config, mut sources) = load_partial(cli.no_config)?;

    // Check for mutually exclusive name options
    if (cli.new || cli.last)
        && (partial_config.name.is_some() || env::var("SANDBOX_NAME").is_ok())
    {
        // If --new or --last is specified, name must not be provided via env or config
        return Err(anyhow::anyhow!(
            "Cannot use --new or --last when name is specified in config file or environment variable"
        ));
    }

    // Override with environment variables if set
    if let Ok(log_level) = env::var("SANDBOX_LOG_LEVEL") {
        if let Ok(log_level) = log::LevelFilter::from_str(&log_level) {
            partial_config.log_level = Some(log_level);
            sources.insert("log_level".into(), "environment".into());
        } else {
            return Err(anyhow::anyhow!("Invalid log level: {}", log_level));
        }
    }
    if !cli.new && !cli.last {
        if let Ok(name) = env::var("SANDBOX_NAME") {
            if !name.is_empty() {
                partial_config.name = Some(name);
                sources.insert("name".into(), "environment".into());
            }
        }
    }
    if let Ok(storage_dir) = env::var("SANDBOX_STORAGE_DIR") {
        if !storage_dir.is_empty() {
            partial_config.storage_dir = Some(storage_dir);
            sources.insert("storage_dir".into(), "environment".into());
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

    // Handle bind mounts from environment variable (additive)
    if let Ok(bind_mounts_env) = env::var("SANDBOX_BIND") {
        let env_bind_mounts: Vec<String> = bind_mounts_env
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        if !env_bind_mounts.is_empty() {
            match &mut partial_config.bind_mounts {
                Some(existing) => existing.extend(env_bind_mounts),
                None => partial_config.bind_mounts = Some(env_bind_mounts),
            }
            sources.insert("bind_mounts".into(), "environment".into());
        }
    }

    // Handle mask paths from environment variable (additive)
    if let Ok(mask_paths_env) = env::var("SANDBOX_MASK") {
        let env_mask_paths: Vec<String> = mask_paths_env
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .map(|path| format!("{}:{}:mask", path, path))
            .collect();

        if !env_mask_paths.is_empty() {
            match &mut partial_config.bind_mounts {
                Some(existing) => existing.extend(env_mask_paths),
                None => partial_config.bind_mounts = Some(env_mask_paths),
            }
            sources.insert("bind_mounts".into(), "environment".into());
        }
    }

    if let Ok(no_default_binds_env) = env::var("SANDBOX_NO_DEFAULT_BINDS") {
        let no_default_binds_val = bool::from_str(&no_default_binds_env)
            .expect("Invalid value for SANDBOX_NO_DEFAULT_BINDS");
        partial_config.no_default_binds = Some(no_default_binds_val);
        sources.insert("no_default_binds".into(), "environment".into());
    }

    // Override with CLI args if provided (highest precedence)
    if let Some(log_level) = cli.log_level {
        partial_config.log_level = Some(log_level);
        sources.insert("log_level".into(), "cli".into());
    }

    // Set storage_dir first if provided via CLI, as it's needed for --new and --last
    if let Some(storage_dir) = cli.storage_dir {
        partial_config.storage_dir = Some(storage_dir);
        sources.insert("storage_dir".into(), "cli".into());
    }

    // Handle name resolution based on CLI flags
    if cli.new {
        // Generate a new timestamp-based name
        let name = generate_timestamp_name(
            &partial_config.storage_dir,
            uid_gid_home.uid,
            uid_gid_home.gid,
        )?;
        partial_config.name = Some(name);
        sources.insert("name".into(), "cli --new".into());
    } else if cli.last {
        // Find the most recently created sandbox
        let name = find_last_sandbox(
            &partial_config.storage_dir,
            uid_gid_home.uid,
            uid_gid_home.gid,
        )?;
        partial_config.name = Some(name);
        sources.insert("name".into(), "cli --last".into());
    } else if let Some(name) = cli.name {
        partial_config.name = Some(name);
        sources.insert("name".into(), "cli".into());
    }

    if let Some(net) = cli.net {
        partial_config.net = Some(net);
        sources.insert("net".into(), "cli".into());
    }

    if cli.ignored {
        partial_config.ignored = Some(true);
        sources.insert("ignored".into(), "cli".into());
    }

    if cli.no_default_binds {
        partial_config.no_default_binds = Some(true);
        sources.insert("no_default_binds".into(), "cli".into());
    }

    // Handle bind mounts from CLI (additive)
    if let Some(cli_bind_mounts) = cli.bind {
        match &mut partial_config.bind_mounts {
            Some(existing) => existing.extend(cli_bind_mounts),
            None => partial_config.bind_mounts = Some(cli_bind_mounts),
        }
    }

    // Handle mask entries from CLI - convert to bind mounts with mask option
    if let Some(cli_mask_paths) = cli.mask {
        let mask_bind_mounts: Vec<String> = cli_mask_paths
            .into_iter()
            .map(|path| format!("{}:{}:mask", path, path))
            .collect();
        
        match &mut partial_config.bind_mounts {
            Some(existing) => existing.extend(mask_bind_mounts),
            None => partial_config.bind_mounts = Some(mask_bind_mounts),
        }
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

    let ignored = partial_config.ignored.unwrap_or(false);
    if !sources.contains_key("ignored") {
        sources.insert("ignored".into(), "default".into());
    }

    let no_default_binds = partial_config.no_default_binds.unwrap_or(false);
    if !sources.contains_key("no_default_binds") {
        sources.insert("no_default_binds".into(), "default".into());
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

    // Collect all bind mounts including defaults
    let mut all_bind_mounts = vec![];

    // Add default bind mounts unless disabled
    if !no_default_binds {
        let default_binds = get_default_bind_mounts(&net, &uid_gid_home);
        all_bind_mounts.extend(default_binds);
    }

    if let Some(bind_mounts) = partial_config.bind_mounts {
        all_bind_mounts.extend(bind_mounts);
    }

    // Deduplicate bind mounts, preserving order
    let mut bind_set: HashSet<String> = HashSet::new();
    let bind_mounts: Vec<String> = all_bind_mounts
        .into_iter()
        .filter(|mount| {
            if bind_set.contains(mount.as_str()) {
                return false;
            }
            bind_set.insert(mount.clone());
            true
        })
        .collect();

    if bind_mounts.is_empty() {
        sources.insert("bind_mounts".into(), "default".into());
    }

    let config = Config {
        name,
        net,
        storage_dir,
        sandbox_dir,
        upper_cwd,
        overlay_cwd,
        log_level: partial_config.log_level.unwrap_or(log::LevelFilter::Info),
        ignored,
        sources,
        bind_mounts,
        no_default_binds,
    };

    validate_config(&config)?;

    trace!("Storage dir: {:?}", config.storage_dir);
    trace!("Sandbox: {:?}", config.name);

    Ok(config)
}

fn get_default_bind_mounts(
    net: &Network,
    uid_gid_home: &UidGidHome,
) -> Vec<String> {
    let mut default_binds = Vec::new();

    // System binds

    // /dev/fuse - for AppImages and FUSE filesystems
    if Path::new("/dev/fuse").exists() {
        default_binds.push("/dev/fuse".to_string());
    }

    // D-Bus and systemd sockets (only when using host networking)
    if matches!(net, Network::Host) {
        // System D-Bus
        if Path::new("/run/dbus").exists() {
            default_binds.push("/run/dbus".to_string());
        }

        // User D-Bus from XDG_RUNTIME_DIR
        let xdg_runtime_dir =
            env::var("XDG_RUNTIME_DIR").ok().filter(|s| !s.is_empty());

        #[cfg(feature = "coverage")]
        let xdg_runtime_dir =
            if std::env::var_os("TEST_EMPTY_XDG_RUNTIME_DIR").is_some() {
                None
            } else {
                xdg_runtime_dir
            };

        if let Some(xdg_runtime_dir) = xdg_runtime_dir {
            let bus_path = Path::new(&xdg_runtime_dir).join("bus");
            if bus_path.exists() {
                default_binds.push(bus_path.to_string_lossy().to_string());
            }
        }

        // Systemd
        if Path::new("/run/systemd").exists() {
            default_binds.push("/run/systemd".to_string());
        }
    }

    // User directories for AI coding assistants
    let user_dirs = [
        ".claude", // Claude artifacts and settings
        ".aider",  // Aider chat history and settings
        ".cursor", // Cursor settings
    ];

    for dir in &user_dirs {
        let user_dir = uid_gid_home.home.join(dir);
        if user_dir.exists() {
            default_binds.push(format!("{}", user_dir.display()));
        }
    }

    default_binds
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
    if let Some(no_default_binds) = override_config.no_default_binds {
        base.no_default_binds = Some(no_default_binds);
        sources.insert("no_default_binds".into(), source.into());
    }
    // Handle bind_mounts additively
    if let Some(bind_mounts) = override_config.bind_mounts {
        match &mut base.bind_mounts {
            Some(existing) => existing.extend(bind_mounts),
            None => base.bind_mounts = Some(bind_mounts),
        }
    }
}

fn validate_config(config: &Config) -> Result<()> {
    if config.name.contains("/") {
        return Err(anyhow::anyhow!("Invalid sandbox name: {}", config.name));
    }

    Ok(())
}

fn generate_timestamp_name(
    storage_dir: &Option<String>,
    uid: Uid,
    gid: Gid,
) -> Result<String> {
    let storage_dir =
        resolve_sandbox_storage_dir(storage_dir.clone(), uid, gid)?;

    #[cfg(feature = "coverage")]
    let mut iter = 0;

    loop {
        let now = Local::now();
        let base_name =
            format!("ephemeral_{}", now.format("%Y-%m-%d_%H:%M:%S"));

        // Check if this name already exists
        let sandbox_path = storage_dir.join(&base_name);
        if !sandbox_path.exists() {
            return Ok(base_name);
        }

        // If it exists, try with millisecond precision
        let name_with_ms =
            format!("{}.{:03}", base_name, now.timestamp_subsec_millis());

        #[cfg(feature = "coverage")]
        let name_with_ms =
            if std::env::var_os("TEST_NAME_WITH_MS").is_some() && iter == 0 {
                std::env::var("TEST_NAME_WITH_MS").unwrap()
            } else {
                name_with_ms
            };

        let sandbox_path_ms = storage_dir.join(&name_with_ms);
        if !sandbox_path_ms.exists() {
            return Ok(name_with_ms);
        }

        // If even that exists, sleep for a millisecond and try again
        std::thread::sleep(Duration::from_millis(1));

        #[cfg(feature = "coverage")]
        {
            iter += 1;
        }
    }
}

fn find_last_sandbox(
    storage_dir: &Option<String>,
    uid: Uid,
    gid: Gid,
) -> Result<String> {
    let storage_dir =
        resolve_sandbox_storage_dir(storage_dir.clone(), uid, gid)?;

    let entries = std::fs::read_dir(&storage_dir).context(format!(
        "Failed to read sandbox storage directory: {:?}",
        storage_dir
    ))?;

    let mut sandboxes: Vec<(String, SystemTime)> = Vec::new();

    for entry in entries {
        let entry = entry?;
        let path = entry.path();

        // Check if this is a valid sandbox directory
        if path.is_dir()
            && path.join("upper").is_dir()
            && path.join("work").is_dir()
            && path.join("overlay").is_dir()
        {
            let name = entry.file_name().to_string_lossy().to_string();
            let metadata = entry.metadata()?;
            let created = metadata
                .created()
                .or_else(|_| metadata.modified())
                .context("Failed to get creation time for sandbox")?;
            sandboxes.push((name, created));
        }
    }

    #[cfg(feature = "coverage")]
    let mut sandboxes = if std::env::var_os("TEST_LAST_IS_EMPTY").is_some() {
        vec![]
    } else {
        sandboxes
    };

    if sandboxes.is_empty() {
        return Err(anyhow::anyhow!("No sandboxes found"));
    }

    // Sort by creation time (newest first)
    sandboxes.sort_by(|a, b| b.1.cmp(&a.1));

    Ok(sandboxes[0].0.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Network;
    use log::LevelFilter;
    use std::collections::HashMap;
    use std::sync::Mutex;

    // Mutex to ensure XDG_RUNTIME_DIR tests don't interfere with each other
    static XDG_TEST_MUTEX: Mutex<()> = Mutex::new(());

    #[test]
    fn ai_test_get_default_bind_mounts_xdg_runtime_dir() {
        let _guard = XDG_TEST_MUTEX.lock().unwrap();
        use crate::types::UidGidHome;
        use std::path::PathBuf;

        let uid_gid_home = UidGidHome {
            uid: Uid::from_raw(1000),
            gid: Gid::from_raw(1000),
            home: PathBuf::from("/home/test"),
        };

        // Test 1: XDG_RUNTIME_DIR is set, but TEST_EMPTY_XDG_RUNTIME_DIR overrides it
        unsafe {
            env::set_var("XDG_RUNTIME_DIR", "/tmp/xdg");
            env::set_var("TEST_EMPTY_XDG_RUNTIME_DIR", "1");
        }

        let binds = get_default_bind_mounts(&Network::Host, &uid_gid_home);
        assert!(!binds.iter().any(|b| b.contains("/tmp/xdg/bus")));

        unsafe {
            env::remove_var("TEST_EMPTY_XDG_RUNTIME_DIR");
        }

        // Test 2: XDG_RUNTIME_DIR is not set
        unsafe {
            env::remove_var("XDG_RUNTIME_DIR");
        }

        let binds = get_default_bind_mounts(&Network::Host, &uid_gid_home);
        assert!(!binds.iter().any(|b| b.contains("/bus")));

        // Test 3: XDG_RUNTIME_DIR is set to a test path that doesn't exist
        unsafe {
            env::set_var("XDG_RUNTIME_DIR", "/tmp/test_xdg_runtime_dir");
        }

        // Since /tmp/test_xdg_runtime_dir/bus won't exist, it shouldn't be included
        let binds = get_default_bind_mounts(&Network::Host, &uid_gid_home);
        assert!(
            !binds
                .iter()
                .any(|b| b.contains("/tmp/test_xdg_runtime_dir/bus"))
        );

        // Test 4: Test with Network::None (should not include any XDG bus)
        let binds = get_default_bind_mounts(&Network::None, &uid_gid_home);
        assert!(!binds.iter().any(|b| b.contains("/bus")));
    }

    #[test]
    fn test_validate_config() {
        let mut config = Config {
            name: "test-sandbox".to_string(),
            storage_dir: PathBuf::from("/tmp/test"),
            sandbox_dir: PathBuf::from("/tmp/test"),
            upper_cwd: PathBuf::from("/tmp/test"),
            overlay_cwd: PathBuf::from("/tmp/test"),
            net: Network::Host,
            sources: HashMap::new(),
            ignored: false,
            log_level: LevelFilter::Info,
            bind_mounts: vec![],
            no_default_binds: false,
        };
        assert!(validate_config(&config).is_ok());
        config.name = "../test-sandbox".to_string();
        assert!(validate_config(&config).is_err());
        config.name = "test-sandbox/".to_string();
        assert!(validate_config(&config).is_err());
        config.name = "test-sandbox\\".to_string();
        assert!(validate_config(&config).is_ok());
        config.name = "test-sandbox.".to_string();
        assert!(validate_config(&config).is_ok());
        config.name = ".test".to_string();
        assert!(validate_config(&config).is_ok());
    }

    #[test]
    fn test_merge_configs() {
        let mut base = PartialConfig::default();
        let mut sources = HashMap::new();
        let mut base_with_binds = PartialConfig {
            bind_mounts: Some(vec!["/tmp/test1".to_string()]),
            ..PartialConfig::default()
        };

        let override_config = PartialConfig {
            log_level: Some(LevelFilter::Debug),
            name: Some("test-sandbox".to_string()),
            storage_dir: Some("/tmp/test".to_string()),
            net: Some(Network::Host),
            ignored: Some(true),
            bind_mounts: Some(vec![
                "/tmp/test1".to_string(),
                "/tmp/test2".to_string(),
            ]),
            no_default_binds: Some(true),
        };

        merge_configs(
            &mut base,
            &mut sources,
            override_config.clone(),
            "test-config",
        );
        merge_configs(
            &mut base_with_binds,
            &mut sources,
            override_config,
            "test-config",
        );

        // Verify all fields were merged
        assert_eq!(base.log_level, Some(LevelFilter::Debug));
        assert_eq!(base.name, Some("test-sandbox".to_string()));
        assert_eq!(base.storage_dir, Some("/tmp/test".to_string()));
        assert!(matches!(base.net, Some(Network::Host)));
        assert_eq!(base.ignored, Some(true));
        assert_eq!(
            base.bind_mounts,
            Some(vec!["/tmp/test1".to_string(), "/tmp/test2".to_string()])
        );
        assert_eq!(
            base_with_binds.bind_mounts,
            Some(vec![
                "/tmp/test1".to_string(),
                "/tmp/test1".to_string(),
                "/tmp/test2".to_string(),
            ])
        );
        assert_eq!(base.no_default_binds, Some(true));

        // Verify sources tracking
        assert_eq!(sources.get("log_level"), Some(&"test-config".to_string()));
        assert_eq!(sources.get("name"), Some(&"test-config".to_string()));
        assert_eq!(
            sources.get("storage_dir"),
            Some(&"test-config".to_string())
        );
        assert_eq!(sources.get("net"), Some(&"test-config".to_string()));
        assert_eq!(sources.get("ignored"), Some(&"test-config".to_string()));
        assert_eq!(
            sources.get("no_default_binds"),
            Some(&"test-config".to_string())
        );
    }
}
