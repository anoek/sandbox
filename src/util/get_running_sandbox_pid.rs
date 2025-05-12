use nix::{sys::signal::kill, unistd::Pid};
use std::path::Path;
use std::path::PathBuf;

use log::trace;

/**
 * Get the PID of a running sandbox.
 *
 * Returns None if the sandbox is not running.
 */
pub fn get_running_sandbox_pid(
    sandboxes_storage_dir: &Path,
    sandbox_name: &str,
) -> Option<Pid> {
    let sandboxes_storage_dir = &PathBuf::from(sandboxes_storage_dir);
    let pid_file = get_sandbox_pid_path(sandboxes_storage_dir, sandbox_name);
    if pid_file.exists() {
        match std::fs::read_to_string(&pid_file)
            .ok()
            .and_then(|pid_str| pid_str.parse::<i32>().ok())
            .map(Pid::from_raw)
            .filter(|&pid| kill(pid, None).is_ok())
            .and_then(|pid| {
                trace!(
                    "Existing sandbox '{}' found with PID: {}",
                    sandbox_name, pid
                );

                std::fs::read_to_string(format!("/proc/{}/stat", pid.as_raw()))
                    .ok()
                    .map(|status| (pid, status))
            })
            .and_then(|(pid, status)| {
                let components: Vec<&str> = status.split_whitespace().collect();
                components
                    .get(2)
                    .filter(|&status| *status != "Z" && *status != "T")
                    .map(|_| pid)
            }) {
            Some(pid) => return Some(pid),
            None => {
                trace!(
                    "Invalid PID file or sandbox was not in a good state, cleaning up"
                );
                std::fs::remove_file(&pid_file).ok();
            }
        }
    }

    None
}

pub fn get_sandbox_pid_path(
    sandboxes_storage_dir: &Path,
    sandbox_name: &str,
) -> PathBuf {
    sandboxes_storage_dir.join(format!("{}.pid", sandbox_name))
}
