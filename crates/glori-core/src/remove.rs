use crate::{
    error::{Error, Result},
    operation::Operation,
    paths::Paths,
    registry::Registry,
};

pub fn plan(name: &str, paths: &Paths) -> Result<Vec<Operation>> {
    let registry = Registry::load(&paths.registry_file())?;
    let entry = registry
        .find(name)
        .ok_or_else(|| Error::NotInstalled(name.to_string()))?;
    if let Some(outside) = entry.files.iter().find(|file| !paths.manages(file)) {
        return Err(Error::UnsafeRemoval(outside.clone()));
    }
    let mut operations: Vec<Operation> = entry
        .files
        .iter()
        .map(|path| Operation::Remove { path: path.clone() })
        .collect();
    operations.push(Operation::Unregister {
        name: name.to_string(),
    });
    Ok(operations)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::{Entry, Kind};

    fn setup(files: impl FnOnce(&Paths) -> Vec<std::path::PathBuf>) -> (tempfile::TempDir, Paths) {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path().to_path_buf(), dir.path().join(".local/share"));
        let files = files(&paths);
        let mut registry = Registry::load(&paths.registry_file()).unwrap();
        registry
            .insert(Entry {
                name: "a".into(),
                version: None,
                kind: Kind::Archive,
                installed_at: 1,
                source: "a.tar.gz".into(),
                files,
            })
            .unwrap();
        registry.save(&paths.registry_file()).unwrap();
        (dir, paths)
    }

    #[test]
    fn removal_plan_lists_files_then_forgets_the_entry() {
        let (_keep, paths) = setup(|paths| vec![paths.app_dir("a"), paths.bin_dir().join("a")]);
        let operations = plan("a", &paths).unwrap();
        assert_eq!(operations.len(), 3);
        assert_eq!(
            operations[0],
            Operation::Remove {
                path: paths.app_dir("a")
            }
        );
        assert_eq!(
            operations.last(),
            Some(&Operation::Unregister { name: "a".into() })
        );
    }

    #[test]
    fn unknown_names_are_reported() {
        let (_keep, paths) = setup(|_| vec![]);
        assert!(matches!(plan("zzz", &paths), Err(Error::NotInstalled(_))));
    }

    #[test]
    fn a_tampered_registry_cannot_delete_outside_managed_directories() {
        let (_keep, paths) = setup(|_| vec!["/etc/passwd".into()]);
        assert!(matches!(plan("a", &paths), Err(Error::UnsafeRemoval(_))));
    }
}
