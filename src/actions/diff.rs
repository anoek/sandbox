#![allow(clippy::print_stdout)]
use anyhow::{Context, Result};
use colored::Colorize;
use colored::control::SHOULD_COLORIZE;
use log::trace;
use std::io::{BufRead, BufReader, Write};
use std::process::Command;

use crate::{config::Config, sandbox::Sandbox};

pub fn diff(config: &Config, json: bool, sandbox: &Sandbox) -> Result<()> {
    trace!("Diffing sandbox {}", sandbox.name);

    if json {
        return Err(anyhow::anyhow!("JSON mode is not supported for diff"));
    }

    let should_colorize = SHOULD_COLORIZE.should_colorize();
    let cwd = std::env::current_dir()?;
    let overlay_path = config.overlay_cwd.to_string_lossy();
    let replacement = format!("<{}>", sandbox.name).cyan().to_string();

    #[cfg(feature = "coverage")]
    let should_colorize = if std::env::var_os("TEST_FORCE_DIFF_COLOR").is_some()
    {
        true
    } else {
        should_colorize
    };

    let output = Command::new("diff")
        .arg("-ruN")
        .arg(if should_colorize {
            "--color=always"
        } else {
            "--color=never"
        })
        .arg(".")
        .arg(config.overlay_cwd.clone())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .context(format!(
            "failed to diff {} {}",
            cwd.display(),
            config.overlay_cwd.display(),
        ))?;

    let stdout = output.stdout.context("Failed to capture stdout")?;
    let reader = BufReader::new(stdout);
    let stdout = std::io::stdout();
    let mut stdout_lock = stdout.lock();

    for line in reader.lines() {
        let line = line.context("Failed to read line from diff output")?;
        let processed_line = line.replace(&*overlay_path, &replacement);
        writeln!(stdout_lock, "{}", processed_line)
            .context("Failed to write to stdout")?;
    }

    Ok(())
}
