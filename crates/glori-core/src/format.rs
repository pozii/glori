use std::{fs::File, io::Read, path::Path};

use serde::Serialize;

use crate::{
    distro::Family,
    error::{Error, Result},
};

const HEADER_LEN: u64 = 32;
const DEB: &[u8] = b"!<arch>\ndebian-binary";
const RPM: &[u8] = &[0xED, 0xAB, 0xEE, 0xDB];
const ELF: &[u8] = b"\x7fELF";
const GZIP: &[u8] = &[0x1F, 0x8B];
const XZ: &[u8] = &[0xFD, b'7', b'z', b'X', b'Z', 0x00];
const ZSTD: &[u8] = &[0x28, 0xB5, 0x2F, 0xFD];
const BZIP2: &[u8] = b"BZh";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Format {
    Deb,
    Rpm,
    AppImage,
    ArchPackage,
    TarGz,
    TarXz,
    TarZst,
    TarBz2,
}

impl Format {
    pub fn label(self) -> &'static str {
        match self {
            Format::Deb => "Debian package (.deb)",
            Format::Rpm => "RPM package (.rpm)",
            Format::AppImage => "AppImage",
            Format::ArchPackage => "Arch package (.pkg.tar)",
            Format::TarGz => "gzip tarball (.tar.gz)",
            Format::TarXz => "xz tarball (.tar.xz)",
            Format::TarZst => "zstd tarball (.tar.zst)",
            Format::TarBz2 => "bzip2 tarball (.tar.bz2)",
        }
    }

    pub fn is_native_to(self, family: Family) -> bool {
        matches!(
            (self, family),
            (Format::Deb, Family::Debian)
                | (Format::Rpm, Family::Fedora | Family::Suse)
                | (Format::ArchPackage, Family::Arch)
        )
    }
}

pub fn detect(path: &Path) -> Result<Format> {
    let mut header = Vec::with_capacity(HEADER_LEN as usize);
    File::open(path)
        .and_then(|file| file.take(HEADER_LEN).read_to_end(&mut header))
        .map_err(Error::io(path))?;
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    from_header(&header, &name).ok_or_else(|| Error::UnknownFormat(path.to_path_buf()))
}

pub fn from_header(header: &[u8], file_name: &str) -> Option<Format> {
    let tarball = |format| {
        if file_name.contains(".pkg.tar") {
            Format::ArchPackage
        } else {
            format
        }
    };
    match header {
        h if h.starts_with(DEB) => Some(Format::Deb),
        h if h.starts_with(RPM) => Some(Format::Rpm),
        h if is_appimage(h) => Some(Format::AppImage),
        h if h.starts_with(GZIP) => Some(tarball(Format::TarGz)),
        h if h.starts_with(XZ) => Some(tarball(Format::TarXz)),
        h if h.starts_with(ZSTD) => Some(tarball(Format::TarZst)),
        h if h.starts_with(BZIP2) => Some(tarball(Format::TarBz2)),
        _ => None,
    }
}

fn is_appimage(header: &[u8]) -> bool {
    header.starts_with(ELF)
        && header.get(8..10) == Some(b"AI".as_slice())
        && matches!(header.get(10), Some(1 | 2))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn appimage_header() -> Vec<u8> {
        let mut header = b"\x7fELF".to_vec();
        header.extend_from_slice(&[2, 1, 1, 0, b'A', b'I', 2]);
        header
    }

    #[test]
    fn detects_deb() {
        assert_eq!(
            from_header(b"!<arch>\ndebian-binary   ", "x.bin"),
            Some(Format::Deb)
        );
    }

    #[test]
    fn static_library_is_not_deb() {
        assert_eq!(from_header(b"!<arch>\nlibfoo.o/       ", "libfoo.a"), None);
    }

    #[test]
    fn detects_rpm() {
        assert_eq!(
            from_header(&[0xED, 0xAB, 0xEE, 0xDB, 3], "x"),
            Some(Format::Rpm)
        );
    }

    #[test]
    fn detects_appimage() {
        assert_eq!(
            from_header(&appimage_header(), "app"),
            Some(Format::AppImage)
        );
    }

    #[test]
    fn plain_elf_is_not_appimage() {
        assert_eq!(
            from_header(b"\x7fELF\x02\x01\x01\x00\x00\x00\x00", "ls"),
            None
        );
    }

    #[test]
    fn detects_tarballs_by_compression() {
        assert_eq!(
            from_header(&[0x1F, 0x8B, 8], "a.tar.gz"),
            Some(Format::TarGz)
        );
        assert_eq!(
            from_header(&[0xFD, b'7', b'z', b'X', b'Z', 0], "a.tar.xz"),
            Some(Format::TarXz)
        );
        assert_eq!(
            from_header(&[0x28, 0xB5, 0x2F, 0xFD], "a.tar.zst"),
            Some(Format::TarZst)
        );
        assert_eq!(from_header(b"BZh9", "a.tar.bz2"), Some(Format::TarBz2));
    }

    #[test]
    fn detects_arch_package_by_name() {
        assert_eq!(
            from_header(&[0x28, 0xB5, 0x2F, 0xFD], "foo-1-1-x86_64.pkg.tar.zst"),
            Some(Format::ArchPackage)
        );
    }

    #[test]
    fn nativeness() {
        assert!(Format::Deb.is_native_to(Family::Debian));
        assert!(Format::Rpm.is_native_to(Family::Suse));
        assert!(!Format::Deb.is_native_to(Family::Fedora));
        assert!(!Format::TarGz.is_native_to(Family::Debian));
    }
}
