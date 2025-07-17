use super::impls::deserialize_level_filter;
use serde::Deserialize;
use std::{collections::HashMap, path::PathBuf};

#[derive(Deserialize, Debug, Clone, clap::ValueEnum)]
pub enum Network {
    #[serde(rename = "none")]
    None,
    #[serde(rename = "host")]
    Host,
}

#[derive(Deserialize, Default, Clone)]
pub struct PartialConfig {
    #[serde(deserialize_with = "deserialize_level_filter", default)]
    pub log_level: Option<log::LevelFilter>,
    pub name: Option<String>,
    pub storage_dir: Option<String>,
    pub net: Option<Network>,
    pub ignored: Option<bool>,
    #[serde(rename = "bind", default)]
    pub bind_mounts: Option<Vec<String>>,
    pub no_default_binds: Option<bool>,
}

#[derive(Clone)]
pub struct Config {
    pub log_level: log::LevelFilter,
    pub name: String,
    pub storage_dir: PathBuf,
    pub sandbox_dir: PathBuf,
    pub upper_cwd: PathBuf,
    pub overlay_cwd: PathBuf,
    pub net: Network,
    pub sources: HashMap<String, String>,
    pub ignored: bool,
    pub bind_mounts: Vec<String>,
    pub no_default_binds: bool,
}
