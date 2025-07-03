use super::mount::MountHash;
use crate::config::Config;
use crate::config::Network;
use crate::sandbox::Sandbox;
#[cfg(feature = "coverage")]
use crate::util::CLONE_FS;
use crate::util::Lock;
use crate::util::get_running_sandbox_pid;
use crate::util::get_sandbox_pid_path;
use crate::util::{
    CLONE_NEWCGROUP, CLONE_NEWIPC, CLONE_NEWNET, CLONE_NEWNS, CLONE_NEWPID,
    CLONE_NEWUTS, Clone3Args, check_path_for_mount_option_compatibility,
    clone3, mkdir, mount,
};
use anyhow::Context;
use anyhow::{Result, anyhow};
use log::{error, trace};
use nix::sys::stat::FchmodatFlags;
use nix::sys::stat::fchmodat;
use nix::{
    mount::MsFlags,
    mount::{MntFlags, umount2},
    sys::stat::{Mode, SFlag, makedev, mknod},
    unistd::{Gid, Pid, Uid, close, pipe, pivot_root, setsid},
};
#[cfg(feature = "coverage")]
use std::ffi::CString;
use std::os::{
    fd::{AsRawFd, OwnedFd},
    unix::fs::symlink,
};
use std::path::Path;
use std::path::PathBuf;
use uuid::Uuid;
#[cfg(feature = "coverage")]
unsafe extern "C" {
    fn __llvm_profile_set_filename(filename: *const i8);
    fn __llvm_profile_dump();
}

const TMPFS_SIZE: i32 = 64 * 1024 * 1024; // 64MB, because that's what docker does

impl Sandbox {
    pub fn from_location(
        sandboxes_storage_dir: &Path,
        sandbox_name: &str,
        uid: Uid,
        gid: Gid,
    ) -> Sandbox {
        let root_suffix = data_encoding::BASE32_NOPAD.encode(b"/");
        let sandbox_base_dir = sandboxes_storage_dir.join(sandbox_name);
        let work_base = sandbox_base_dir.join("work");
        let upper_base = sandbox_base_dir.join("upper");
        let overlay_base = sandbox_base_dir.join("overlay");

        Sandbox {
            name: sandbox_name.to_string(),
            base: sandbox_base_dir.clone(),
            work_base: work_base.clone(),
            upper_base: upper_base.clone(),
            overlay_base: overlay_base.clone(),
            root_overlay: overlay_base.join(&root_suffix),
            pid: Pid::from_raw(-1),
            uid,
            gid,
        }
    }

    pub fn get(
        sandboxes_storage_dir: &Path,
        sandbox_name: &str,
        uid: Uid,
        gid: Gid,
        lock: Option<Box<Lock>>,
    ) -> Result<(Option<Sandbox>, Box<Lock>)> {
        /* Lock so we don't have multiple processes trying to get at the same time */
        let lock = match lock {
            Some(l) => l,
            None => Lock::sandbox(sandboxes_storage_dir, sandbox_name)
                .context(format!("failed to lock sandbox: {}", sandbox_name))?,
        };

        let sandbox_base_dir = sandboxes_storage_dir.join(sandbox_name);
        if !sandbox_base_dir.exists() {
            return Ok((None, lock));
        }

        /* If we have an existing sandbox, return it */
        if let Some(pid) =
            get_running_sandbox_pid(sandboxes_storage_dir, sandbox_name)
        {
            let mut sandbox = Sandbox::from_location(
                sandboxes_storage_dir,
                sandbox_name,
                uid,
                gid,
            );
            sandbox.pid = pid;
            return Ok((Some(sandbox), lock));
        }

        Ok((None, lock))
    }

    pub fn get_or_create(
        config: &Config,
        uid: Uid,
        gid: Gid,
    ) -> Result<Sandbox> {
        let storage_dir = &config.storage_dir;
        let sandbox_name = &config.name;
        /* Ensure that the storage directory exists */
        mkdir(storage_dir, uid, gid).context(format!(
            "failed to create storage directory: {}",
            storage_dir.display()
        ))?;

        /* First lock so we don't have multiple processes trying to get_or_create at the same time */
        let lock = Lock::sandbox(storage_dir, sandbox_name)
            .context(format!("failed to lock sandbox: {}", sandbox_name))?;

        /* If we have an existing sandbox, return it */
        let (sandbox, _lock) =
            Sandbox::get(storage_dir, sandbox_name, uid, gid, Some(lock))?;

        if let Some(sandbox) = sandbox {
            return Ok(sandbox);
        }

        /*
         * Otherwise, create a new sandbox
         */
        let mut sandbox =
            Sandbox::from_location(storage_dir, sandbox_name, uid, gid);

        trace!(
            "Creating new sandbox '{}': {}",
            sandbox_name,
            sandbox.base.display()
        );

        /* Sanity check our storage path*/
        check_path_for_mount_option_compatibility(storage_dir).context(
            format!(
                "failed to check path for mount option compatibility: {}",
                storage_dir.display()
            ),
        )?;

        trace!("Mounting sandbox");
        let mounts = sandbox
            .mount()
            .context(format!("failed to mount sandbox: {}", sandbox_name))?;

        /* Create PID 1 for our sandbox */
        sandbox.pid = match sandbox.start_sandbox(
            config,
            &get_sandbox_pid_path(storage_dir, sandbox_name),
            &mounts,
        ) {
            Ok(pid) => pid,
            Err(e) => {
                return Err(anyhow!("Failed to start sandbox: {}", e));
            }
        };

        Ok(sandbox)
    }

    /**
     * This launches a new process with new namespaces. The process will go to sleep
     * but stick around to allow future processes to be launched in the same namespaces.
     */
    fn start_sandbox(
        &self,
        config: &Config,
        pid_file: &PathBuf,
        mounts: &Vec<MountHash>,
    ) -> Result<Pid> {
        // create a pipe between ourselves and the main sandbox process so the main sandbox process can
        // let us know when it's done setting up the namespaces
        let (read_fd, write_fd) =
            pipe().context("failed to create pipe for sandbox")?;

        let clone_args = Clone3Args {
            flags: CLONE_NEWNS
                | (if let Network::Host = config.net {
                    trace!("Using host network");
                    0
                } else {
                    trace!("Creating new network namespace");
                    CLONE_NEWNET
                })
                | CLONE_NEWPID
                | CLONE_NEWIPC
                | CLONE_NEWUTS
                | CLONE_NEWCGROUP,
            ..Default::default()
        };

        #[cfg(feature = "coverage")]
        let clone_args =
            if std::env::var_os("TEST_START_SANDBOX_FAILURE").is_some() {
                let mut new_args = clone_args.clone();
                new_args.flags |= CLONE_FS; // invalid flag combination with NEWNS
                new_args
            } else {
                clone_args
            };

        let pid = clone3(&clone_args)
            .context("clone3 call to start sandbox failed")?;

        if pid.as_raw() > 0 {
            // We are the parent process
            drop(write_fd);

            // write pid to pid file
            std::fs::write(pid_file, pid.to_string()).context(format!(
                "failed to write pid to pid file: {}",
                pid_file.display()
            ))?;

            // Wait for the child process to finish mounting procfs and doing whatever
            // setup stuff it needs to do. Once done, it sends a byte over the pipe, that's
            // our signal that the namespaces are ready for use.
            let mut buffer = [0; 1];
            nix::unistd::read(read_fd.as_raw_fd(), &mut buffer)
                .context("failed to read from sandbox ready pipe")?;

            if buffer[0] == 1 {
                return Err(anyhow!("Failed to setup sandbox"));
            }

            drop(read_fd);

            Ok(pid)
        } else {
            // We are the child process. We should never return from this branch.

            // we'll never read from our pipe, we only write a byte after we're done initializing to
            // signal that the namespaces are ready for use.
            drop(read_fd);

            // Normal operation is that this doesn't return.
            let _ = self
                .setup_sandbox_and_sleep_forever(config, &write_fd, mounts)
                .map_err(|e| {
                    error!("Failed to setup sandbox: {}", e);
                    e
                });

            // If we're here, an error occurred and we'll exit
            nix::unistd::write(&write_fd, &[1])
                .context("failed to write to sandbox ready pipe")?;

            std::process::exit(1);
        }
    }

    /* After forking via clone3, this is function gets called to finish setting up the sandbox.
     * namespaces. It should never return, if it does it's an error. */
    fn setup_sandbox_and_sleep_forever(
        &self,
        config: &Config,
        write_fd: &OwnedFd,
        mounts: &Vec<MountHash>,
    ) -> Result<()> {
        trace!("Setting up sandbox root process");

        #[cfg(feature = "coverage")]
        let cwd = nix::unistd::getcwd()?;

        // Create a new session and set our process group id
        setsid()
            .context("failed to create new session and set process group id")?;

        trace!("Preparing new filesystem namespace: /");
        // Prepare our new filesystem namespace
        let new_root = self.root_overlay.clone();
        let new_root_cstr = std::ffi::CString::new(
            new_root
                .to_str()
                .context("path contains invalid UTF-8 characters")?
                .as_bytes(),
        )
        .context("new_root_cstr creation error")?;
        let new_root_cstr = new_root_cstr.as_c_str();

        /* Ensure that 'new_root' and its parent mount don't have
         * shared propagation (which would cause pivot_root() to
         * return an error), and prevent propagation of mount
         * events to the initial mount namespace.
         *
         * source: pivot_root(2) man page
         */
        let null: Option<&str> = None;
        mount(null, "/", null, MsFlags::MS_REC | MsFlags::MS_PRIVATE, null)
            .context("failed to mount / with MS_REC | MS_PRIVATE")?;

        /* Ensure that 'new_root' is a mount point.
         *
         * source: pivot_root(2) man page
         */
        mount(Some(&new_root), &new_root, null, MsFlags::MS_BIND, null)
            .context("failed to mount new_root with MS_BIND")?;

        /* Create a place for the old root to go */
        let old_root_uuid = Uuid::new_v4();
        let old_root_local_path =
            PathBuf::from(format!("/old-root-{}", old_root_uuid));
        let old_root_host_path = PathBuf::from(format!(
            "{}{}",
            self.root_overlay.display(),
            old_root_local_path.display()
        ));
        let old_root_local = std::ffi::CString::new(
            old_root_local_path
                .to_str()
                .context(
                    "old_root_local_path contains invalid UTF-8 characters",
                )?
                .as_bytes(),
        )
        .context("old_root_local creation error")?;
        let old_root_local_cstr = old_root_local.as_c_str();
        let old_root_host = std::ffi::CString::new(
            old_root_host_path
                .to_str()
                .context(
                    "old_root_host_path contains invalid UTF-8 characters",
                )?
                .as_bytes(),
        )
        .context("old_root_host creation error")?;
        let old_root_host_cstr = old_root_host.as_c_str();

        if std::env::var_os("TEST_CREATE_OLD_ROOT_HOST_PATH_FAILURE").is_some()
        {
            // simulate a failure by pre-creating the directory so the real
            // creation below fails (exercising the error branch for tests).
            std::fs::create_dir(&old_root_host_path)?;
        }

        std::fs::create_dir(&old_root_host_path).context(format!(
            "Failed to create place to pivot our old root to {}",
            old_root_host_path.display()
        ))?;

        /* Prepare /dev here so that we can bind /dev/fuse if we need to.
         * This needs to happen before we pivot_root. */
        let new_root_dev = new_root.join("dev");
        mount(
            Some("none"),
            new_root_dev,
            Some("tmpfs"),
            MsFlags::MS_NOSUID,
            Some(format!("mode=0755,size={}", TMPFS_SIZE)),
        )?;

        /* Bind /dev/fuse if we have it enabled */
        if config.bind_fuse && Path::new("/dev/fuse").exists() {
            let new_root_dev_fuse = new_root.join("dev").join("fuse");
            std::fs::write(&new_root_dev_fuse, "").context(format!(
                "Failed to create {}",
                new_root_dev_fuse.display()
            ))?;
            mount(
                Some("/dev/fuse"),
                &new_root_dev_fuse,
                Some("bind"),
                MsFlags::MS_BIND,
                null,
            )
            .context(format!(
                "failed to bind /dev/fuse to {}",
                new_root_dev_fuse.display()
            ))?;
        }

        /* Prepare /run here so that we can bind /run/dbus and /run/systemd if we need to. This
         * needs to happen before we pivot_root. */
        let new_root_run = new_root.join("run");
        mount(
            Some("none"),
            new_root_run.clone(),
            Some("tmpfs"),
            MsFlags::MS_NOSUID,
            Some(format!("mode=0755,size={}", TMPFS_SIZE)),
        )?;

        /* Bind D-Bus sockets if using host networking */
        if matches!(config.net, Network::Host)
            && Path::new("/run/dbus").exists()
        {
            trace!("Binding system bus");
            let new_root_run_dbus = new_root_run.join("dbus");

            mkdir(
                &new_root_run_dbus,
                nix::unistd::Uid::from_raw(0),
                nix::unistd::Gid::from_raw(0),
            )?;

            mount(
                Some("/run/dbus"),
                &new_root_run_dbus,
                Some("bind"),
                MsFlags::MS_BIND,
                null,
            )
            .context(format!(
                "failed to bind mount /run/dbus to {}",
                new_root_run_dbus.display()
            ))?;
        }

        if matches!(config.net, Network::Host)
            && Path::new("/run/systemd").exists()
        {
            trace!("Binding systemd");
            let new_root_run_systemd = new_root_run.join("systemd");

            mkdir(
                &new_root_run_systemd,
                nix::unistd::Uid::from_raw(0),
                nix::unistd::Gid::from_raw(0),
            )?;

            mount(
                Some("/run/systemd"),
                &new_root_run_systemd,
                Some("bind"),
                MsFlags::MS_BIND | MsFlags::MS_NOSUID | MsFlags::MS_RDONLY,
                null,
            )
            .context(format!(
                "failed to bind mount /run/systemd to {}",
                new_root_run_systemd.display()
            ))?;
        }

        /* Pivot (similar to chroot in effect) */
        #[cfg(feature = "coverage")]
        let bad_path = std::ffi::CString::new("/non-existent-path")?;
        #[cfg(feature = "coverage")]
        let new_root_cstr =
            if std::env::var_os("TEST_PIVOT_ROOT_FAILURE").is_some() {
                bad_path.as_c_str()
            } else {
                new_root_cstr
            };

        pivot_root(new_root_cstr, old_root_host_cstr)
            .context("failed to pivot_root")?;

        /* Switch the current working directory to the new root */
        nix::unistd::chdir(c"/").context("failed to chdir to /")?;

        /* Bind all other overlays */
        for mnt in mounts {
            if mnt.dir == "/" {
                // Already mounted root
                continue;
            }

            let base = self.overlay_base.strip_prefix("/")?;

            trace!(
                "Binding overlay {} to {}",
                old_root_local_path.join(base).join(&mnt.hash).display(),
                mnt.dir
            );

            mount(
                Some(old_root_local_path.join(base).join(&mnt.hash)),
                &mnt.dir,
                Some("bind"),
                MsFlags::MS_BIND,
                null,
            )?;
        }

        /* Unmount old root */
        umount2(old_root_local_cstr, MntFlags::MNT_DETACH)
            .context("failed to unmount old root")?;

        /* Removes our temporary directory used for the pivot */
        std::fs::remove_dir(&old_root_local_path)
            .context("failed to remove old root")?;

        /* Mount procfs */
        mount(
            Some("proc"),
            "/proc",
            Some("proc"),
            MsFlags::MS_NOSUID | MsFlags::MS_NODEV | MsFlags::MS_NOEXEC,
            null,
        )?;

        // Setup /dev. The choices from these come from inspecting a running docker
        // container and doing what they appear to do, some more thorough research
        // should be done into what else might be needed in what circumstances.
        {
            for (path, (major, minor)) in [
                (c"/dev/null", (1, 3)),
                (c"/dev/zero", (1, 5)),
                (c"/dev/full", (1, 7)),
                (c"/dev/random", (1, 8)),
                (c"/dev/urandom", (1, 9)),
                (c"/dev/tty", (5, 0)),
            ] {
                let mode = Mode::S_IRUSR
                    | Mode::S_IWUSR
                    | Mode::S_IRGRP
                    | Mode::S_IWGRP
                    | Mode::S_IROTH
                    | Mode::S_IWOTH;

                mknod(path, SFlag::S_IFCHR, mode, makedev(major, minor))
                    .context(format!("failed to mknod {:?}", path))?;
                fchmodat(None, path, mode, FchmodatFlags::NoFollowSymlink)
                    .context(format!("failed to fchmodat {:?}", path))?;
            }

            // devpts: https://www.kernel.org/doc/Documentation/filesystems/devpts.txt
            mkdir(&PathBuf::from("/dev/pts"), self.uid, self.gid)?;
            mount(
                Some("devpts"),
                "/dev/pts",
                Some("devpts"),
                MsFlags::MS_NOSUID | MsFlags::MS_NOEXEC,
                Some("mode=620,ptmxmode=666"),
            )?;

            // /dev symlinks
            for (src, dst) in [
                ("/proc/kcore", "/dev/core"),
                ("/proc/self/fd/0", "/dev/stdin"),
                ("/proc/self/fd/1", "/dev/stdout"),
                ("/proc/self/fd/2", "/dev/stderr"),
                ("/proc/self/fd", "/dev/fd"),
                ("/dev/pts/ptmx", "/dev/ptmx"),
            ] {
                symlink(src, dst)
                    .context(format!("failed to symlink {} {}", src, dst))?;
            }

            // mqueue
            mkdir(&PathBuf::from("/dev/mqueue"), self.uid, self.gid)?;
            mount(
                Some("mqueue"),
                "/dev/mqueue",
                Some("mqueue"),
                MsFlags::MS_NOSUID | MsFlags::MS_NOEXEC | MsFlags::MS_NODEV,
                null,
            )?;

            // shm
            mkdir(&PathBuf::from("/dev/shm"), self.uid, self.gid)?;
            mount(
                Some("shm"),
                "/dev/shm",
                Some("tmpfs"),
                MsFlags::MS_NOSUID | MsFlags::MS_NOEXEC | MsFlags::MS_NODEV,
                Some(format!("size={}", TMPFS_SIZE)),
            )?;
        }

        // Mount /sys
        {
            mount(
                Some("sysfs"),
                "/sys",
                Some("sysfs"),
                MsFlags::MS_NOSUID | MsFlags::MS_NODEV | MsFlags::MS_RDONLY,
                null,
            )?;

            // mount /sys/fs/cgroup
            mount(
                Some("cgroup"),
                "/sys/fs/cgroup",
                Some("cgroup2"),
                MsFlags::MS_NOSUID
                    | MsFlags::MS_NODEV
                    | MsFlags::MS_NOEXEC
                    | MsFlags::MS_RDONLY,
                null,
            )?;
        }

        /* Mask out certain things from /proc and /sys to help isolate the sandbox.
         * These are based off of the things that docker masks out by default, see
         *
         * https://github.com/moby/moby/blob/master/oci/defaults.go
         * https://github.com/moby/moby/blob/master/vendor/github.com/containerd/containerd/v2/pkg/oci/spec.go
         *
         */

        let mut masked_paths = vec![
            "/proc/asound",
            "/proc/acpi",
            "/proc/interrupts", // https://github.com/moby/moby/security/advisories/GHSA-6fw5-f8r9-fgfm
            "/proc/kcore",
            "/proc/keys",
            "/proc/latency_stats",
            "/proc/timer_list",
            "/proc/timer_stats",
            "/proc/sched_debug",
            "/proc/scsi",
            "/sys/firmware",
            "/sys/devices/virtual/powercap", // https://github.com/moby/moby/security/advisories/GHSA-jq35-85cj-fj4p
        ];

        let readonly_paths = [
            "/proc/bus",
            "/proc/fs",
            "/proc/irq",
            "/proc/sys",
            "/proc/sysrq-trigger",
        ];

        // https://github.com/moby/moby/security/advisories/GHSA-6fw5-f8r9-fgfm
        std::fs::read_dir("/sys/devices/system/cpu").map(|entries| {
            for entry in entries.flatten() {
                let path = entry.path();
                let thermal_path = path.join("thermal_throttle");
                if let Some(path_str) = thermal_path.to_str() {
                    let path_literal =
                        Box::leak(path_str.to_string().into_boxed_str());
                    masked_paths.push(path_literal);
                }
            }
        })?;

        /* Masked paths */
        for path in masked_paths.iter().filter(|path| {
            Path::new(path).exists()
                && (Path::new(path).is_dir() || Path::new(path).is_file())
        }) {
            if Path::new(path).is_dir() {
                // directories we mask with tmpfs
                mount(
                    Some("mask"),
                    path,
                    Some("tmpfs"),
                    MsFlags::MS_NOSUID | MsFlags::MS_NODEV | MsFlags::MS_RDONLY,
                    Some(format!("size={}", TMPFS_SIZE)),
                )
                .context(format!("failed to mask {} with tmpfs", path))?;
            } else {
                // files we rebind to dev null
                mount(
                    Some("/dev/null"),
                    path,
                    Some("bind"),
                    MsFlags::MS_BIND,
                    null,
                )
                .context(format!("failed to mask {} with bind", path))?;
            }
        }

        /* Readonly paths */
        for path in readonly_paths
            .iter()
            .filter(|path| Path::new(path).exists())
        {
            mount(Some(&path), path, Some("bind"), MsFlags::MS_BIND, null)
                .context(format!("failed to bind mount {}", path))?;
            // After creating the bind mount, remount it read-only
            mount(
                Some("none"),
                path,
                Some("bind"),
                MsFlags::MS_BIND
                    | MsFlags::MS_REMOUNT
                    | MsFlags::MS_NODEV
                    | MsFlags::MS_NOEXEC
                    | MsFlags::MS_NOSUID
                    | MsFlags::MS_RDONLY,
                null,
            )
            .context(format!("failed to remount {} read-only", path))?;
        }

        // Set our hostname
        nix::unistd::sethostname(self.name.as_str())
            .context(format!("failed to set hostname: {}", self.name))?;

        // send a byte over the pipe to signal that the sandbox is initialized and
        // ready for use
        nix::unistd::write(write_fd, &[0])
            .context(format!("failed to write to sandbox ready pipe"))?;
        close(write_fd.as_raw_fd())
            .context(format!("failed to close sandbox ready pipe"))?;

        trace!("Sandbox setup complete");

        /* Close all file descriptors so our test harness doesn't hang waiting for them to close
         * before returning a result. */
        close(std::io::stdout().as_raw_fd())
            .context(format!("failed to close stdout"))?;
        close(std::io::stderr().as_raw_fd())
            .context(format!("failed to close stderr"))?;
        close(std::io::stdin().as_raw_fd())
            .context(format!("failed to close stdin"))?;

        unsafe {
            libc::signal(
                libc::SIGTERM,
                std::process::exit as libc::sighandler_t,
            );
        }

        // sleep forever, this process hangs around only to host the namespaces so other processes
        // can join in.
        let mut count = 0;
        loop {
            std::thread::sleep(std::time::Duration::from_millis(count));
            /* Cheap hack to fill out coverage */
            count += 1;
            if count >= 2 {
                count += 10000;
                // Flush gcov coverage data before we spin forever
                #[cfg(feature = "coverage")]
                {
                    nix::unistd::chdir(&cwd)?;
                    unsafe {
                        __llvm_profile_set_filename(
                            CString::new(format!("coverage/profraw/setup_sandbox-{}-{}-%m.profraw", self.pid, std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs()).as_str(),)
                                .unwrap()
                                .as_ptr(),
                        );
                        __llvm_profile_dump();
                    }
                }
            }
        }
    }
}
