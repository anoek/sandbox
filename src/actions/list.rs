use anyhow::Result;
use colored::*;
use fast_glob::glob_match;
use log::trace;
use nix::{sys::signal::kill, unistd::Pid};
use serde_json::Value;
use std::path::Path;

use crate::{
    outln,
    util::{get_sandbox_pid_path, set_json_output},
};

pub fn list(sandboxes_storage_dir: &Path, patterns: &[String]) -> Result<()> {
    trace!("Listing sandboxes");

    let mut running_sandboxes: Vec<String> = Vec::new();
    let mut stopped_sandboxes: Vec<String> = Vec::new();

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

            if patterns.is_empty()
                || patterns.iter().any(|pattern| {
                    let mut pattern = pattern.clone();
                    pattern = format!("*{pattern}*");
                    glob_match(&pattern, &sandbox_name)
                })
            {
                let pid_file =
                    get_sandbox_pid_path(sandboxes_storage_dir, &sandbox_name);
                if pid_file.exists() {
                    let pid = std::fs::read_to_string(pid_file)?;

                    match pid.parse::<i32>() {
                        Ok(pid) => {
                            let pid = Pid::from_raw(pid);
                            if kill(pid, None).is_ok() {
                                running_sandboxes.push(sandbox_name);
                            } else {
                                stopped_sandboxes.push(sandbox_name);
                            }
                        }
                        Err(_) => {
                            stopped_sandboxes.push(sandbox_name);
                        }
                    }
                } else {
                    stopped_sandboxes.push(sandbox_name);
                }
            }
        }
    }

    if !running_sandboxes.is_empty() {
        outln!("Running sandboxes:");
        for sandbox in &running_sandboxes {
            outln!("{}", sandbox);
        }
    }

    if !running_sandboxes.is_empty() && !stopped_sandboxes.is_empty() {
        outln!("");
    }

    if !stopped_sandboxes.is_empty() {
        outln!("{}", "Stopped sandboxes:".dimmed());
        for sandbox in &stopped_sandboxes {
            outln!("{}", sandbox.dimmed());
        }
    }

    set_json_output(
        "running_sandboxes",
        &Value::Array(
            running_sandboxes.into_iter().map(Value::String).collect(),
        ),
    );
    set_json_output(
        "stopped_sandboxes",
        &Value::Array(
            stopped_sandboxes.into_iter().map(Value::String).collect(),
        ),
    );

    Ok(())
}
