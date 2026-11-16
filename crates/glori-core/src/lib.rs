pub mod archive;
pub mod desktop;
pub mod distro;
pub mod doctor;
pub mod download;
pub mod error;
pub mod extras;
pub mod format;
pub mod fsops;
pub mod installer;
pub mod libs;
pub mod naming;
pub mod operation;
pub mod paths;
pub mod plan;
pub mod privilege;
pub mod registry;
pub mod remove;
pub mod runner;
pub mod step;
pub mod tools;
pub mod update;
pub mod verify;
mod zstd_frames;

pub const VERSION: &str = match option_env!("GLORI_VERSION") {
    Some(version) => version,
    None => "0.0.0",
};

pub use distro::{Distro, Family, PackageManager};
pub use error::{Error, Result};
pub use format::Format;
pub use operation::Operation;
pub use paths::Paths;
pub use plan::Strategy;
pub use privilege::Elevation;
pub use step::Step;
pub use verify::{Trust, Verdict};
