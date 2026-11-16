use std::path::PathBuf;

use serde::Serialize;

use crate::{distro::Distro, tools};

#[derive(Debug, Clone, Serialize)]
pub struct Check {
    pub tool: &'static str,
    pub purpose: &'static str,
    pub required: bool,
    pub path: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Report {
    pub distro: Distro,
    pub checks: Vec<Check>,
}

impl Check {
    pub fn found(&self) -> bool {
        self.path.is_some()
    }
}

impl Report {
    pub fn healthy(&self) -> bool {
        self.checks
            .iter()
            .all(|check| !check.required || check.found())
    }
}

pub fn run(distro: Distro) -> Report {
    let package_manager = distro
        .family
        .package_manager()
        .map(|manager| check(manager.binary(), "native package manager", true));
    let checks = package_manager
        .into_iter()
        .chain([
            check("sudo", "privilege escalation", false),
            check("pkexec", "graphical privilege escalation", false),
            check("alien", "convert between deb and rpm", false),
        ])
        .collect();
    Report { distro, checks }
}

fn check(tool: &'static str, purpose: &'static str, required: bool) -> Check {
    Check {
        tool,
        purpose,
        required,
        path: tools::find(tool),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::distro::Family;

    fn report(checks: Vec<Check>) -> Report {
        Report {
            distro: Distro {
                id: "test".into(),
                name: "Test".into(),
                family: Family::Unknown,
            },
            checks,
        }
    }

    fn check_with(required: bool, present: bool) -> Check {
        Check {
            tool: "x",
            purpose: "y",
            required,
            path: present.then(|| PathBuf::from("/bin/x")),
        }
    }

    #[test]
    fn missing_optional_tool_is_healthy() {
        assert!(report(vec![check_with(false, false)]).healthy());
    }

    #[test]
    fn missing_required_tool_is_unhealthy() {
        assert!(!report(vec![check_with(true, false), check_with(true, true)]).healthy());
    }

    #[test]
    fn unknown_family_has_no_package_manager_check() {
        let distro = Distro {
            id: "nixos".into(),
            name: "NixOS".into(),
            family: Family::Unknown,
        };
        assert!(run(distro).checks.iter().all(|check| !check.required));
    }
}
