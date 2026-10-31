//! cgroup v2 support for limiting the CPU and memory available to a sandbox.
//!
//! Sandboxes with resource limits get a cgroup at
//! /sys/fs/cgroup/sandbox/<uid>-<name>. The sandbox's PID 1 is started in that
//! cgroup, which also makes it the root of the sandbox's cgroup namespace, and
//! every process that later joins the sandbox is moved into it.

use crate::config::Config;
use crate::sandbox::{Sandbox, SandboxSettings, describe_limits};
use anyhow::{Context, Result, anyhow};
use log::{debug, trace, warn};
use std::os::fd::OwnedFd;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const CGROUP_ROOT: &str = "/sys/fs/cgroup";
const CPU_PERIOD_US: u64 = 100_000;

/// The cgroup all sandbox cgroups are created in
fn cgroup_parent() -> String {
    let parent = String::from("sandbox");

    // Lets tests start from a parent cgroup that doesn't exist yet
    #[cfg(feature = "coverage")]
    let parent = std::env::var("TEST_CGROUP_PARENT").unwrap_or(parent);

    parent
}

impl Sandbox {
    fn cgroup_name(&self) -> String {
        format!("{}-{}", self.uid, self.name)
    }

    pub(crate) fn cgroup_path(&self) -> PathBuf {
        Path::new(CGROUP_ROOT)
            .join(cgroup_parent())
            .join(self.cgroup_name())
    }

    /// Sets up our cgroup like create_cgroup, except that inside another
    /// sandbox, where cgroups can't be managed, we warn and run without a
    /// cgroup of our own. We're still in the outer sandbox's cgroup, so its
    /// limits continue to apply.
    pub(crate) fn create_cgroup_or_share_outer(
        &self,
        config: &Config,
    ) -> Result<Option<OwnedFd>> {
        let error = match self.create_cgroup(config) {
            Ok(cgroup) => return Ok(cgroup),
            Err(e) => e.context("Failed to set up resource limits"),
        };

        // A nested sandbox's storage directory is the outer sandbox's data
        // directory, which holds the outer sandbox's settings
        let outer_settings = config.storage_dir.join("settings.json");
        let Ok(outer) = SandboxSettings::load_from_file(&outer_settings) else {
            return Err(error);
        };

        debug!("{:#}", error);
        warn!(
            "Resource limits can't be set up inside another sandbox, so this sandbox shares the limits of the sandbox it's running in ({})",
            describe_limits(outer.cpus, outer.memory)
        );
        Ok(None)
    }

    /// Creates a cgroup with the resource limits from our config. Returns a
    /// handle to it for use with CLONE_INTO_CGROUP, or None if the config
    /// has no resource limits.
    pub(crate) fn create_cgroup(
        &self,
        config: &Config,
    ) -> Result<Option<OwnedFd>> {
        let mut controllers = vec![];
        if config.cpus.is_some() {
            controllers.push("cpu");
        }
        if config.memory.is_some() {
            controllers.push("memory");
        }
        if controllers.is_empty() {
            return Ok(None);
        }

        let root = Path::new(CGROUP_ROOT);

        #[cfg(feature = "coverage")]
        let root = if std::env::var_os("TEST_NO_CGROUP2").is_some() {
            Path::new("/does-not-exist")
        } else {
            root
        };

        if !root.join("cgroup.controllers").exists() {
            return Err(anyhow!(
                "Limiting cpus or memory requires cgroup v2, which is not mounted at {}",
                root.display()
            ));
        }

        let parent = root.join(cgroup_parent());
        enable_controllers(root, &controllers)?;
        if !parent.exists() {
            let context =
                format!("Failed to create cgroup {}", parent.display());
            std::fs::create_dir(&parent).context(context)?;
        }
        enable_controllers(&parent, &controllers)?;

        let path = self.cgroup_path();
        if path.exists() {
            // Left behind by a sandbox that wasn't stopped cleanly, which we
            // can remove because it's empty. If it's not empty it belongs to
            // a running sandbox with the same name in another storage dir.
            std::fs::remove_dir(&path).context(format!(
                "Failed to remove existing cgroup {}, is a sandbox with the same name running from a different storage directory?",
                path.display()
            ))?;
        }
        trace!("Creating cgroup {}", path.display());
        std::fs::create_dir(&path)
            .context(format!("Failed to create cgroup {}", path.display()))?;

        let result = self.write_limits(&path, config).and_then(|_| {
            std::fs::File::open(&path)
                .map(OwnedFd::from)
                .context(format!("Failed to open cgroup {}", path.display()))
        });
        if result.is_err() {
            remove_cgroup(&path);
        }
        result.map(Some)
    }

    fn write_limits(&self, path: &Path, config: &Config) -> Result<()> {
        if let Some(cpus) = config.cpus {
            let quota = (cpus * CPU_PERIOD_US as f64).round() as u64;
            write_cgroup_file(
                path,
                "cpu.max",
                &format!("{} {}", quota, CPU_PERIOD_US),
            )?;
        }
        if let Some(memory) = config.memory {
            write_cgroup_file(path, "memory.max", &memory.to_string())?;
            // Swap would let the sandbox use more memory than its limit, but
            // the file only exists when swap accounting is enabled.
            let swap_accounting = path.join("memory.swap.max").exists();

            #[cfg(feature = "coverage")]
            let swap_accounting = swap_accounting
                && std::env::var_os("TEST_NO_SWAP_ACCOUNTING").is_none();

            if swap_accounting {
                write_cgroup_file(path, "memory.swap.max", "0")?;
            }
        }
        Ok(())
    }

    /// Returns the path to our cgroup if our sandbox is running in one. We
    /// go by the cgroup our PID 1 is in rather than the name we'd give a new
    /// cgroup, because the uid in the name is that of the user who started
    /// the sandbox, which isn't always ours (e.g. `delete` doesn't know it).
    pub(crate) fn running_cgroup(&self) -> Option<PathBuf> {
        let cgroups =
            std::fs::read_to_string(format!("/proc/{}/cgroup", self.pid))
                .ok()?;
        let current =
            cgroups.lines().find_map(|line| line.strip_prefix("0::"))?;
        let relative = Path::new(current).strip_prefix("/").ok()?;
        let cgroup_name =
            relative.strip_prefix(cgroup_parent()).ok()?.to_str()?;
        let (uid, name) = cgroup_name.split_once('-')?;
        (uid.parse::<u32>().is_ok() && name == self.name)
            .then(|| Path::new(CGROUP_ROOT).join(relative))
    }

    /// Moves the current process into our sandbox's cgroup, if it has one,
    /// so processes we launch in the sandbox are subject to its limits.
    pub(crate) fn join_cgroup(&self) -> Result<()> {
        if let Some(path) = self.running_cgroup() {
            trace!("Joining cgroup {}", path.display());
            let pid = std::process::id().to_string();
            write_cgroup_file(&path, "cgroup.procs", &pid)?;
        }
        Ok(())
    }
}

/// Removes a cgroup once the processes in it have exited, waiting a little
/// while for that. Sandbox processes have already been killed by this point.
/// The only other processes in the cgroup are the `sandbox` commands waiting
/// on them, which exit by themselves once their child is gone, so they get to
/// report its exit status instead of being killed too.
pub(crate) fn remove_cgroup(path: &Path) {
    trace!("Removing cgroup {}", path.display());
    let deadline = Instant::now() + Duration::from_secs(5);
    while std::fs::remove_dir(path).is_err() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn write_cgroup_file(dir: &Path, file: &str, value: &str) -> Result<()> {
    let path = dir.join(file);

    #[cfg(feature = "coverage")]
    let path = if std::env::var_os("TEST_CGROUP_WRITE_FAILURE")
        .is_some_and(|f| f == file)
    {
        PathBuf::from("/does-not-exist").join(file)
    } else {
        path
    };

    std::fs::write(&path, value).context(format!(
        "Failed to write '{}' to {}",
        value,
        path.display()
    ))
}

/// Enables controllers for a cgroup's children if they aren't already
fn enable_controllers(dir: &Path, controllers: &[&str]) -> Result<()> {
    let subtree_control = dir.join("cgroup.subtree_control");
    let context = format!("Failed to read {}", subtree_control.display());
    let enabled = std::fs::read_to_string(&subtree_control).context(context)?;
    for controller in controllers {
        if !enabled.split_whitespace().any(|c| c == *controller) {
            let context = format!(
                "Failed to enable the {} cgroup controller. Limiting cpus or memory requires permission to manage {}, which may not be available inside containers or other sandboxes",
                controller,
                dir.display()
            );
            let value = format!("+{}", controller);
            write_cgroup_file(dir, "cgroup.subtree_control", &value)
                .context(context)?;
        }
    }
    Ok(())
}
