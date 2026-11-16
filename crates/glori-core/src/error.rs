use std::{
    io,
    path::{Path, PathBuf},
    process::ExitStatus,
};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("cannot read {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("unrecognized file format: {0}")]
    UnknownFormat(PathBuf),
    #[error("cannot detect the distribution: no os-release file found")]
    MissingOsRelease,
    #[error("{0} is not supported yet")]
    Unsupported(&'static str),
    #[error("path is not valid UTF-8: {0}")]
    NonUtf8Path(PathBuf),
    #[error("this step needs root, but neither sudo nor pkexec was found")]
    NoPrivilegeTool,
    #[error("cannot start `{command}`: {source}")]
    Spawn {
        command: String,
        #[source]
        source: io::Error,
    },
    #[error("`{command}` failed ({status})")]
    StepFailed { command: String, status: ExitStatus },
}

pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    pub fn io(path: &Path) -> impl FnOnce(io::Error) -> Error + '_ {
        move |source| Error::Io {
            path: path.to_path_buf(),
            source,
        }
    }
}
