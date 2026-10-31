use anyhow::{Result, anyhow};
use serde::Deserialize;

/// The smallest CPU limit cgroups will accept: a 1ms quota per 100ms period
const MIN_CPUS: f64 = 0.01;

/// Number of CPUs this process may run on. This matches `nproc` unless we
/// are already running under a CPU quota, in which case the quota is used.
pub fn available_cpus() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
}

/// Parses a CPU limit. Positive values are a number of CPUs and may be
/// fractional. Negative values mean all but that many of the `available`
/// CPUs, so -2 on a 32 CPU machine means 30.
pub fn parse_cpus(spec: &str, available: usize) -> Result<f64> {
    let invalid = || {
        anyhow!(
            "Invalid cpus value '{}': expected a number of CPUs such as 2 or 1.5, or a negative number such as -2 to use all but that many CPUs",
            spec
        )
    };

    let value: f64 = spec.trim().parse().map_err(|_| invalid())?;
    if !value.is_finite() || value == 0.0 {
        return Err(invalid());
    }

    let cpus = if value < 0.0 {
        available as f64 + value
    } else {
        value
    };

    if cpus < MIN_CPUS {
        return Err(anyhow!(
            "Invalid cpus value '{}': this leaves {} of the {} available CPUs for the sandbox, the minimum is {}",
            spec,
            cpus.max(0.0),
            available,
            MIN_CPUS
        ));
    }

    Ok(cpus)
}

/// Parses a memory size such as 512M, 4G, 1.5GiB or a plain number of bytes.
/// Suffixes are binary (K = 1024) and case insensitive.
pub fn parse_memory(spec: &str) -> Result<u64> {
    let invalid = || {
        anyhow!(
            "Invalid memory value '{}': expected a size such as 512M or 4G",
            spec
        )
    };

    let trimmed = spec.trim();
    let split = trimmed
        .find(|c: char| c.is_ascii_alphabetic())
        .unwrap_or(trimmed.len());
    let (number, suffix) = trimmed.split_at(split);

    let multiplier: u64 = match suffix.to_ascii_lowercase().as_str() {
        "" | "b" => 1,
        "k" | "kb" | "kib" => 1 << 10,
        "m" | "mb" | "mib" => 1 << 20,
        "g" | "gb" | "gib" => 1 << 30,
        "t" | "tb" | "tib" => 1 << 40,
        _ => return Err(invalid()),
    };

    let number: f64 = number.trim().parse().map_err(|_| invalid())?;
    let bytes = number * multiplier as f64;
    if !bytes.is_finite() || bytes < 1.0 || bytes > u64::MAX as f64 {
        return Err(invalid());
    }

    Ok(bytes as u64)
}

/// Lets config files specify limits as either numbers or strings, e.g.
/// `cpus = -2` or `memory = "4G"`
pub(crate) fn deserialize_number_or_string<'de, D>(
    deserializer: D,
) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum NumberOrString {
        Integer(i64),
        Float(f64),
        String(String),
    }

    Ok(
        Option::<NumberOrString>::deserialize(deserializer)?.map(|value| {
            match value {
                NumberOrString::Integer(i) => i.to_string(),
                NumberOrString::Float(f) => f.to_string(),
                NumberOrString::String(s) => s,
            }
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_cpus() {
        assert_eq!(parse_cpus("2", 32).unwrap(), 2.0);
        assert_eq!(parse_cpus(" 1.5 ", 32).unwrap(), 1.5);
        assert_eq!(parse_cpus("0.01", 32).unwrap(), 0.01);
        assert_eq!(parse_cpus("64", 32).unwrap(), 64.0);
        assert_eq!(parse_cpus("-2", 32).unwrap(), 30.0);
        assert_eq!(parse_cpus("-0.5", 4).unwrap(), 3.5);
        assert_eq!(parse_cpus("-31", 32).unwrap(), 1.0);
    }

    #[test]
    fn test_parse_cpus_invalid() {
        for spec in ["", "abc", "0", "-0", "nan", "inf", "-inf", "2 cpus"] {
            let err = parse_cpus(spec, 32).unwrap_err().to_string();
            assert!(err.contains("expected a number of CPUs"), "{}", err);
        }

        let err = parse_cpus("-32", 32).unwrap_err().to_string();
        assert!(err.contains("leaves 0 of the 32 available CPUs"), "{}", err);
        let err = parse_cpus("-40", 32).unwrap_err().to_string();
        assert!(err.contains("leaves 0 of the 32 available CPUs"), "{}", err);
        let err = parse_cpus("0.001", 32).unwrap_err().to_string();
        assert!(err.contains("the minimum is 0.01"), "{}", err);
    }

    #[test]
    fn test_parse_memory() {
        assert_eq!(parse_memory("1024").unwrap(), 1024);
        assert_eq!(parse_memory("1024b").unwrap(), 1024);
        assert_eq!(parse_memory("4k").unwrap(), 4096);
        assert_eq!(parse_memory("4KiB").unwrap(), 4096);
        assert_eq!(parse_memory("512M").unwrap(), 512 << 20);
        assert_eq!(parse_memory("512mb").unwrap(), 512 << 20);
        assert_eq!(parse_memory(" 4G ").unwrap(), 4 << 30);
        assert_eq!(parse_memory("4 GiB").unwrap(), 4 << 30);
        assert_eq!(parse_memory("1.5G").unwrap(), 3 << 29);
        assert_eq!(parse_memory("2T").unwrap(), 2 << 40);
    }

    #[test]
    fn test_parse_memory_invalid() {
        for spec in [
            "", "G", "abc", "0", "0.5", "-1G", "4X", "4GBs", "1e30T", "inf",
        ] {
            let err = parse_memory(spec).unwrap_err().to_string();
            assert!(err.contains("expected a size"), "{}: {}", spec, err);
        }
    }

    #[test]
    fn test_deserialize_number_or_string() {
        #[derive(Deserialize)]
        struct Limits {
            #[serde(
                deserialize_with = "deserialize_number_or_string",
                default
            )]
            value: Option<String>,
        }

        let parse = |s: &str| toml::from_str::<Limits>(s).unwrap().value;
        assert_eq!(parse("value = -2"), Some("-2".to_string()));
        assert_eq!(parse("value = 1.5"), Some("1.5".to_string()));
        assert_eq!(parse("value = \"4G\""), Some("4G".to_string()));
        assert_eq!(parse(""), None);
        assert!(toml::from_str::<Limits>("value = true").is_err());
    }
}
