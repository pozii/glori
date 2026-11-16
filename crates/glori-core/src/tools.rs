use std::{
    env,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};

pub fn find(name: &str) -> Option<PathBuf> {
    let search_path = env::var_os("PATH")?;
    find_in(name, &search_path)
}

pub fn find_in(name: &str, search_path: &std::ffi::OsStr) -> Option<PathBuf> {
    env::split_paths(search_path)
        .map(|dir| dir.join(name))
        .find(|candidate| is_executable(candidate))
}

fn is_executable(path: &Path) -> bool {
    path.metadata()
        .map(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_a_standard_tool() {
        assert!(find("sh").is_some());
    }

    #[test]
    fn missing_tool_is_none() {
        assert!(find("glori-definitely-not-a-tool").is_none());
    }

    #[test]
    fn empty_search_path_finds_nothing() {
        assert!(find_in("sh", std::ffi::OsStr::new("")).is_none());
    }
}
