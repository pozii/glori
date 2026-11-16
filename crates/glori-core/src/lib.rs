pub mod distro;
pub mod doctor;
pub mod error;
pub mod format;
pub mod installer;
pub mod plan;
pub mod privilege;
pub mod runner;
pub mod step;
pub mod tools;

pub use distro::{Distro, Family, PackageManager};
pub use error::{Error, Result};
pub use format::Format;
pub use plan::Strategy;
pub use privilege::Elevation;
pub use step::Step;
