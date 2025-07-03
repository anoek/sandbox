#![allow(clippy::print_stdout)]
use anyhow::{Context, Result};
use colored::Colorize;
use colored::control::SHOULD_COLORIZE;
use log::trace;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::Command;

use crate::{
    config::Config,
    sandbox::{Sandbox, changes::EntryOperation},
};

pub fn diff(
    config: &Config,
    json: bool,
    sandbox: &Sandbox,
    patterns: &[String],
) -> Result<()> {
    trace!("Diffing sandbox {}", sandbox.name);

    if json {
        return Err(anyhow::anyhow!("JSON mode is not supported for diff"));
    }

    let cwd = std::env::current_dir()?;
    let all_changes = sandbox.changes(config)?;
    let changes = all_changes.matching(&cwd, patterns);

    // Exercise `EntryOperation::Error` handling during coverage builds.
    #[cfg(feature = "coverage")]
    let mut changes = changes.clone();
    #[cfg(feature = "coverage")]
    {
        use crate::sandbox::changes::ChangeError;

        changes.0.push(crate::sandbox::changes::ChangeEntry {
            operation: EntryOperation::Error(ChangeError::UnsupportedFileType),
            source: None,
            destination: PathBuf::from("/dev/null"),
            staged: None,
            tmp_path: None,
        });
    }

    let should_colorize = SHOULD_COLORIZE.should_colorize();
    let upper_cwd_str = config.upper_cwd.to_string_lossy();
    let replacement = format!("<{}>", sandbox.name).cyan().to_string();

    #[cfg(feature = "coverage")]
    let should_colorize = if std::env::var_os("TEST_FORCE_DIFF_COLOR").is_some()
    {
        true
    } else {
        should_colorize
    };

    for change in changes.iter() {
        let stdout = std::io::stdout();
        let mut stdout_lock = stdout.lock();

        trace!(
            "{:?} {} {}",
            change.operation,
            change.destination.display(),
            change
                .staged
                .as_ref()
                .map(|s| s.path.to_string_lossy())
                .unwrap_or_else(|| "/dev/null".into()),
        );
        match &change.operation {
            EntryOperation::Rename => {
                let source = change
                    .source
                    .as_ref()
                    .expect("Rename operation must include a source entry");
                let from = source.path.display();
                let to = change.destination.display();
                let moved_msg = format!("### Moved {from} to {to}");
                if should_colorize {
                    writeln!(stdout_lock, "{}", moved_msg.yellow())?;
                } else {
                    writeln!(stdout_lock, "{}", moved_msg)?;
                }
                continue; // Nothing further to diff
            }
            EntryOperation::Error(_) => continue,
            EntryOperation::Set(_) | EntryOperation::Remove => {
                if !change.destination.is_file() {
                    let staged = &change
                        .staged
                        .as_ref()
                        .expect("Set operation must include a staged entry");
                    if !staged.is_file() {
                        continue;
                    }
                }

                // Paths: destination vs staged/null
                let left_path: PathBuf = if change.destination.exists() {
                    change.destination.clone()
                } else {
                    PathBuf::from("/dev/null")
                };

                let right_path: PathBuf =
                    if let EntryOperation::Set(_) = &change.operation {
                        change
                            .staged
                            .as_ref()
                            .expect("Set operation must include a staged entry")
                            .path
                            .clone()
                    } else {
                        PathBuf::from("/dev/null")
                    };

                let output = Command::new("diff")
                    .arg("-uN")
                    .arg(if should_colorize {
                        "--color=always"
                    } else {
                        "--color=never"
                    })
                    .arg(&left_path)
                    .arg(&right_path)
                    .stdout(std::process::Stdio::piped())
                    .spawn()?;
                let diff_stdout =
                    output.stdout.context("Failed to capture stdout")?;
                let reader = BufReader::new(diff_stdout);

                for line in reader.lines() {
                    let line =
                        line.context("Failed to read line from diff output")?;
                    let processed_line =
                        line.replace(&*upper_cwd_str, &replacement);
                    writeln!(stdout_lock, "{}", processed_line)
                        .context("Failed to write diff output")?;
                }
            }
        }
    }

    Ok(())
}
