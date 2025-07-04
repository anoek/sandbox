use crate::actions::list::get_sandboxes;
use crate::config::Config;
use crate::sandbox::Sandbox;
use crate::util::Lock;
use crate::{outln, util::set_json_output};
use anyhow::Result;
use colored::{Color, Colorize};
use log::trace;
use serde_json::Value;
use std::io::{self, Write};
use std::path::Path;

struct SandboxEntry {
    name: String,
    stopped: bool,
    sandbox: Sandbox,
    _lock: Box<Lock>,
}

impl SandboxEntry {
    pub fn new(
        sandboxes_storage_dir: &Path,
        sandbox_name: &str,
        stopped: bool,
    ) -> Result<Self> {
        let (sandbox, lock) = Sandbox::get(
            sandboxes_storage_dir,
            sandbox_name,
            nix::unistd::Uid::from_raw(0),
            nix::unistd::Gid::from_raw(0),
            None,
        )?;

        let sandbox = match sandbox {
            Some(sandbox) => sandbox,
            None => Sandbox::from_location(
                sandboxes_storage_dir,
                sandbox_name,
                nix::unistd::Uid::from_raw(0),
                nix::unistd::Gid::from_raw(0),
            ),
        };

        Ok(Self {
            name: sandbox_name.to_string(),
            stopped,
            sandbox,
            _lock: lock,
        })
    }
}

pub fn delete(
    config: &Config,
    sandboxes_storage_dir: &Path,
    patterns: &[String],
    force: bool,
) -> Result<()> {
    trace!("Preparing to delete sandboxes");

    let [running_sandboxes, stopped_sandboxes] = if patterns.is_empty() {
        if Sandbox::from_location(
            sandboxes_storage_dir,
            &config.name,
            nix::unistd::Uid::from_raw(0),
            nix::unistd::Gid::from_raw(0),
        )
        .exists()
        {
            // serves as a check to see if the sandbox is running or not
            if Sandbox::get(
                sandboxes_storage_dir,
                &config.name,
                nix::unistd::Uid::from_raw(0),
                nix::unistd::Gid::from_raw(0),
                None,
            )?
            .0
            .is_some()
            {
                [vec![config.name.clone()], vec![]]
            } else {
                [vec![], vec![config.name.clone()]]
            }
        } else {
            [vec![], vec![]]
        }
    } else {
        get_sandboxes(sandboxes_storage_dir, patterns)?
    };
    /*
    let mut sandbox_entries = Vec::new();
    for sandbox_name in running_sandboxes {
        sandbox_entries.push(SandboxEntry::new(
            sandboxes_storage_dir,
            sandbox_name.as_str(),
            false,
        )?);
    }
    for sandbox_name in stopped_sandboxes {
        sandbox_entries.push(SandboxEntry::new(
            sandboxes_storage_dir,
            sandbox_name.as_str(),
            true,
        )?);
    }
    */

    if stopped_sandboxes.is_empty() && running_sandboxes.is_empty() {
        if patterns.is_empty() {
            outln!(
                "No sandbox by the name of '{}' found to delete.",
                config.name
            );
        } else {
            outln!("No sandboxes found matching the specified patterns.");
        }
        return Ok(());
    }

    // Show sandboxes that will be deleted and ask for confirmation
    if !force {
        outln!("The following sandboxes will be deleted:");
        for (list, stopped) in
            [(&running_sandboxes, false), (&stopped_sandboxes, true)]
        {
            for name in list {
                let sandbox_entry = SandboxEntry::new(
                    sandboxes_storage_dir,
                    name.as_str(),
                    stopped,
                )?;

                let modifications =
                    sandbox_entry.sandbox.count_upper_entries(config)?;
                let color = if modifications.not_ignored > 0 {
                    Color::Red
                } else if !sandbox_entry.stopped {
                    Color::Green
                } else {
                    Color::White
                };

                outln!(
                    "  {} {} ({} entries, {} ignored)",
                    if sandbox_entry.stopped {
                        "stopped"
                    } else {
                        "running"
                    },
                    sandbox_entry.name.color(color),
                    modifications.not_ignored,
                    modifications.ignored
                );
            }
        }

        // Use eprint! for the prompt since print! is not allowed
        eprint!("\nAre you sure you want to delete these sandboxes? [y/N] ");
        let _ = io::stderr().flush();

        let mut response = String::new();
        io::stdin().read_line(&mut response)?;

        if !response.trim().eq_ignore_ascii_case("y") {
            outln!("Delete operation cancelled.");
            return Ok(());
        }
    }

    // Delete each sandbox
    let mut deleted_sandboxes = Vec::new();
    let mut errors = Vec::new();

    for (list, stopped) in
        [(&running_sandboxes, false), (&stopped_sandboxes, true)]
    {
        for name in list {
            let sandbox_entry = SandboxEntry::new(
                sandboxes_storage_dir,
                name.as_str(),
                stopped,
            )?;

            trace!("Deleting sandbox: {}", sandbox_entry.name);
            match sandbox_entry.sandbox.delete() {
                Ok(()) => {
                    outln!("Deleted sandbox: {}", sandbox_entry.name.green());
                    deleted_sandboxes
                        .push(Value::String(sandbox_entry.name.clone()));
                }
                Err(e) => {
                    outln!("Error deleting sandbox: {}", e);
                    errors.push(Value::String(e.to_string()));
                }
            }
        }
    }

    outln!("{} sandboxes deleted", deleted_sandboxes.len());

    // Set JSON output
    set_json_output("status", &Value::String("success".to_string()));
    set_json_output("deleted", &Value::Array(deleted_sandboxes));
    set_json_output("errors", &Value::Array(errors));

    Ok(())
}
