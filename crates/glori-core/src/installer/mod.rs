mod appimage;
mod common;
mod native;
mod tarball;

use std::path::Path;

use crate::{
    distro::PackageManager,
    error::{Error, Result},
    format::Format,
    libs,
    operation::Operation,
    paths::Paths,
    plan::Strategy,
};

pub use native::install_packages;

pub struct Input<'a> {
    pub file: &'a Path,
    pub format: Format,
    pub name: Option<&'a str>,
    pub paths: &'a Paths,
    pub manager: Option<PackageManager>,
}

#[derive(Debug, Clone, Default)]
pub struct Plan {
    pub operations: Vec<Operation>,
    pub libraries: libs::Report,
}

impl Plan {
    pub fn of(operations: Vec<Operation>) -> Self {
        Plan {
            operations,
            libraries: libs::Report::default(),
        }
    }
}

pub trait Installer {
    fn plan(&self, input: &Input) -> Result<Plan>;
}

pub fn select(strategy: Strategy) -> Result<Box<dyn Installer>> {
    match strategy {
        Strategy::Native { manager } => Ok(Box::new(native::Native::new(manager))),
        Strategy::Convert => Err(Error::Unsupported(
            "installing a package from another distribution family",
        )),
        Strategy::Archive => Ok(Box::new(tarball::Tarball)),
        Strategy::AppImage => Ok(Box::new(appimage::AppImage)),
    }
}
