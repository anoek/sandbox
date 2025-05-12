#![allow(clippy::option_map_unit_fn)]
use crate::config::Config;
use crate::outln;
use crate::util::set_json_output;
use anyhow::Result;
use log::debug;
use serde_json::Value;
use std::collections::HashMap;

pub fn config(config: &Config, keys: Option<Vec<String>>) -> Result<()> {
    let keys = keys.unwrap_or_else(|| {
        [
            "name",
            "net",
            "log_level",
            "bind_fuse",
            "storage_dir",
            "sandbox_dir",
            "upper_cwd",
            "overlay_cwd",
        ]
        .map(String::from)
        .to_vec()
    });
    let multi_line = keys.len() > 1;

    let net_str = format!("{}", config.net);
    let bind_fuse_str = format!("{}", config.bind_fuse);
    for key in keys {
        let (key, value) = match key.as_str() {
            "storage_dir" | "storage-dir" => (
                "storage_dir",
                config.storage_dir.to_str().unwrap_or("<error>"),
            ),
            "sandbox_dir" | "sandbox-dir" => (
                "sandbox_dir",
                config.sandbox_dir.to_str().unwrap_or("<error>"),
            ),
            "upper_cwd" | "upper-cwd" => {
                ("upper_cwd", config.upper_cwd.to_str().unwrap_or("<error>"))
            }
            "overlay_cwd" | "overlay-cwd" => (
                "overlay_cwd",
                config.overlay_cwd.to_str().unwrap_or("<error>"),
            ),
            "net" => ("net", net_str.as_str()),
            "bind_fuse" => ("bind_fuse", bind_fuse_str.as_str()),
            "name" => ("name", config.name.as_str()),
            "log_level" => ("log_level", config.log_level.as_str()),
            _ => {
                return Err(anyhow::anyhow!("Unknown key: {}", key));
            }
        };
        print_config_line(key, value, multi_line, &config.sources);
    }

    Ok(())
}

fn print_config_line(
    key: &str,
    value: &str,
    multi_line: bool,
    sources: &HashMap<String, String>,
) {
    sources.get(key).map(|s| {
        debug!("{}={} set from {}", key, value, s);
    });
    set_json_output(key, &Value::String(value.to_string()));

    if multi_line {
        outln!("{}={}", key, value);
    } else {
        outln!("{}", value);
    }
}
