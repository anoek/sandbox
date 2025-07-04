#![allow(
    clippy::collapsible_else_if,
    clippy::collapsible_if,
    clippy::module_inception,
    clippy::needless_range_loop,
    clippy::result_map_unit_fn,
    clippy::useless_format
)]
#![deny(
    clippy::get_unwrap,
    clippy::panic,
    clippy::print_stdout,
    clippy::unwrap_used,
    clippy::use_debug,
    clippy::used_underscore_binding,
    clippy::used_underscore_items
)]

mod actions;
mod config;
mod logger;
mod sandbox;
mod types;
mod util;

use anyhow::{Context, Result, anyhow};
use clap::CommandFactory;
use clap_complete::CompleteEnv;
use config::{cli, resolve_config};

use log::Log;
use nix::unistd::geteuid;
use sandbox::Sandbox;
use serde_json::Value;
use util::{
    drop_privileges, print_json_output, resolve_uid_gid_home, set_json_output,
    set_should_print_output,
};

use clap::Parser;
pub fn main() -> Result<()> {
    let logger = logger::SandboxLogger::new(log::LevelFilter::Trace)
        .init()
        .map_err(|e| anyhow!("Failed to initialize logger: {}", e))?;
    let cli: cli::Args = cli::Args::parse();

    if let Some(log_level) = cli.log_level {
        logger.set_level(log_level);
    } else {
        logger.set_level(log::LevelFilter::Info);
    };

    let config = resolve_config(cli.clone()).context("Resolving config")?;
    let uid_gid_home =
        resolve_uid_gid_home().context("Resolving uid/gid/home")?;

    // This is tab completion stuff. We put it after loading configs and parsing command line stuff
    // so that we can use the potential storage directory specified to complete the sandbox names.
    // This function will not return if there is tab completion requested.
    if std::env::var("COMPLETE").is_ok() {
        drop_privileges(uid_gid_home.uid, uid_gid_home.gid)?;
        CompleteEnv::with_factory(cli::Args::command).complete();
        return Ok(());
    }

    // Now that we've loaded the config, we can set the log level and print out any deferred messages
    // emitted while we were loading the config.
    logger.set_level(config.log_level);
    logger.print_deferred();

    set_should_print_output(!cli.json);

    // Ensure we're running as root
    let effective_uid = geteuid();
    if !effective_uid.is_root() {
        return Err(anyhow!(
            "Insufficient permissions to create the sandbox, please retry using `sudo` or setuid flags"
        ));
    }

    // Figure out who we really are
    let sandboxes_storage_dir = config.storage_dir.clone();

    let sandbox = Sandbox::from_location(
        &sandboxes_storage_dir,
        &config.name,
        uid_gid_home.uid,
        uid_gid_home.gid,
    );

    // Handle the action if one was specified
    if let Some(subcommand) = cli.action {
        let result = match subcommand {
            cli::Action::Config { keys } => actions::config(&config, keys),
            cli::Action::Sync => actions::sync(),
            cli::Action::List { patterns } => actions::list(
                &sandboxes_storage_dir,
                &patterns.unwrap_or_default(),
            ),
            cli::Action::Stop { all, patterns } => {
                if all {
                    actions::stop_all(
                        &sandboxes_storage_dir,
                        uid_gid_home.uid,
                        uid_gid_home.gid,
                        &["*".to_string()],
                    )
                } else if let Some(patterns) = patterns {
                    actions::stop_all(
                        &sandboxes_storage_dir,
                        uid_gid_home.uid,
                        uid_gid_home.gid,
                        &patterns,
                    )
                } else {
                    actions::stop(
                        &sandboxes_storage_dir,
                        &config.name,
                        uid_gid_home.uid,
                        uid_gid_home.gid,
                    )
                }
            }
            cli::Action::Status { patterns } => actions::status(
                &config,
                &sandbox,
                &patterns.unwrap_or_default(),
            ),
            cli::Action::Diff { patterns } => actions::diff(
                &config,
                cli.json,
                &sandbox,
                &patterns.unwrap_or_default(),
            ),
            cli::Action::Reject { patterns } => actions::reject(
                &config,
                &sandbox,
                &patterns.unwrap_or_default(),
            ),
            cli::Action::Accept { patterns } => actions::accept(
                &config,
                &sandbox,
                &patterns.unwrap_or_default(),
            ),
            cli::Action::Delete { yes, patterns } => actions::delete(
                &config,
                &sandboxes_storage_dir,
                &patterns.unwrap_or_default(),
                yes,
            ),
        };
        if cli.json {
            if result.is_ok() {
                set_json_output(
                    "status",
                    &Value::String("success".to_string()),
                );
            } else {
                set_json_output("status", &Value::String("error".to_string()));
                set_json_output(
                    "error",
                    &Value::String(
                        result
                            .as_ref()
                            .expect_err("Failed to get error")
                            .to_string(),
                    ),
                );
            }
            print_json_output()?;
            if result.is_err() {
                std::process::exit(1);
            }
        }
        logger.flush();
        return result;
    }

    //
    // If no subcommand was specified, we're running a command in the sandbox
    //
    let sandboxed_command = match cli.sandboxed_command {
        Some(sandboxed_command) => sandboxed_command,
        None => vec![std::env::var("SHELL").unwrap_or("sh".to_string())],
    };

    let sandbox =
        Sandbox::get_or_create(&config, uid_gid_home.uid, uid_gid_home.gid)
            .context("Getting or creating sandbox")?;

    // On success, we never actually return from this call
    sandbox.exec(&sandboxed_command)
}
