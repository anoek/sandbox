use super::structs::Network;

use serde::Deserialize;
use std::{
    fmt::{self, Display},
    str::FromStr,
};

impl FromStr for Network {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "none" => Ok(Network::None),
            "host" => Ok(Network::Host),
            _ => Err(format!("Invalid network type: {}", s)),
        }
    }
}

impl Display for Network {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Network::None => write!(f, "none"),
            Network::Host => write!(f, "host"),
        }
    }
}

pub(crate) fn deserialize_level_filter<'de, D>(
    deserializer: D,
) -> Result<Option<log::LevelFilter>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s: Option<String> = Option::deserialize(deserializer)?;
    s.map_or(Ok(None), |s| {
        log::LevelFilter::from_str(&s)
            .map(Some)
            .map_err(serde::de::Error::custom)
    })
}
