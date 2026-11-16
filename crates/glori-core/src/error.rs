use std::{
    io,
    path::{Path, PathBuf},
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
