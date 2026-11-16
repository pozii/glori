use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

const FORMAT_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Archive,
    AppImage,
}

impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Kind::Archive => "archive",
            Kind::AppImage => "appimage",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub name: String,
    pub version: Option<String>,
    pub kind: Kind,
    pub installed_at: u64,
    pub source: String,
    pub files: Vec<PathBuf>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Registry {
    version: u32,
    entries: Vec<Entry>,
}

pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

impl Registry {
    pub fn load(path: &Path) -> Result<Self> {
        match fs::read_to_string(path) {
            Ok(content) => serde_json::from_str(&content).map_err(|error| Error::Registry {
                path: path.to_path_buf(),
                message: error.to_string(),
            }),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Registry::empty()),
            Err(error) => Err(Error::io(path)(error)),
        }
    }

    fn empty() -> Self {
        Registry {
            version: FORMAT_VERSION,
            entries: Vec::new(),
        }
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let parent = path.parent().unwrap_or(Path::new("."));
        fs::create_dir_all(parent).map_err(Error::io(parent))?;
        let temporary = path.with_extension("json.tmp");
        let content = serde_json::to_string_pretty(self).map_err(|error| Error::Registry {
            path: path.to_path_buf(),
            message: error.to_string(),
        })?;
        fs::write(&temporary, content).map_err(Error::io(&temporary))?;
        fs::rename(&temporary, path).map_err(Error::io(path))
    }

    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    pub fn find(&self, name: &str) -> Option<&Entry> {
        self.entries.iter().find(|entry| entry.name == name)
    }

    pub fn insert(&mut self, entry: Entry) -> Result<()> {
        if self.find(&entry.name).is_some() {
            return Err(Error::AlreadyInstalled(entry.name));
        }
        self.entries.push(entry);
        self.entries.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(())
    }

    pub fn remove(&mut self, name: &str) -> Option<Entry> {
        let index = self.entries.iter().position(|entry| entry.name == name)?;
        Some(self.entries.remove(index))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str) -> Entry {
        Entry {
            name: name.into(),
            version: Some("1.0".into()),
            kind: Kind::Archive,
            installed_at: 1,
            source: format!("{name}.tar.gz"),
            files: vec![PathBuf::from(format!("/h/.local/opt/{name}"))],
        }
    }

    #[test]
    fn missing_file_is_an_empty_registry() {
        let dir = tempfile::tempdir().unwrap();
        let registry = Registry::load(&dir.path().join("none.json")).unwrap();
        assert!(registry.entries().is_empty());
    }

    #[test]
    fn save_and_load_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested/registry.json");
        let mut registry = Registry::load(&path).unwrap();
        registry.insert(entry("b")).unwrap();
        registry.insert(entry("a")).unwrap();
        registry.save(&path).unwrap();
        let loaded = Registry::load(&path).unwrap();
        let names: Vec<_> = loaded.entries().iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["a", "b"]);
        assert_eq!(loaded.find("a"), Some(&entry("a")));
    }

    #[test]
    fn duplicate_names_are_rejected() {
        let mut registry = Registry::empty();
        registry.insert(entry("a")).unwrap();
        assert!(matches!(
            registry.insert(entry("a")),
            Err(Error::AlreadyInstalled(name)) if name == "a"
        ));
    }

    #[test]
    fn remove_returns_the_entry_once() {
        let mut registry = Registry::empty();
        registry.insert(entry("a")).unwrap();
        assert_eq!(registry.remove("a"), Some(entry("a")));
        assert_eq!(registry.remove("a"), None);
    }

    #[test]
    fn corrupted_file_is_reported_not_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("registry.json");
        fs::write(&path, "{ not json").unwrap();
        assert!(matches!(Registry::load(&path), Err(Error::Registry { .. })));
        assert_eq!(fs::read_to_string(&path).unwrap(), "{ not json");
    }

    #[test]
    fn save_leaves_no_temporary_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("registry.json");
        Registry::empty().save(&path).unwrap();
        assert!(!dir.path().join("registry.json.tmp").exists());
    }
}
