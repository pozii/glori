use std::{fs, path::Path};

use serde::Serialize;

use crate::error::{Error, Result};

const OS_RELEASE_PATHS: [&str; 2] = ["/etc/os-release", "/usr/lib/os-release"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Family {
    Debian,
    Fedora,
    Arch,
    Suse,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PackageManager {
    Apt,
    Dnf,
    Pacman,
    Zypper,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Distro {
    pub id: String,
    pub name: String,
    pub family: Family,
}

impl Family {
    pub fn label(self) -> &'static str {
        match self {
            Family::Debian => "Debian",
            Family::Fedora => "Fedora",
            Family::Arch => "Arch",
            Family::Suse => "SUSE",
            Family::Unknown => "Unknown",
        }
    }

    pub fn package_manager(self) -> Option<PackageManager> {
        match self {
            Family::Debian => Some(PackageManager::Apt),
            Family::Fedora => Some(PackageManager::Dnf),
            Family::Arch => Some(PackageManager::Pacman),
            Family::Suse => Some(PackageManager::Zypper),
            Family::Unknown => None,
        }
    }
}

impl PackageManager {
    pub fn binary(self) -> &'static str {
        match self {
            PackageManager::Apt => "apt",
            PackageManager::Dnf => "dnf",
            PackageManager::Pacman => "pacman",
            PackageManager::Zypper => "zypper",
        }
    }
}

pub fn detect() -> Result<Distro> {
    let path = OS_RELEASE_PATHS
        .iter()
        .map(Path::new)
        .find(|path| path.exists())
        .ok_or(Error::MissingOsRelease)?;
    let content = fs::read_to_string(path).map_err(Error::io(path))?;
    Ok(parse(&content))
}

pub fn parse(content: &str) -> Distro {
    let id = field(content, "ID").to_lowercase();
    let name = [field(content, "PRETTY_NAME"), field(content, "NAME"), &id]
        .into_iter()
        .find(|value| !value.is_empty())
        .unwrap_or_default()
        .to_string();
    let family = std::iter::once(id.as_str())
        .chain(field(content, "ID_LIKE").split_whitespace())
        .find_map(family_of)
        .unwrap_or(Family::Unknown);
    Distro { id, name, family }
}

fn field<'a>(content: &'a str, key: &str) -> &'a str {
    content
        .lines()
        .find_map(|line| line.strip_prefix(key)?.strip_prefix('='))
        .map(|value| value.trim().trim_matches('"'))
        .unwrap_or_default()
}

fn family_of(id: &str) -> Option<Family> {
    match id {
        "debian" | "ubuntu" => Some(Family::Debian),
        "fedora" | "rhel" | "centos" => Some(Family::Fedora),
        "arch" => Some(Family::Arch),
        "sles" => Some(Family::Suse),
        id if id.contains("suse") => Some(Family::Suse),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_fedora() {
        let distro = parse(
            "NAME=\"Fedora Linux\"\nID=fedora\nVERSION_ID=45\nPRETTY_NAME=\"Fedora Linux 45\"\n",
        );
        assert_eq!(distro.id, "fedora");
        assert_eq!(distro.name, "Fedora Linux 45");
        assert_eq!(distro.family, Family::Fedora);
    }

    #[test]
    fn derivative_resolves_through_id_like() {
        assert_eq!(
            parse("ID=linuxmint\nID_LIKE=\"ubuntu debian\"\n").family,
            Family::Debian
        );
        assert_eq!(parse("ID=manjaro\nID_LIKE=arch\n").family, Family::Arch);
        assert_eq!(
            parse("ID=rocky\nID_LIKE=\"rhel centos fedora\"\n").family,
            Family::Fedora
        );
    }

    #[test]
    fn suse_variants() {
        assert_eq!(
            parse("ID=opensuse-tumbleweed\nID_LIKE=\"opensuse suse\"\n").family,
            Family::Suse
        );
    }

    #[test]
    fn unknown_distro() {
        assert_eq!(parse("ID=nixos\n").family, Family::Unknown);
        assert_eq!(parse("").family, Family::Unknown);
    }

    #[test]
    fn id_like_does_not_shadow_id() {
        assert_eq!(parse("ID_LIKE=debian\nID=arch\n").id, "arch");
    }

    #[test]
    fn package_managers() {
        assert_eq!(Family::Fedora.package_manager(), Some(PackageManager::Dnf));
        assert_eq!(Family::Unknown.package_manager(), None);
    }
}
