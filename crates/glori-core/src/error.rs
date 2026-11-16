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
    #[error("the HOME environment variable is not set")]
    NoHome,
    #[error("{0} is already installed; remove it first or choose another name with --name")]
    AlreadyInstalled(String),
    #[error("{0} was not installed by Glori")]
    NotInstalled(String),
    #[error("{0} already exists and Glori will not overwrite it")]
    Conflict(PathBuf),
    #[error("the archive contains an unsafe path: {0}")]
    UnsafeArchive(String),
    #[error("refusing to touch {0}: it is outside the directories Glori manages")]
    UnsafeRemoval(PathBuf),
    #[error("cannot derive a name from {0}; pass one with --name")]
    NoName(String),
    #[error("the archive is empty")]
    EmptyArchive,
    #[error("cannot derive a file name from the URL {0}")]
    NoFileName(String),
    #[error("download of {url} failed: {reason}")]
    Download { url: String, reason: String },
    #[error("{0}")]
    Update(String),
    #[error("the registry at {path} is corrupted: {message}")]
    Registry { path: PathBuf, message: String },
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
