use std::path::{Path, PathBuf};

use glori_core::download::{self, Download, Progress};

pub struct Source {
    path: PathBuf,
    _download: Option<Download>,
}

impl Source {
    pub fn path(&self) -> &Path {
        &self.path
    }
}

pub fn resolve(input: &str, has_checksum: bool, quiet: bool) -> anyhow::Result<Source> {
    if !download::is_url(input) {
        return Ok(Source {
            path: PathBuf::from(input),
            _download: None,
        });
    }
    let progress = if quiet {
        Progress::Hide
    } else {
        Progress::Show
    };
    let download = download::fetch(input, progress, has_checksum)?;
    Ok(Source {
        path: download.path().to_path_buf(),
        _download: Some(download),
    })
}
