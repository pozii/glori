use std::{cmp::Ordering, fmt, str::FromStr};

use serde::{Serialize, Serializer};

#[derive(Debug, Clone)]
pub struct Version {
    parts: [u64; 4],
    pre_release: Option<String>,
    text: String,
}

impl Version {
    pub fn current() -> Self {
        crate::VERSION.parse().expect("the build version is valid")
    }
}

impl FromStr for Version {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let trimmed = text.trim().trim_start_matches('v');
        let without_build = trimmed.split('+').next().unwrap_or(trimmed);
        let (core, pre_release) = match without_build.split_once('-') {
            Some((core, pre)) if !pre.is_empty() => (core, Some(pre.to_string())),
            Some(_) => return Err(format!("invalid version: {text}")),
            None => (without_build, None),
        };
        let numbers: Vec<u64> = core
            .split('.')
            .map(str::parse)
            .collect::<Result<_, _>>()
            .map_err(|_| format!("invalid version: {text}"))?;
        if !(3..=4).contains(&numbers.len()) {
            return Err(format!("invalid version: {text}"));
        }
        let mut parts = [0; 4];
        parts[..numbers.len()].copy_from_slice(&numbers);
        Ok(Version {
            parts,
            pre_release,
            text: trimmed.to_string(),
        })
    }
}

impl PartialEq for Version {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Version {}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        self.parts
            .cmp(&other.parts)
            .then_with(|| match (&self.pre_release, &other.pre_release) {
                (None, None) => Ordering::Equal,
                (None, Some(_)) => Ordering::Greater,
                (Some(_), None) => Ordering::Less,
                (Some(left), Some(right)) => left.cmp(right),
            })
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for Version {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.text)
    }
}

impl Serialize for Version {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(text: &str) -> Version {
        text.parse().unwrap()
    }

    #[test]
    fn parses_plain_and_prefixed_versions() {
        assert_eq!(v("1.2.3").to_string(), "1.2.3");
        assert_eq!(v("v1.2.3").to_string(), "1.2.3");
        assert_eq!(v(" v0.10.0\n").to_string(), "0.10.0");
        assert_eq!(v("2026.11.17-rc.1").to_string(), "2026.11.17-rc.1");
    }

    #[test]
    fn rejects_malformed_versions() {
        for text in [
            "",
            "1",
            "1.2",
            "1.2.3.4.5",
            "a.b.c",
            "1.2.x",
            "1.2.3-",
            "-1.2.3",
        ] {
            assert!(text.parse::<Version>().is_err(), "{text}");
        }
    }

    #[test]
    fn compares_numerically_not_textually() {
        assert!(v("0.10.0") > v("0.9.0"));
        assert!(v("1.0.0") > v("0.99.99"));
        assert!(v("1.2.10") > v("1.2.9"));
        assert_eq!(v("1.2.3"), v("v1.2.3"));
    }

    #[test]
    fn a_pre_release_sorts_before_its_release() {
        assert!(v("1.0.0-rc.1") < v("1.0.0"));
        assert!(v("1.0.0-rc.1") < v("1.0.0-rc.2"));
        assert!(v("0.9.9") < v("1.0.0-rc.1"));
    }

    #[test]
    fn calendar_versions_order_by_date_then_by_the_same_day_counter() {
        assert!(v("2026.11.17") < v("2026.11.17.1"));
        assert!(v("2026.11.17.1") < v("2026.11.17.2"));
        assert!(v("2026.11.17.9") < v("2026.11.17.10"));
        assert!(v("2026.11.17.2") < v("2026.11.18"));
        assert!(v("2026.11.30") < v("2026.12.01"));
        assert!(v("2026.12.31") < v("2027.01.01"));
    }

    #[test]
    fn leading_zeros_and_a_zero_counter_are_the_same_version() {
        assert_eq!(v("2026.01.05"), v("2026.1.5"));
        assert_eq!(v("2026.11.17.0"), v("2026.11.17"));
        assert_eq!(v("2026.01.05").to_string(), "2026.01.05");
        assert_eq!(v("v2026.11.17.1").to_string(), "2026.11.17.1");
    }

    #[test]
    fn a_calendar_pre_release_sorts_before_its_release() {
        assert!(v("2026.11.17-rc.1") < v("2026.11.17"));
        assert!(v("2026.11.16.3") < v("2026.11.17-rc.1"));
    }

    #[test]
    fn build_metadata_is_ignored() {
        assert_eq!(v("1.0.0+abc"), v("1.0.0"));
    }

    #[test]
    fn the_running_version_parses() {
        assert!(Version::current() >= v("0.0.0"));
    }
}
