use super::{Network, resolve_config};
use crate::util::{drop_privileges, resolve_uid_gid_home};
use clap::Parser;
use clap_complete::engine::{ArgValueCompleter, CompletionCandidate};
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Parser, Clone, Debug)]
#[command(version, about, long_about = None,
    override_usage = "\n    sandbox [OPTIONS] [ACTION] [ACTION_ARGUMENTS...]\n    sandbox [OPTIONS] <SANDBOXED_COMMAND ...>")]
pub struct Args {
    /********************/
    /* Flags and settings */
    /********************/
    /// Set the log level to one of trace, debug, info, warn, or error.
    /// `-v` is shorthand for enabling verbose (trace) logging.
    #[arg(short = 'v',
        long,
        default_missing_value = "trace", 
        num_args = 0..=1,
        require_equals = true,
        value_parser = parse_log_level
    )]
    pub log_level: Option<log::LevelFilter>,

    /// Name of the sandbox, defaults to "sandbox"
    #[arg(long, value_hint = clap::ValueHint::Other, add = ArgValueCompleter::new(sandbox_name_completion))]
    pub name: Option<String>,

    /// Base storage directory for all sandboxes. Defaults to `~/.sandboxes/`
    #[arg(long)]
    pub storage_dir: Option<String>,

    /// Enable host (or other) network. Defaults to `none`, which disables network access. If you
    /// want to enable network access by default you can store net="host" in a config file.
    #[arg(
        long,
        num_args = 0..=1,
        default_missing_value = "host",
        require_equals = true,
        value_enum
        )]
    pub net: Option<Network>,

    /// Specify if you want to bind /dev/fuse within the sandbox. Disabling this will prevent
    /// appimages and FUSE-based filesystems from working, but may be considered more secure
    /// depending on your situation.
    #[arg(
        long,
        action = clap::ArgAction::Set,
        value_parser = clap::value_parser!(bool),
        default_missing_value = "true",
        num_args = 0..=1,
        require_equals = true
    )]
    pub bind_fuse: Option<bool>,

    /// Formats action output as a JSON blob. Does nothing for sandboxed commands.
    #[arg(long, action = clap::ArgAction::SetTrue)]
    pub json: bool,

    /// Do not load config files.
    #[arg(long, action = clap::ArgAction::SetTrue)]
    pub no_config: bool,

    /// Include files that would normally be filtered out by ignore rules.
    #[arg(long, action = clap::ArgAction::SetTrue)]
    pub ignored: bool,

    /***************/
    /* Subcommands */
    /***************/
    #[command(subcommand)]
    pub action: Option<Action>,

    /*********************/
    /* Sandboxed Command */
    /*********************/
    /// The command to run in the sandbox. If no command is provided, the current shell will be used.
    /// If the sandbox is not running, it will be started with the command.
    #[arg(
        trailing_var_arg = true,
        allow_hyphen_values = true,
        num_args = 0..,
        value_parser = validate_command,
        help_heading = "Sandboxed Command",
    )]
    pub sandboxed_command: Option<Vec<String>>,
}

#[derive(clap::Subcommand, Clone, Debug)]
#[command(subcommand_help_heading = "Actions")]
pub enum Action {
    /// Get current configuration options
    Config {
        /// The keys to get from the configuration
        #[arg(value_name = "KEYS", num_args = 0..)]
        keys: Option<Vec<String>>,
    },

    /// List running sandboxes matching these patterns (defaults to all)
    List {
        /// Patterns of sandboxes to list
        #[arg(value_name = "PATTERNS", num_args = 0.., flatten = true)]
        patterns: Option<Vec<String>>,
    },

    /// Show status of the sandbox matching the patterns in the current directory, or specified
    /// patterns. Use `status /` to show status of all files in the sandbox.
    Status {
        /// Patterns of files to show
        #[arg(value_name = "PATTERNS", num_args = 0..)]
        patterns: Option<Vec<String>>,
    },

    /// Show changes in the sandbox relative to the current changes
    Diff,

    /// Discard changes in the sandbox
    Discard {
        #[arg(value_name = "PATTERNS", num_args = 0..)]
        patterns: Option<Vec<String>>,
    },

    /// Accept changes in the sandbox
    Accept {
        #[arg(value_name = "PATTERNS", num_args = 0..)]
        patterns: Option<Vec<String>>,
    },

    /// Synchronize changes that might have occurred in your host
    /// file system so that they are reflected in running sandboxes.
    Sync,

    /// Kill all processes in the sandbox and unmount the filesystems.
    /// Note this will not discard any changes.
    Stop {
        /// Kill all sandboxes
        #[arg(long)]
        all: bool,

        /// Patterns of sandboxes to stop
        #[arg(value_name = "PATTERNS", num_args = 0.., conflicts_with = "all")]
        patterns: Option<Vec<String>>,
    },
}

pub fn sandbox_name_completion(
    current: &std::ffi::OsStr,
) -> Vec<CompletionCandidate> {
    let ugh = resolve_uid_gid_home();
    match ugh {
        Ok(ugh) => match drop_privileges(ugh.uid, ugh.gid) {
            Ok(_) => (),
            Err(_) => return vec![],
        },
        Err(_) => return vec![],
    }

    let mut completions = vec![];
    let Some(current) = current.to_str() else {
        return completions;
    };

    let cli: Args = Args::parse();
    let config = match resolve_config(cli.clone()) {
        Ok(config) => config,
        Err(_) => return completions,
    };

    let sandboxes_storage_dir = config.storage_dir;

    let entries = match std::fs::read_dir(sandboxes_storage_dir) {
        Ok(entries) => entries,
        Err(_) => return completions,
    };

    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => continue,
        };

        let file_name = entry.file_name();
        let file_name = match file_name.to_str() {
            Some(file_name) => file_name,
            None => continue,
        };

        if !file_name.starts_with(current) {
            continue;
        }

        let path = entry.path();
        if path.is_dir()
            && path.join("upper").is_dir()
            && path.join("work").is_dir()
            && path.join("overlay").is_dir()
        {
            completions.push(CompletionCandidate::new(file_name));
        }
    }

    completions
}

static ARG_COUNT: AtomicUsize = AtomicUsize::new(0);

// Because of the way clap works, if someone tries to pass a parameter that doesn't exist, we'll
// see it here as a command. This is a bit of a hack to catch that case.
fn validate_command(s: &str) -> Result<String, String> {
    ARG_COUNT.fetch_add(1, Ordering::Relaxed);
    if ARG_COUNT.load(Ordering::Relaxed) == 1 {
        if s.starts_with('-') && s != "--" {
            Err(String::from("Unknown option"))
        } else {
            Ok(s.to_string())
        }
    } else {
        Ok(s.to_string())
    }
}

fn parse_log_level(s: &str) -> Result<log::LevelFilter, String> {
    s.parse::<log::LevelFilter>().map_err(|e| e.to_string())
}
