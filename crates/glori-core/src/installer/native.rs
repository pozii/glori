use super::{Input, Installer, common};
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
    fn plan(&self, input: &Input) -> Result<Vec<Operation>> {
        let absolute = common::canonical(input.file)?;
        let path = absolute
            .to_str()
            .ok_or_else(|| Error::NonUtf8Path(absolute.clone()))?;
        Ok(vec![Operation::Run(install_step(self.manager, path))])
    }
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
    fn missing_file_is_an_io_error() {
        let paths = Paths::new("/h".into(), "/h/.local/share".into());
        let input = Input {
            file: Path::new("/nonexistent/a.rpm"),
            format: Format::Rpm,
            name: None,
            paths: &paths,
        };
        assert!(matches!(
            Native::new(PackageManager::Dnf).plan(&input),
            Err(Error::Io { .. })
        ));
    }
}
