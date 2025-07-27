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

#[cfg(test)]
mod tests {
    use super::super::structs::BindMountOptions;
    use std::str::FromStr;

    #[test]
    fn test_bind_mount_options_from_str() {
        assert!(matches!(
            BindMountOptions::from_str("").unwrap(),
            BindMountOptions::ReadWrite
        ));
        assert!(matches!(
            BindMountOptions::from_str("rw").unwrap(),
            BindMountOptions::ReadWrite
        ));
        assert!(matches!(
            BindMountOptions::from_str("ro").unwrap(),
            BindMountOptions::ReadOnly
        ));
        assert!(matches!(
            BindMountOptions::from_str("mask").unwrap(),
            BindMountOptions::Mask
        ));
        assert!(BindMountOptions::from_str("invalid").is_err());
    }
}
