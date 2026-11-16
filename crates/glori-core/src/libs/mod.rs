pub mod elf;
mod providers;
mod system;

use std::{
    collections::HashSet,
    path::{Component, Path, PathBuf},
};

use serde::Serialize;

use crate::distro::PackageManager;
use elf::{Class, Elf};
pub use system::SystemLibs;

const MAX_LOOKUPS: usize = 24;
const MAX_REPORTED_USERS: usize = 3;

#[derive(Debug, Clone)]
pub struct Binary {
    pub path: PathBuf,
    pub elf: Elf,
}

#[derive(Debug, Clone, Default)]
pub struct FileIndex {
    paths: HashSet<PathBuf>,
    names: HashSet<String>,
}

impl FileIndex {
    pub fn insert(&mut self, path: PathBuf) {
        if let Some(name) = path.file_name() {
            self.names.insert(name.to_string_lossy().into_owned());
        }
        self.paths.insert(path);
    }

    fn contains(&self, path: &Path) -> bool {
        self.paths.contains(path)
    }

    fn has_name(&self, name: &str) -> bool {
        self.names.contains(name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Missing {
    pub soname: String,
    pub needed_by: Vec<String>,
    pub package: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Report {
    pub missing: Vec<Missing>,
    pub incompatible: Vec<String>,
    pub scanned: usize,
    pub hint: Option<String>,
}

impl Report {
    pub fn packages(&self) -> Vec<String> {
        let mut packages: Vec<String> = self
            .missing
            .iter()
            .filter_map(|missing| missing.package.clone())
            .collect();
        packages.sort();
        packages.dedup();
        packages
    }
}

pub fn host_machine() -> u16 {
    machine_of(std::env::consts::ARCH)
}

pub fn machine_of(arch: &str) -> u16 {
    match arch {
        "x86_64" => 62,
        "x86" => 3,
        "aarch64" => 183,
        "arm" => 40,
        "riscv64" => 243,
        "powerpc64" => 21,
        "s390x" => 22,
        _ => 0,
    }
}

pub fn machine_name(machine: u16) -> &'static str {
    match machine {
        62 => "x86_64",
        3 => "x86",
        183 => "aarch64",
        40 => "arm",
        243 => "riscv64",
        21 => "powerpc64",
        22 => "s390x",
        _ => "an unknown architecture",
    }
}

pub fn analyze(
    binaries: &[Binary],
    launchers: &[PathBuf],
    index: &FileIndex,
    system: &SystemLibs,
    host: u16,
) -> Report {
    let mut missing: Vec<Missing> = Vec::new();
    for binary in binaries {
        for name in &binary.elf.needed {
            if satisfied(name, binary, index, system) {
                continue;
            }
            let user = binary
                .path
                .file_name()
                .map(|file| file.to_string_lossy().into_owned())
                .unwrap_or_default();
            match missing.iter_mut().find(|entry| &entry.soname == name) {
                Some(entry) => {
                    if entry.needed_by.len() < MAX_REPORTED_USERS
                        && !entry.needed_by.contains(&user)
                    {
                        entry.needed_by.push(user);
                    }
                }
                None => missing.push(Missing {
                    soname: name.clone(),
                    needed_by: vec![user],
                    package: None,
                }),
            }
        }
    }
    missing.sort_by(|a, b| a.soname.cmp(&b.soname));
    let incompatible = if host == 0 {
        Vec::new()
    } else {
        launchers
            .iter()
            .filter_map(|launcher| {
                let binary = binaries.iter().find(|binary| &binary.path == launcher)?;
                (binary.elf.machine != host).then(|| {
                    format!(
                        "{} is built for {}, but this machine is {}",
                        launcher.display(),
                        machine_name(binary.elf.machine),
                        machine_name(host)
                    )
                })
            })
            .collect()
    };
    Report {
        missing,
        incompatible,
        scanned: binaries.len(),
        hint: None,
    }
}

fn satisfied(name: &str, binary: &Binary, index: &FileIndex, system: &SystemLibs) -> bool {
    if name.contains('/') || system.has(name) {
        return true;
    }
    let origin = binary.path.parent().unwrap_or(Path::new(""));
    let in_search_path = binary
        .elf
        .runpath
        .iter()
        .chain(&binary.elf.rpath)
        .filter_map(|directory| expand_origin(directory, origin))
        .any(|directory| index.contains(&directory.join(name)));
    in_search_path || index.has_name(name)
}

fn expand_origin(directory: &str, origin: &Path) -> Option<PathBuf> {
    if !directory.contains("$ORIGIN") && !directory.contains("${ORIGIN}") {
        return None;
    }
    let origin_text = origin.to_string_lossy();
    let expanded = directory
        .replace("${ORIGIN}", &origin_text)
        .replace("$ORIGIN", &origin_text);
    normalize(Path::new(&expanded))
}

fn normalize(path: &Path) -> Option<PathBuf> {
    let mut parts: Vec<&std::ffi::OsStr> = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => parts.push(part),
            Component::CurDir | Component::RootDir => {}
            Component::ParentDir => {
                parts.pop()?;
            }
            Component::Prefix(_) => return None,
        }
    }
    Some(parts.into_iter().collect())
}

pub fn resolve_packages(report: &mut Report, manager: PackageManager, class: Class) {
    let wide = class == Class::Bits64;
    for missing in report.missing.iter_mut().take(MAX_LOOKUPS) {
        match providers::package_for(manager, &missing.soname, wide) {
            providers::Lookup::Found(package) => missing.package = Some(package),
            providers::Lookup::NotFound => {}
            providers::Lookup::Unavailable(hint) => {
                report.hint = Some(hint.to_string());
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn binary(path: &str, needed: &[&str], runpath: &[&str]) -> Binary {
        Binary {
            path: PathBuf::from(path),
            elf: Elf {
                class: Class::Bits64,
                machine: 62,
                needed: needed.iter().map(|name| name.to_string()).collect(),
                soname: None,
                rpath: Vec::new(),
                runpath: runpath.iter().map(|dir| dir.to_string()).collect(),
            },
        }
    }

    fn index(paths: &[&str]) -> FileIndex {
        let mut index = FileIndex::default();
        for path in paths {
            index.insert(PathBuf::from(path));
        }
        index
    }

    fn run(binaries: &[Binary], files: &[&str], system: &[&str]) -> Report {
        analyze(
            binaries,
            &[],
            &index(files),
            &SystemLibs::from_names(system.iter().copied()),
            62,
        )
    }

    #[test]
    fn libraries_found_on_the_system_are_fine() {
        let report = run(
            &[binary("bin/app", &["libc.so.6"], &[])],
            &[],
            &["libc.so.6"],
        );
        assert!(report.missing.is_empty());
    }

    #[test]
    fn a_missing_library_is_reported_with_who_needs_it() {
        let report = run(
            &[binary("bin/app", &["libc.so.6", "libcrypt.so.1"], &[])],
            &[],
            &["libc.so.6"],
        );
        assert_eq!(report.missing.len(), 1);
        assert_eq!(report.missing[0].soname, "libcrypt.so.1");
        assert_eq!(report.missing[0].needed_by, ["app"]);
        assert_eq!(report.missing[0].package, None);
    }

    #[test]
    fn a_library_found_through_origin_relative_runpath_is_bundled() {
        let report = run(
            &[binary("bin/app", &["libfoo.so.1"], &["$ORIGIN/../lib"])],
            &["lib/libfoo.so.1"],
            &[],
        );
        assert!(report.missing.is_empty());
    }

    #[test]
    fn a_symlinked_library_in_the_archive_counts() {
        let report = run(
            &[binary("bin/app", &["libfoo.so.1"], &["$ORIGIN/../lib"])],
            &["lib/libfoo.so.1", "lib/libfoo.so.1.0.0"],
            &[],
        );
        assert!(report.missing.is_empty());
    }

    #[test]
    fn a_library_bundled_elsewhere_is_given_the_benefit_of_the_doubt() {
        let report = run(
            &[binary("bin/app", &["libfoo.so.1"], &[])],
            &["private/libs/libfoo.so.1"],
            &[],
        );
        assert!(report.missing.is_empty());
    }

    #[test]
    fn libraries_depending_on_each_other_are_resolved_per_file() {
        let report = run(
            &[
                binary("bin/app", &["libfoo.so.1"], &["$ORIGIN/../lib"]),
                binary("lib/libfoo.so.1", &["libcrypt.so.1"], &["$ORIGIN"]),
            ],
            &["lib/libfoo.so.1"],
            &[],
        );
        assert_eq!(report.missing.len(), 1);
        assert_eq!(report.missing[0].soname, "libcrypt.so.1");
        assert_eq!(report.missing[0].needed_by, ["libfoo.so.1"]);
    }

    #[test]
    fn one_missing_library_needed_by_many_files_is_listed_once() {
        let report = run(
            &[
                binary("bin/a", &["libx.so.1"], &[]),
                binary("bin/b", &["libx.so.1"], &[]),
                binary("bin/c", &["libx.so.1"], &[]),
                binary("bin/d", &["libx.so.1"], &[]),
            ],
            &[],
            &[],
        );
        assert_eq!(report.missing.len(), 1);
        assert_eq!(report.missing[0].needed_by.len(), MAX_REPORTED_USERS);
    }

    #[test]
    fn explicit_paths_are_not_judged() {
        let report = run(&[binary("bin/app", &["/opt/x/libx.so"], &[])], &[], &[]);
        assert!(report.missing.is_empty());
    }

    #[test]
    fn a_launcher_built_for_another_architecture_is_flagged() {
        let mut arm = binary("bin/app", &[], &[]);
        arm.elf.machine = 183;
        let report = analyze(
            &[arm],
            &[PathBuf::from("bin/app")],
            &FileIndex::default(),
            &SystemLibs::default(),
            62,
        );
        assert_eq!(report.incompatible.len(), 1);
        assert!(report.incompatible[0].contains("aarch64"));
        assert!(report.incompatible[0].contains("x86_64"));
    }

    #[test]
    fn matching_architectures_and_unknown_hosts_are_not_flagged() {
        let app = binary("bin/app", &[], &[]);
        let launchers = [PathBuf::from("bin/app")];
        let flagged = |host| {
            analyze(
                std::slice::from_ref(&app),
                &launchers,
                &FileIndex::default(),
                &SystemLibs::default(),
                host,
            )
            .incompatible
        };
        assert!(flagged(62).is_empty());
        assert!(flagged(0).is_empty());
    }

    #[test]
    fn origin_expansion_handles_both_spellings_and_parent_directories() {
        let origin = Path::new("a/b");
        assert_eq!(
            expand_origin("$ORIGIN/../lib", origin),
            Some(PathBuf::from("a/lib"))
        );
        assert_eq!(
            expand_origin("${ORIGIN}/x", origin),
            Some(PathBuf::from("a/b/x"))
        );
        assert_eq!(expand_origin("/usr/lib", origin), None);
        assert_eq!(expand_origin("$ORIGIN/../../../..", origin), None);
    }

    #[test]
    fn package_list_is_sorted_and_deduplicated() {
        let report = Report {
            missing: vec![
                Missing {
                    soname: "a".into(),
                    needed_by: vec![],
                    package: Some("gtk2".into()),
                },
                Missing {
                    soname: "b".into(),
                    needed_by: vec![],
                    package: Some("gtk2".into()),
                },
                Missing {
                    soname: "c".into(),
                    needed_by: vec![],
                    package: None,
                },
                Missing {
                    soname: "d".into(),
                    needed_by: vec![],
                    package: Some("alpha".into()),
                },
            ],
            ..Report::default()
        };
        assert_eq!(report.packages(), ["alpha", "gtk2"]);
    }

    #[test]
    fn machines_round_trip() {
        assert_eq!(machine_of("x86_64"), 62);
        assert_eq!(machine_name(machine_of("aarch64")), "aarch64");
        assert_eq!(machine_of("sparc"), 0);
    }
}
