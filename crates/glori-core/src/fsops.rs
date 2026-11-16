use std::{
    fs,
    os::unix::fs::{PermissionsExt, symlink},
    path::{Path, PathBuf},
};

use crate::{
    archive,
    error::{Error, Result},
    format::Format,
    paths::Paths,
};

pub fn extract(archive_file: &Path, format: Format, dest: &Path, flatten: bool) -> Result<()> {
    ensure_absent(dest)?;
    let parent = dest.parent().unwrap_or(Path::new("."));
    fs::create_dir_all(parent).map_err(Error::io(parent))?;
    let staging = staging_dir(dest);
    remove_tree(&staging)?;
    fs::create_dir(&staging).map_err(Error::io(&staging))?;
    let outcome =
        unpack(archive_file, format, &staging).and_then(|()| place(&staging, dest, flatten));
    if outcome.is_err() {
        let _ = fs::remove_dir_all(&staging);
    }
    outcome
}

fn staging_dir(dest: &Path) -> PathBuf {
    let name = dest.file_name().unwrap_or_default().to_string_lossy();
    dest.with_file_name(format!(".{name}.glori-tmp"))
}

fn unpack(archive_file: &Path, format: Format, staging: &Path) -> Result<()> {
    let mut archive = tar::Archive::new(archive::open(archive_file, format)?);
    archive.set_preserve_permissions(true);
    archive.set_unpack_xattrs(false);
    for entry in archive.entries().map_err(Error::io(archive_file))? {
        let mut entry = entry.map_err(Error::io(archive_file))?;
        let name = entry.path().map_err(Error::io(archive_file))?.into_owned();
        let unpacked = entry.unpack_in(staging).map_err(Error::io(archive_file))?;
        if !unpacked {
            return Err(Error::UnsafeArchive(name.display().to_string()));
        }
    }
    Ok(())
}

fn place(staging: &Path, dest: &Path, flatten: bool) -> Result<()> {
    let source = if flatten {
        only_child_directory(staging)?
    } else {
        staging.to_path_buf()
    };
    fs::rename(&source, dest).map_err(Error::io(dest))?;
    if flatten {
        fs::remove_dir_all(staging).map_err(Error::io(staging))?;
    }
    Ok(())
}

fn only_child_directory(staging: &Path) -> Result<PathBuf> {
    let mut children = fs::read_dir(staging)
        .map_err(Error::io(staging))?
        .collect::<std::io::Result<Vec<_>>>()
        .map_err(Error::io(staging))?;
    match (children.pop(), children.is_empty()) {
        (Some(child), true) if child.path().is_dir() => Ok(child.path()),
        _ => Err(Error::UnsafeArchive(
            "the archive does not have a single top-level directory".to_string(),
        )),
    }
}

pub fn copy(from: &Path, to: &Path, mode: u32) -> Result<()> {
    ensure_absent(to)?;
    ensure_parent(to)?;
    fs::copy(from, to).map_err(Error::io(from))?;
    set_mode(to, mode)
}

pub fn link(target: &Path, link: &Path) -> Result<()> {
    ensure_absent(link)?;
    ensure_parent(link)?;
    symlink(target, link).map_err(Error::io(link))
}

pub fn write(path: &Path, content: &str, mode: u32) -> Result<()> {
    ensure_absent(path)?;
    ensure_parent(path)?;
    fs::write(path, content).map_err(Error::io(path))?;
    set_mode(path, mode)
}

pub fn remove(path: &Path, paths: &Paths) -> Result<()> {
    if !paths.manages(path) {
        return Err(Error::UnsafeRemoval(path.to_path_buf()));
    }
    remove_tree(path)
}

fn remove_tree(path: &Path) -> Result<()> {
    let outcome = match fs::symlink_metadata(path) {
        Ok(meta) if meta.is_dir() => fs::remove_dir_all(path),
        Ok(_) => fs::remove_file(path),
        Err(error) => Err(error),
    };
    match outcome {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(Error::io(path)(error)),
        _ => Ok(()),
    }
}

fn ensure_absent(path: &Path) -> Result<()> {
    if fs::symlink_metadata(path).is_ok() {
        return Err(Error::Conflict(path.to_path_buf()));
    }
    Ok(())
}

fn ensure_parent(path: &Path) -> Result<()> {
    let parent = path.parent().unwrap_or(Path::new("."));
    fs::create_dir_all(parent).map_err(Error::io(parent))
}

fn set_mode(path: &Path, mode: u32) -> Result<()> {
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).map_err(Error::io(path))
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    fn tarball(build: impl FnOnce(&mut tar::Builder<Vec<u8>>)) -> Vec<u8> {
        let mut builder = tar::Builder::new(Vec::new());
        build(&mut builder);
        builder.into_inner().unwrap()
    }

    fn add_file(builder: &mut tar::Builder<Vec<u8>>, path: &str, data: &[u8], mode: u32) {
        let mut header = tar::Header::new_gnu();
        header.set_size(data.len() as u64);
        header.set_mode(mode);
        header.set_cksum();
        builder.append_data(&mut header, path, data).unwrap();
    }

    fn add_dir(builder: &mut tar::Builder<Vec<u8>>, path: &str) {
        let mut header = tar::Header::new_gnu();
        header.set_entry_type(tar::EntryType::Directory);
        header.set_mode(0o755);
        header.set_size(0);
        header.set_cksum();
        builder
            .append_data(&mut header, path, std::io::empty())
            .unwrap();
    }

    fn gzip(data: &[u8]) -> Vec<u8> {
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(data).unwrap();
        encoder.finish().unwrap()
    }

    fn write_archive(dir: &Path, name: &str, bytes: Vec<u8>) -> PathBuf {
        let path = dir.join(name);
        fs::write(&path, bytes).unwrap();
        path
    }

    #[test]
    fn extracts_and_flattens_a_single_top_directory() {
        let dir = tempfile::tempdir().unwrap();
        let archive = write_archive(
            dir.path(),
            "a.tar.gz",
            gzip(&tarball(|b| {
                add_dir(b, "app-1.0/");
                add_file(b, "app-1.0/bin/app", b"#!/bin/sh\n", 0o755);
                add_file(b, "app-1.0/README", b"hi", 0o644);
            })),
        );
        let dest = dir.path().join("opt/app");
        extract(&archive, Format::TarGz, &dest, true).unwrap();
        assert_eq!(fs::read(dest.join("README")).unwrap(), b"hi");
        let mode = fs::metadata(dest.join("bin/app"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o755);
        assert!(!dir.path().join("opt/.app.glori-tmp").exists());
    }

    #[test]
    fn extracts_without_flattening() {
        let dir = tempfile::tempdir().unwrap();
        let archive = write_archive(
            dir.path(),
            "a.tar.gz",
            gzip(&tarball(|b| {
                add_file(b, "tool", b"x", 0o755);
                add_file(b, "README", b"y", 0o644);
            })),
        );
        let dest = dir.path().join("opt/tool");
        extract(&archive, Format::TarGz, &dest, false).unwrap();
        assert!(dest.join("tool").is_file());
        assert!(dest.join("README").is_file());
    }

    #[test]
    fn refuses_to_overwrite_an_existing_install() {
        let dir = tempfile::tempdir().unwrap();
        let archive = write_archive(
            dir.path(),
            "a.tar.gz",
            gzip(&tarball(|b| add_file(b, "f", b"x", 0o644))),
        );
        let dest = dir.path().join("opt/app");
        fs::create_dir_all(&dest).unwrap();
        assert!(matches!(
            extract(&archive, Format::TarGz, &dest, false),
            Err(Error::Conflict(_))
        ));
    }

    #[test]
    fn failed_extraction_leaves_nothing_behind() {
        let dir = tempfile::tempdir().unwrap();
        let archive = write_archive(dir.path(), "broken.tar.gz", gzip(b"not a tar at all"));
        let dest = dir.path().join("opt/app");
        assert!(extract(&archive, Format::TarGz, &dest, false).is_err());
        assert!(!dest.exists());
        assert!(!dir.path().join("opt/.app.glori-tmp").exists());
    }

    #[test]
    fn multiple_top_directories_cannot_be_flattened() {
        let dir = tempfile::tempdir().unwrap();
        let archive = write_archive(
            dir.path(),
            "a.tar.gz",
            gzip(&tarball(|b| {
                add_file(b, "one/f", b"1", 0o644);
                add_file(b, "two/f", b"2", 0o644);
            })),
        );
        let dest = dir.path().join("opt/app");
        assert!(extract(&archive, Format::TarGz, &dest, true).is_err());
        assert!(!dest.exists());
    }

    #[test]
    fn copy_link_and_write_create_parents_and_refuse_overwrites() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("src.bin");
        fs::write(&source, b"data").unwrap();
        let copied = dir.path().join("a/b/copy.bin");
        copy(&source, &copied, 0o755).unwrap();
        assert_eq!(
            fs::metadata(&copied).unwrap().permissions().mode() & 0o777,
            0o755
        );
        assert!(matches!(
            copy(&source, &copied, 0o755),
            Err(Error::Conflict(_))
        ));
        let linked = dir.path().join("bin/copy");
        link(&copied, &linked).unwrap();
        assert_eq!(fs::read_link(&linked).unwrap(), copied);
        assert!(matches!(link(&copied, &linked), Err(Error::Conflict(_))));
        let written = dir.path().join("apps/x.desktop");
        write(&written, "hi", 0o644).unwrap();
        assert_eq!(fs::read_to_string(&written).unwrap(), "hi");
        assert!(matches!(
            write(&written, "again", 0o644),
            Err(Error::Conflict(_))
        ));
    }

    #[test]
    fn a_dangling_symlink_counts_as_existing() {
        let dir = tempfile::tempdir().unwrap();
        let dangling = dir.path().join("l");
        symlink("/nonexistent", &dangling).unwrap();
        assert!(matches!(
            write(&dangling, "x", 0o644),
            Err(Error::Conflict(_))
        ));
    }

    #[test]
    fn remove_only_touches_managed_paths() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().to_path_buf();
        let paths = Paths::new(home.clone(), home.join(".local/share"));
        let app = paths.app_dir("a");
        fs::create_dir_all(app.join("sub")).unwrap();
        fs::write(app.join("sub/f"), "x").unwrap();
        remove(&app, &paths).unwrap();
        assert!(!app.exists());
        remove(&app, &paths).unwrap();
        let outside = home.join("precious");
        fs::write(&outside, "keep").unwrap();
        assert!(matches!(
            remove(&outside, &paths),
            Err(Error::UnsafeRemoval(_))
        ));
        assert!(outside.exists());
    }

    #[test]
    fn removing_a_symlink_does_not_touch_its_target() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().to_path_buf();
        let paths = Paths::new(home.clone(), home.join(".local/share"));
        let target = home.join("precious");
        fs::write(&target, "keep").unwrap();
        let bin = paths.bin_dir();
        fs::create_dir_all(&bin).unwrap();
        let link_path = bin.join("tool");
        symlink(&target, &link_path).unwrap();
        remove(&link_path, &paths).unwrap();
        assert!(!link_path.exists());
        assert_eq!(fs::read_to_string(&target).unwrap(), "keep");
    }
}
