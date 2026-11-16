mod appimage;
mod common;
mod native;
mod tarball;

use std::path::Path;

use crate::{
    error::{Error, Result},
    format::Format,
    operation::Operation,
    paths::Paths,
    plan::Strategy,
};

pub struct Input<'a> {
    pub file: &'a Path,
    pub format: Format,
    pub name: Option<&'a str>,
    pub paths: &'a Paths,
}

pub trait Installer {
    fn plan(&self, input: &Input) -> Result<Vec<Operation>>;
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
