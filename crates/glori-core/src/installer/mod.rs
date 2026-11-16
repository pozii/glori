mod native;

use std::path::Path;

use crate::{
    error::{Error, Result},
    plan::Strategy,
    step::Step,
};

pub trait Installer {
    fn steps(&self, file: &Path) -> Result<Vec<Step>>;
}

pub fn select(strategy: Strategy) -> Result<Box<dyn Installer>> {
    match strategy {
        Strategy::Native { manager } => Ok(Box::new(native::Native::new(manager))),
        Strategy::Convert => Err(Error::Unsupported(
            "installing a package from another distribution family",
        )),
        Strategy::Archive => Err(Error::Unsupported("installing tarball archives")),
        Strategy::AppImage => Err(Error::Unsupported("installing AppImages")),
    }
}
