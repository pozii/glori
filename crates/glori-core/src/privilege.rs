use std::{fs, os::unix::fs::MetadataExt};

use serde::Serialize;

use crate::{
    error::{Error, Result},
    tools,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Elevation {
    Direct,
    Sudo,
    Pkexec,
}

impl Elevation {
    pub fn prefix(self) -> &'static [&'static str] {
        match self {
            Elevation::Direct => &[],
            Elevation::Sudo => &["sudo"],
            Elevation::Pkexec => &["pkexec"],
        }
    }
}

pub fn detect() -> Result<Elevation> {
    choose(
        is_root(),
        tools::find("sudo").is_some(),
        tools::find("pkexec").is_some(),
    )
}

pub fn choose(root: bool, sudo: bool, pkexec: bool) -> Result<Elevation> {
    match (root, sudo, pkexec) {
        (true, _, _) => Ok(Elevation::Direct),
        (_, true, _) => Ok(Elevation::Sudo),
        (_, _, true) => Ok(Elevation::Pkexec),
        _ => Err(Error::NoPrivilegeTool),
    }
}

fn is_root() -> bool {
    fs::metadata("/proc/self")
        .map(|meta| meta.uid() == 0)
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_needs_no_tool() {
        assert_eq!(choose(true, false, false).unwrap(), Elevation::Direct);
    }

    #[test]
    fn sudo_is_preferred_over_pkexec() {
        assert_eq!(choose(false, true, true).unwrap(), Elevation::Sudo);
    }

    #[test]
    fn pkexec_is_the_fallback() {
        assert_eq!(choose(false, false, true).unwrap(), Elevation::Pkexec);
    }

    #[test]
    fn no_tool_is_an_error() {
        assert!(matches!(
            choose(false, false, false),
            Err(Error::NoPrivilegeTool)
        ));
    }

    #[test]
    fn prefixes() {
        assert!(Elevation::Direct.prefix().is_empty());
        assert_eq!(Elevation::Sudo.prefix(), ["sudo"]);
        assert_eq!(Elevation::Pkexec.prefix(), ["pkexec"]);
    }
}
