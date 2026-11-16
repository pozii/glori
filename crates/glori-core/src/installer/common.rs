use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::{
    desktop,
    error::{Error, Result},
    naming::Identity,
    operation::Operation,
    paths::Paths,
    registry::{self, Entry, Kind, Registry},
};

pub fn canonical(file: &Path) -> Result<PathBuf> {
    fs::canonicalize(file).map_err(Error::io(file))
}

pub fn file_name(file: &Path) -> String {
    file.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

pub fn absent(path: &Path) -> Result<()> {
    if fs::symlink_metadata(path).is_ok() {
        return Err(Error::Conflict(path.to_path_buf()));
    }
    Ok(())
}

pub fn identify(file: &Path, name: Option<&str>) -> Result<Identity> {
    let file_name = file_name(file);
    let identity = crate::naming::identify(&file_name, name);
    if identity.name.is_empty() {
        return Err(Error::NoName(file_name));
    }
    Ok(identity)
}

pub fn ensure_free(paths: &Paths, name: &str) -> Result<()> {
    let registry = Registry::load(&paths.registry_file())?;
    if registry.find(name).is_some() {
        return Err(Error::AlreadyInstalled(name.to_string()));
    }
    absent(&paths.app_dir(name))
}

pub fn desktop_entry(
    paths: &Paths,
    identity: &Identity,
    exec: &Path,
    icon: Option<&Path>,
) -> Result<Operation> {
    let path = paths.desktop_file(&identity.name);
    absent(&path)?;
    Ok(Operation::Write {
        content: desktop::entry(&identity.display, exec, icon),
        path,
        mode: 0o644,
    })
}

pub fn register(identity: &Identity, kind: Kind, source: String, files: Vec<PathBuf>) -> Operation {
    Operation::Register(Entry {
        name: identity.name.clone(),
        version: identity.version.clone(),
        kind,
        installed_at: registry::now(),
        source,
        files,
    })
}
