use serde::Serialize;

use crate::{
    distro::{Distro, PackageManager},
    format::Format,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Strategy {
    Native { manager: PackageManager },
    Convert,
    Archive,
    AppImage,
}

pub fn plan(format: Format, distro: &Distro) -> Strategy {
    match format {
        Format::AppImage => Strategy::AppImage,
        Format::TarGz | Format::TarXz | Format::TarZst | Format::TarBz2 => Strategy::Archive,
        _ => match distro.family.package_manager() {
            Some(manager) if format.is_native_to(distro.family) => Strategy::Native { manager },
            _ => Strategy::Convert,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::distro::{self, Family};

    fn distro_of(family: Family) -> Distro {
        Distro {
            id: "test".into(),
            name: "Test".into(),
            family,
        }
    }

    #[test]
    fn native_formats_use_the_package_manager() {
        assert_eq!(
            plan(Format::Deb, &distro_of(Family::Debian)),
            Strategy::Native {
                manager: PackageManager::Apt
            }
        );
        assert_eq!(
            plan(Format::Rpm, &distro_of(Family::Fedora)),
            Strategy::Native {
                manager: PackageManager::Dnf
            }
        );
        assert_eq!(
            plan(Format::ArchPackage, &distro_of(Family::Arch)),
            Strategy::Native {
                manager: PackageManager::Pacman
            }
        );
    }

    #[test]
    fn foreign_packages_need_conversion() {
        assert_eq!(
            plan(Format::Deb, &distro_of(Family::Fedora)),
            Strategy::Convert
        );
        assert_eq!(
            plan(Format::Rpm, &distro_of(Family::Arch)),
            Strategy::Convert
        );
        assert_eq!(
            plan(Format::Deb, &distro_of(Family::Unknown)),
            Strategy::Convert
        );
    }

    #[test]
    fn archives_and_appimages_are_distro_independent() {
        let fedora = distro::parse("ID=fedora\n");
        assert_eq!(plan(Format::TarZst, &fedora), Strategy::Archive);
        assert_eq!(plan(Format::AppImage, &fedora), Strategy::AppImage);
    }
}
