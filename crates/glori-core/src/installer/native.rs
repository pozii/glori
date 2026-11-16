use super::{Input, Installer, Plan, common};
use crate::{
    distro::PackageManager,
    error::{Error, Result},
    operation::Operation,
    step::Step,
};

pub struct Native {
    manager: PackageManager,
}

impl Native {
    pub fn new(manager: PackageManager) -> Self {
        Native { manager }
    }
}

impl Installer for Native {
    fn plan(&self, input: &Input) -> Result<Plan> {
        let absolute = common::canonical(input.file)?;
        let path = absolute
            .to_str()
            .ok_or_else(|| Error::NonUtf8Path(absolute.clone()))?;
        Ok(Plan::of(vec![Operation::Run(install_step(
            self.manager,
            path,
        ))]))
    }
}

pub fn install_packages(manager: PackageManager, packages: &[String]) -> Step {
    let names: Vec<&str> = packages.iter().map(String::as_str).collect();
    let (program, mut args): (&str, Vec<&str>) = match manager {
        PackageManager::Apt => ("apt-get", vec!["install", "-y"]),
        PackageManager::Dnf => ("dnf", vec!["install", "-y"]),
        PackageManager::Pacman => ("pacman", vec!["-S", "--needed", "--noconfirm"]),
        PackageManager::Zypper => ("zypper", vec!["--non-interactive", "install"]),
    };
    args.extend(names);
    Step::new(program, &args, true)
}

fn install_step(manager: PackageManager, path: &str) -> Step {
    match manager {
        PackageManager::Apt => Step::new("apt-get", &["install", "-y", path], true),
        PackageManager::Dnf => Step::new("dnf", &["install", "-y", path], true),
        PackageManager::Pacman => Step::new("pacman", &["-U", "--noconfirm", path], true),
        PackageManager::Zypper => Step::new(
            "zypper",
            &["--non-interactive", "install", "--allow-unsigned-rpm", path],
            true,
        ),
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::{format::Format, paths::Paths};

    #[test]
    fn each_manager_installs_a_local_file() {
        let cases = [
            (PackageManager::Apt, "apt-get install -y /p/a.deb"),
            (PackageManager::Dnf, "dnf install -y /p/a.deb"),
            (PackageManager::Pacman, "pacman -U --noconfirm /p/a.deb"),
            (
                PackageManager::Zypper,
                "zypper --non-interactive install --allow-unsigned-rpm /p/a.deb",
            ),
        ];
        for (manager, expected) in cases {
            let step = install_step(manager, "/p/a.deb");
            assert!(step.privileged);
            assert_eq!(step.command_line(None), expected);
        }
    }

    #[test]
    fn dependency_packages_are_installed_by_name() {
        let packages = vec!["gtk2".to_string(), "libxcrypt-compat".to_string()];
        let cases = [
            (
                PackageManager::Apt,
                "apt-get install -y gtk2 libxcrypt-compat",
            ),
            (PackageManager::Dnf, "dnf install -y gtk2 libxcrypt-compat"),
            (
                PackageManager::Pacman,
                "pacman -S --needed --noconfirm gtk2 libxcrypt-compat",
            ),
            (
                PackageManager::Zypper,
                "zypper --non-interactive install gtk2 libxcrypt-compat",
            ),
        ];
        for (manager, expected) in cases {
            let step = install_packages(manager, &packages);
            assert!(step.privileged);
            assert_eq!(step.command_line(None), expected);
        }
    }

    #[test]
    fn missing_file_is_an_io_error() {
        let paths = Paths::new("/h".into(), "/h/.local/share".into());
        let input = Input {
            file: Path::new("/nonexistent/a.rpm"),
            format: Format::Rpm,
            name: None,
            paths: &paths,
            manager: None,
        };
        assert!(matches!(
            Native::new(PackageManager::Dnf).plan(&input),
            Err(Error::Io { .. })
        ));
    }
}
