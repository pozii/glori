use std::{
    env,
    path::{Path, PathBuf},
};

use crate::error::{Error, Result};

pub fn resolve(home: Option<PathBuf>, data_home: Option<PathBuf>) -> Result<Paths> {
    let home = home
        .filter(|path| path.is_absolute())
        .ok_or(Error::NoHome)?;
    let data_home = data_home
        .filter(|path| path.is_absolute())
        .unwrap_or_else(|| home.join(".local/share"));
    Ok(Paths::new(home, data_home))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paths {
    home: PathBuf,
    data_home: PathBuf,
}

impl Paths {
    pub fn new(home: PathBuf, data_home: PathBuf) -> Self {
        Paths { home, data_home }
    }

    pub fn from_env() -> Result<Self> {
        resolve(
            env::var_os("HOME").map(PathBuf::from),
            env::var_os("XDG_DATA_HOME").map(PathBuf::from),
        )
    }

    pub fn bin_dir(&self) -> PathBuf {
        self.home.join(".local/bin")
    }

    pub fn opt_dir(&self) -> PathBuf {
        self.home.join(".local/opt")
    }

    pub fn applications_dir(&self) -> PathBuf {
        self.data_home.join("applications")
    }

    pub fn registry_file(&self) -> PathBuf {
        self.data_home.join("glori/registry.json")
    }

    pub fn app_dir(&self, name: &str) -> PathBuf {
        self.opt_dir().join(name)
    }

    pub fn desktop_file(&self, name: &str) -> PathBuf {
        self.applications_dir().join(format!("{name}.desktop"))
    }

    pub fn manages(&self, path: &Path) -> bool {
        [self.opt_dir(), self.bin_dir(), self.applications_dir()]
            .iter()
            .any(|root| path.starts_with(root) && path != root)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths() -> Paths {
        Paths::new("/home/u".into(), "/home/u/.local/share".into())
    }

    #[test]
    fn standard_locations() {
        let paths = paths();
        assert_eq!(paths.bin_dir(), Path::new("/home/u/.local/bin"));
        assert_eq!(paths.app_dir("rg"), Path::new("/home/u/.local/opt/rg"));
        assert_eq!(
            paths.desktop_file("rg"),
            Path::new("/home/u/.local/share/applications/rg.desktop")
        );
        assert_eq!(
            paths.registry_file(),
            Path::new("/home/u/.local/share/glori/registry.json")
        );
    }

    #[test]
    fn only_managed_directories_are_manageable() {
        let paths = paths();
        assert!(paths.manages(Path::new("/home/u/.local/opt/rg")));
        assert!(paths.manages(Path::new("/home/u/.local/bin/rg")));
        assert!(!paths.manages(Path::new("/home/u/.local/bin")));
        assert!(!paths.manages(Path::new("/home/u/.ssh/id_rsa")));
        assert!(!paths.manages(Path::new("/etc/passwd")));
    }

    #[test]
    fn data_home_defaults_under_home() {
        let paths = resolve(Some("/home/u".into()), None).unwrap();
        assert_eq!(
            paths,
            Paths::new("/home/u".into(), "/home/u/.local/share".into())
        );
    }

    #[test]
    fn xdg_data_home_is_honoured() {
        let paths = resolve(Some("/home/u".into()), Some("/data".into())).unwrap();
        assert_eq!(
            paths.registry_file(),
            Path::new("/data/glori/registry.json")
        );
        assert_eq!(paths.bin_dir(), Path::new("/home/u/.local/bin"));
    }

    #[test]
    fn relative_or_missing_home_is_rejected() {
        assert!(matches!(resolve(None, None), Err(Error::NoHome)));
        assert!(matches!(
            resolve(Some("rel".into()), None),
            Err(Error::NoHome)
        ));
    }

    #[test]
    fn relative_data_home_is_ignored() {
        let paths = resolve(Some("/home/u".into()), Some("data".into())).unwrap();
        assert_eq!(
            paths.applications_dir(),
            Path::new("/home/u/.local/share/applications")
        );
    }
}
