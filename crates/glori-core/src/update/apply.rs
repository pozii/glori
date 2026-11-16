use std::{
    fs::{self, File},
    io::{self, Read},
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::Command,
};

use crate::{
    archive,
    error::{Error, Result},
    format::Format,
};

const BINARY_NAME: &str = "glori";
const MAX_BINARY_BYTES: u64 = 256 * 1024 * 1024;
const SYSTEM_PREFIXES: [&str; 4] = ["/usr/", "/bin/", "/sbin/", "/opt/"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    SelfManaged,
    SystemPackage,
}

pub fn origin_of(exe: &Path) -> Origin {
    let managed = SYSTEM_PREFIXES
        .iter()
        .any(|prefix| exe.to_string_lossy().starts_with(prefix));
    if managed {
        Origin::SystemPackage
    } else {
        Origin::SelfManaged
    }
}

pub fn extract_binary(archive_file: &Path, destination: &Path) -> Result<()> {
    let reader = archive::open(archive_file, Format::TarGz)?;
    let mut tarball = tar::Archive::new(reader);
    let entries = tarball.entries().map_err(Error::io(archive_file))?;
    for entry in entries {
        let entry = entry.map_err(Error::io(archive_file))?;
        let is_binary = entry.header().entry_type().is_file()
            && entry
                .path()
                .is_ok_and(|path| path.file_name().is_some_and(|name| name == BINARY_NAME));
        if !is_binary {
            continue;
        }
        let mut output = File::create(destination).map_err(Error::io(destination))?;
        let copied = io::copy(&mut entry.take(MAX_BINARY_BYTES + 1), &mut output)
            .map_err(Error::io(destination))?;
        if copied > MAX_BINARY_BYTES {
            return Err(Error::Update(
                "the binary in the release is too large".into(),
            ));
        }
        return Ok(());
    }
    Err(Error::Update(format!(
        "{} contains no {BINARY_NAME} binary",
        archive_file.display()
    )))
}

pub fn replace(exe: &Path, new_binary: &Path, expected_version: &str) -> Result<()> {
    let staged = staging_path(exe)?;
    let result = stage(&staged, new_binary, expected_version)
        .and_then(|()| fs::rename(&staged, exe).map_err(Error::io(exe)));
    if result.is_err() {
        let _ = fs::remove_file(&staged);
    }
    result
}

fn stage(staged: &Path, new_binary: &Path, expected_version: &str) -> Result<()> {
    fs::copy(new_binary, staged).map_err(Error::io(staged))?;
    fs::set_permissions(staged, fs::Permissions::from_mode(0o755)).map_err(Error::io(staged))?;
    check_runs(staged, expected_version)
}

fn check_runs(binary: &Path, expected_version: &str) -> Result<()> {
    let output = Command::new(binary)
        .arg("--version")
        .output()
        .map_err(|error| Error::Update(format!("the downloaded binary does not run: {error}")))?;
    let printed = String::from_utf8_lossy(&output.stdout);
    if output.status.success() && printed.contains(expected_version) {
        return Ok(());
    }
    Err(Error::Update(format!(
        "the downloaded binary did not report version {expected_version}; the update was not applied"
    )))
}

fn staging_path(exe: &Path) -> Result<PathBuf> {
    let dir = exe
        .parent()
        .ok_or_else(|| Error::Update(format!("{} has no parent directory", exe.display())))?;
    Ok(dir.join(format!(".{BINARY_NAME}-update-{}", std::process::id())))
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    fn script(version: &str) -> Vec<u8> {
        format!("#!/bin/sh\necho glori {version}\n").into_bytes()
    }

    fn tarball(dir: &Path, entries: &[(&str, &[u8])]) -> PathBuf {
        let mut builder = tar::Builder::new(Vec::new());
        for (name, data) in entries {
            let mut header = tar::Header::new_gnu();
            header.set_size(data.len() as u64);
            header.set_mode(0o755);
            header.set_cksum();
            builder.append_data(&mut header, name, *data).unwrap();
        }
        let raw = builder.into_inner().unwrap();
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        encoder.write_all(&raw).unwrap();
        let path = dir.join("release.tar.gz");
        fs::write(&path, encoder.finish().unwrap()).unwrap();
        path
    }

    #[test]
    fn system_locations_are_left_to_the_package_manager() {
        for path in [
            "/usr/bin/glori",
            "/usr/local/bin/glori",
            "/bin/glori",
            "/opt/x/glori",
        ] {
            assert_eq!(origin_of(Path::new(path)), Origin::SystemPackage, "{path}");
        }
        for path in [
            "/home/a/.local/bin/glori",
            "/home/a/.cargo/bin/glori",
            "/tmp/glori",
        ] {
            assert_eq!(origin_of(Path::new(path)), Origin::SelfManaged, "{path}");
        }
    }

    #[test]
    fn extracts_the_binary_from_a_nested_directory() {
        let dir = tempfile::tempdir().unwrap();
        let archive = tarball(
            dir.path(),
            &[
                ("glori-0.2.0/README.md", b"docs"),
                ("glori-0.2.0/glori", b"BIN"),
            ],
        );
        let out = dir.path().join("out");
        extract_binary(&archive, &out).unwrap();
        assert_eq!(fs::read(&out).unwrap(), b"BIN");
    }

    #[test]
    fn an_archive_without_the_binary_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let archive = tarball(dir.path(), &[("README.md", b"docs"), ("glori.1", b"man")]);
        assert!(extract_binary(&archive, &dir.path().join("out")).is_err());
    }

    #[test]
    fn replaces_the_binary_in_place() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("glori");
        fs::write(&exe, script("0.1.0")).unwrap();
        let new = dir.path().join("new");
        fs::write(&new, script("0.2.0")).unwrap();
        replace(&exe, &new, "0.2.0").unwrap();
        assert_eq!(fs::read(&exe).unwrap(), script("0.2.0"));
        assert_eq!(
            fs::metadata(&exe).unwrap().permissions().mode() & 0o777,
            0o755
        );
        let leftovers = fs::read_dir(dir.path()).unwrap().count();
        assert_eq!(leftovers, 2);
    }

    #[test]
    fn a_binary_with_the_wrong_version_is_not_applied() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("glori");
        fs::write(&exe, script("0.1.0")).unwrap();
        let new = dir.path().join("new");
        fs::write(&new, script("0.9.9")).unwrap();
        assert!(replace(&exe, &new, "0.2.0").is_err());
        assert_eq!(fs::read(&exe).unwrap(), script("0.1.0"));
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 2);
    }

    #[test]
    fn a_binary_that_cannot_run_is_not_applied() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("glori");
        fs::write(&exe, script("0.1.0")).unwrap();
        let new = dir.path().join("new");
        fs::write(&new, b"not an executable").unwrap();
        assert!(replace(&exe, &new, "0.2.0").is_err());
        assert_eq!(fs::read(&exe).unwrap(), script("0.1.0"));
    }
}
