use crate::sandbox::Sandbox;
use anyhow::{Context, Result};
use log::trace;
use nix::{fcntl::readlink, unistd::Pid};
use std::path::{Path, PathBuf};

impl Sandbox {
    pub fn stop(&self) -> Result<()> {
        let pid = self.pid;
        if pid == Pid::from_raw(-1) {
            trace!("Sandbox '{}' doesn't appear to be running", self.name);
        } else {
            trace!(
                "Stopping sandbox '{}' located in {} with pid {}",
                self.name,
                self.base.display(),
                pid
            );

            #[cfg(feature = "coverage")]
            if std::env::var_os("TEST_STOP_RACE").is_some() {
                // pre-kill the sandbox process to simulate a race or otherwise
                // inability to read the pid namespace file
                nix::sys::signal::kill(pid, nix::sys::signal::Signal::SIGKILL)?;
                // wait for the process to be killed
                while nix::sys::signal::kill(pid, None).is_ok() {
                    std::thread::sleep(std::time::Duration::from_millis(1));
                }
            }

            // Get our sandbox namespaces
            let sandbox_ns_str = format!("/proc/{}/ns/pid_for_children", pid);
            trace!("sandbox_ns_str: {}", sandbox_ns_str);
            let sandbox_ns_path = Path::new(&sandbox_ns_str);
            trace!("sandbox_ns_path: {}", sandbox_ns_path.display());

            if !sandbox_ns_path.exists() {
                trace!("Sandbox process {} already gone", pid);
            } else {
                let ns = readlink(sandbox_ns_path).context(format!(
                    "failed to readlink {}",
                    sandbox_ns_path.display()
                ))?;
                trace!("sandbox_ns: {}", ns.to_string_lossy());

                // Collect each process that shares a namespace with our sandbox
                for entry in std::fs::read_dir("/proc")
                    .context(format!("failed to read /proc"))?
                    .filter_map(Result::ok)
                {
                    let file_name = entry.file_name();
                    let pid_str = match file_name.to_str() {
                        Some(name)
                            if name.chars().all(|c| c.is_ascii_digit()) =>
                        {
                            name
                        }
                        _ => {
                            continue;
                        } // Skip non-PID entries
                    };

                    let process_pid = Pid::from_raw(pid_str.parse::<i32>()?);

                    // Check if this process belongs to our sandbox namespace
                    let proc_ns_str = format!("/proc/{}/ns/pid", pid_str);

                    #[cfg(feature = "coverage")]
                    let proc_ns_str =
                        if std::env::var_os("TEST_STOP_RACE3").is_some() {
                            String::from("/proc/does-not-exist")
                        } else {
                            proc_ns_str
                        };

                    let proc_ns_path = Path::new(&proc_ns_str);
                    if !proc_ns_path.exists() {
                        continue;
                    }

                    let proc_ns = readlink(proc_ns_path).context(format!(
                        "failed to readlink {}",
                        proc_ns_path.display()
                    ))?;

                    if proc_ns == ns {
                        trace!(
                            "[{}] proc_ns: {}",
                            process_pid,
                            proc_ns.to_string_lossy()
                        );

                        // read /proc/{}/cmdline
                        let command = std::fs::read_to_string(format!(
                            "/proc/{}/cmdline",
                            process_pid
                        ))
                        .context(format!(
                            "failed to read /proc/{}/cmdline",
                            process_pid
                        ))?;
                        trace!("command: {}", command);

                        trace!(
                            "[{}] Killing {} in sandbox '{}'",
                            process_pid, command, self.name
                        );

                        #[cfg(feature = "coverage")]
                        if std::env::var_os("TEST_STOP_RACE2").is_some() {
                            // pre-kill the sandbox process to simulate a race or otherwise
                            // inability to read the pid namespace file
                            nix::sys::signal::kill(
                                pid,
                                nix::sys::signal::Signal::SIGKILL,
                            )?;
                            // wait for the process to be killed
                            while nix::sys::signal::kill(pid, None).is_ok() {
                                std::thread::sleep(
                                    std::time::Duration::from_millis(1),
                                );
                            }
                        }

                        if let Err(e) = nix::sys::signal::kill(
                            process_pid,
                            nix::sys::signal::Signal::SIGKILL,
                        ) {
                            trace!(
                                "Failed to kill process {}: {}",
                                process_pid, e
                            );
                        }
                    }
                }
            }
        }

        let pid_file = PathBuf::from(format!("{}.pid", self.base.display()));
        trace!("Cleaning up PID file: {}", pid_file.display());
        if pid_file.exists() {
            std::fs::remove_file(&pid_file).context(format!(
                "failed to remove PID file {}",
                pid_file.display()
            ))?;
        }

        Ok(())
    }
}
