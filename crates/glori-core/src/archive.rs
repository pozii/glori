use std::{
    fs::File,
    io::{BufReader, Read},
    path::{Component, Path, PathBuf},
};

use tar::EntryType;

use crate::{
    error::{Error, Result},
    format::Format,
    libs::{self, Binary, FileIndex},
    zstd_frames::ZstdFrames,
};

const MAX_ELF_BYTES: u64 = 512 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemberKind {
    File,
    Directory,
    Symlink,
    Hardlink,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Member {
    pub path: PathBuf,
    pub kind: MemberKind,
    pub mode: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Layout {
    pub top: Option<String>,
    pub bin_executables: Vec<PathBuf>,
    pub root_executables: Vec<PathBuf>,
    pub icon: Option<PathBuf>,
    pub has_desktop_file: bool,
}

#[derive(Debug, Clone)]
pub struct Inspection {
    pub layout: Layout,
    pub binaries: Vec<Binary>,
    pub files: FileIndex,
}

pub fn open(file: &Path, format: Format) -> Result<Box<dyn Read>> {
    let reader = BufReader::new(File::open(file).map_err(Error::io(file))?);
    let wrap = |error: std::io::Error| Error::io(file)(error);
    Ok(match format {
        Format::TarGz => Box::new(flate2::read::MultiGzDecoder::new(reader)),
        Format::TarXz => Box::new(lzma_rust2::XzReader::new(reader, true)),
        Format::TarZst => Box::new(ZstdFrames::new(reader).map_err(wrap)?),
        Format::TarBz2 => Box::new(bzip2::read::MultiBzDecoder::new(reader)),
        _ => return Err(Error::Unsupported("reading this format as an archive")),
    })
}

pub fn inspect(file: &Path, format: Format) -> Result<Inspection> {
    let mut archive = tar::Archive::new(open(file, format)?);
    let mut members = Vec::new();
    let mut found = Vec::new();
    for entry in archive.entries().map_err(Error::io(file))? {
        let mut entry = entry.map_err(Error::io(file))?;
        let path = entry.path().map_err(Error::io(file))?.into_owned();
        let link_target = entry
            .link_name()
            .map_err(Error::io(file))?
            .map(|target| target.into_owned());
        let kind = kind_of(entry.header().entry_type());
        if !is_safe(&path) {
            return Err(Error::UnsafeArchive(path.display().to_string()));
        }
        if kind == MemberKind::Hardlink && link_target.as_deref().is_none_or(|t| !is_safe(t)) {
            return Err(Error::UnsafeArchive(path.display().to_string()));
        }
        let mode = entry.header().mode().unwrap_or(0);
        let size = entry.header().size().unwrap_or(0);
        if kind == MemberKind::File && may_be_binary(&path, mode) && size <= MAX_ELF_BYTES {
            let mut data = Vec::with_capacity(usize::try_from(size).unwrap_or(0));
            entry.read_to_end(&mut data).map_err(Error::io(file))?;
            if let Some(elf) = libs::elf::parse(&data) {
                found.push((path.clone(), elf));
            }
        }
        members.push(Member { path, kind, mode });
    }
    if members.is_empty() {
        return Err(Error::EmptyArchive);
    }
    let layout = layout_of(&members);
    let stripped = layout.top.is_some();
    let relative = |path: &Path| -> PathBuf {
        let parts = normal_components(path);
        parts.into_iter().skip(usize::from(stripped)).collect()
    };
    let mut files = FileIndex::default();
    for member in members.iter().filter(|m| m.kind != MemberKind::Directory) {
        files.insert(relative(&member.path));
    }
    let binaries = found
        .into_iter()
        .map(|(path, elf)| Binary {
            path: relative(&path),
            elf,
        })
        .collect();
    Ok(Inspection {
        layout,
        binaries,
        files,
    })
}

fn may_be_binary(path: &Path, mode: u32) -> bool {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy())
        .unwrap_or_default();
    mode & 0o111 != 0 || name.contains(".so") || name.ends_with(".node")
}

fn kind_of(entry_type: EntryType) -> MemberKind {
    match entry_type {
        EntryType::Regular | EntryType::Continuous => MemberKind::File,
        EntryType::Directory => MemberKind::Directory,
        EntryType::Symlink => MemberKind::Symlink,
        EntryType::Link => MemberKind::Hardlink,
        _ => MemberKind::Other,
    }
}

pub fn is_safe(path: &Path) -> bool {
    path.components()
        .all(|component| matches!(component, Component::Normal(_) | Component::CurDir))
}

fn normal_components(path: &Path) -> Vec<String> {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(part) => Some(part.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect()
}

pub fn layout_of(members: &[Member]) -> Layout {
    let top = single_top_directory(members);
    let relative = |member: &Member| -> Vec<String> {
        let parts = normal_components(&member.path);
        if top.is_some() {
            parts.into_iter().skip(1).collect()
        } else {
            parts
        }
    };
    let mut layout = Layout {
        top: top.clone(),
        ..Layout::default()
    };
    for member in members.iter().filter(|m| m.kind == MemberKind::File) {
        let parts = relative(member);
        let Some(file_name) = parts.last() else {
            continue;
        };
        let executable = member.mode & 0o111 != 0;
        if executable && parts.len() == 2 && parts[0] == "bin" {
            layout.bin_executables.push(parts.iter().collect());
        } else if executable && parts.len() == 1 && !looks_like_library(file_name) {
            layout.root_executables.push(parts.iter().collect());
        }
        if file_name.ends_with(".desktop") {
            layout.has_desktop_file = true;
        }
        if icon_rank(&parts)
            > layout
                .icon
                .as_ref()
                .map_or((0, 0), |i| icon_rank(&normal_components(i)))
        {
            layout.icon = Some(parts.iter().collect());
        }
    }
    layout
}

fn single_top_directory(members: &[Member]) -> Option<String> {
    let first = normal_components(&members.first()?.path)
        .into_iter()
        .next()?;
    let all_inside = members.iter().all(|member| {
        let parts = normal_components(&member.path);
        parts.first() == Some(&first) && (parts.len() > 1 || member.kind == MemberKind::Directory)
    });
    all_inside.then_some(first)
}

fn looks_like_library(file_name: &str) -> bool {
    file_name.contains(".so") || file_name.ends_with(".dll") || file_name.ends_with(".dylib")
}

fn icon_rank(parts: &[String]) -> (u8, u32) {
    let Some(file_name) = parts.last() else {
        return (0, 0);
    };
    let extension = file_name.rsplit('.').next().unwrap_or("").to_lowercase();
    let kind = match extension.as_str() {
        "svg" => 3,
        "png" => 2,
        "xpm" => 1,
        _ => return (0, 0),
    };
    let size = parts
        .iter()
        .filter_map(|part| part.split('x').next()?.parse::<u32>().ok())
        .max()
        .unwrap_or(0);
    (kind, size)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn member(path: &str, kind: MemberKind, mode: u32) -> Member {
        Member {
            path: PathBuf::from(path),
            kind,
            mode,
        }
    }

    fn file(path: &str, mode: u32) -> Member {
        member(path, MemberKind::File, mode)
    }

    fn dir(path: &str) -> Member {
        member(path, MemberKind::Directory, 0o755)
    }

    #[test]
    fn single_top_directory_is_detected_and_stripped() {
        let layout = layout_of(&[
            dir("app-1.0/"),
            dir("app-1.0/bin/"),
            file("app-1.0/bin/app", 0o755),
            file("app-1.0/README", 0o644),
        ]);
        assert_eq!(layout.top.as_deref(), Some("app-1.0"));
        assert_eq!(layout.bin_executables, [PathBuf::from("bin/app")]);
    }

    #[test]
    fn archives_without_a_common_directory_are_not_stripped() {
        let layout = layout_of(&[file("tool", 0o755), file("README", 0o644)]);
        assert_eq!(layout.top, None);
        assert_eq!(layout.root_executables, [PathBuf::from("tool")]);
    }

    #[test]
    fn a_single_root_file_is_not_a_top_directory() {
        assert_eq!(layout_of(&[file("tool", 0o755)]).top, None);
    }

    #[test]
    fn root_level_executables_are_found_after_stripping() {
        let layout = layout_of(&[dir("rg/"), file("rg/rg", 0o755), file("rg/doc.txt", 0o644)]);
        assert_eq!(layout.root_executables, [PathBuf::from("rg")]);
    }

    #[test]
    fn shared_libraries_are_not_launchers() {
        let layout = layout_of(&[file("libfoo.so.1", 0o755), file("foo", 0o755)]);
        assert_eq!(layout.root_executables, [PathBuf::from("foo")]);
    }

    #[test]
    fn non_executables_are_ignored() {
        let layout = layout_of(&[dir("a/"), file("a/bin/readme", 0o644)]);
        assert!(layout.bin_executables.is_empty());
    }

    #[test]
    fn the_best_icon_wins() {
        let layout = layout_of(&[
            dir("a/"),
            file("a/share/icons/48x48/app.png", 0o644),
            file("a/share/icons/256x256/app.png", 0o644),
            file("a/share/icons/hicolor/128x128/app.png", 0o644),
        ]);
        assert_eq!(
            layout.icon,
            Some(PathBuf::from("share/icons/256x256/app.png"))
        );
        let with_svg = layout_of(&[
            dir("a/"),
            file("a/icon.png", 0o644),
            file("a/icon.svg", 0o644),
        ]);
        assert_eq!(with_svg.icon, Some(PathBuf::from("icon.svg")));
    }

    #[test]
    fn a_desktop_file_is_noticed() {
        let layout = layout_of(&[dir("a/"), file("a/share/applications/a.desktop", 0o644)]);
        assert!(layout.has_desktop_file);
    }

    #[test]
    fn unsafe_paths_are_recognised() {
        assert!(is_safe(Path::new("a/b")));
        assert!(is_safe(Path::new("./a")));
        assert!(!is_safe(Path::new("../a")));
        assert!(!is_safe(Path::new("a/../../b")));
        assert!(!is_safe(Path::new("/etc/passwd")));
    }
}
