use std::{fs, os::unix::fs::PermissionsExt, path::PathBuf};

use glori_core::{Format, archive, fsops};

const EXPECTED_DATA: &[u8] = include_bytes!("fixtures/data.txt");

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn check(name: &str, format: Format) {
    let file = fixture(name);
    let layout = archive::inspect(&file, format)
        .unwrap_or_else(|e| panic!("{name}: {e}"))
        .layout;
    assert_eq!(layout.top.as_deref(), Some("app-1.0"), "{name}");
    assert_eq!(layout.bin_executables, [PathBuf::from("bin/app")], "{name}");

    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("app");
    fsops::extract(&file, format, &dest, true).unwrap_or_else(|e| panic!("{name}: {e}"));
    assert_eq!(
        fs::read(dest.join("data.txt")).unwrap(),
        EXPECTED_DATA,
        "{name}"
    );
    let mode = fs::metadata(dest.join("bin/app"))
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o755, "{name}");
}

#[test]
fn single_stream_archives_from_real_compressors() {
    check("single.tar.gz", Format::TarGz);
    check("single.tar.xz", Format::TarXz);
    check("single.tar.zst", Format::TarZst);
    check("single.tar.bz2", Format::TarBz2);
}

#[test]
fn multi_stream_archives_are_read_to_the_end() {
    check("multi.tar.gz", Format::TarGz);
    check("multi.tar.xz", Format::TarXz);
    check("multi.tar.zst", Format::TarZst);
    check("multi.tar.bz2", Format::TarBz2);
}

#[test]
fn truncated_archives_are_errors_not_partial_installs() {
    for (name, format) in [
        ("single.tar.gz", Format::TarGz),
        ("single.tar.xz", Format::TarXz),
        ("single.tar.zst", Format::TarZst),
        ("single.tar.bz2", Format::TarBz2),
    ] {
        let bytes = fs::read(fixture(name)).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let cut = dir.path().join(name);
        fs::write(&cut, &bytes[..bytes.len() / 2]).unwrap();
        let dest = dir.path().join("app");
        assert!(
            fsops::extract(&cut, format, &dest, true).is_err(),
            "{name} was truncated but extraction succeeded"
        );
        assert!(!dest.exists(), "{name} left a partial install behind");
    }
}
