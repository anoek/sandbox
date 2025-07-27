use super::impls::deserialize_level_filter;
use anyhow::Result;
use serde::Deserialize;
use std::{collections::HashMap, path::PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BindMount {
    pub source: PathBuf,
    pub target: PathBuf,
    pub options: BindMountOptions,
    pub argument: String,
}

impl std::fmt::Display for BindMount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.argument)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum BindMountOptions {
    ReadWrite,
    ReadOnly,
    Mask,
}

impl std::str::FromStr for BindMountOptions {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "" | "rw" => Ok(BindMountOptions::ReadWrite),
            "ro" => Ok(BindMountOptions::ReadOnly),
            "mask" => Ok(BindMountOptions::Mask),
            _ => Err(anyhow::anyhow!(
                "Unknown bind mount option: '{}'. Valid options are: rw, ro, mask",
                s
            )),
        }
    }
}

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
    pub bind_mounts: Vec<BindMount>,
    pub no_default_binds: bool,
}
