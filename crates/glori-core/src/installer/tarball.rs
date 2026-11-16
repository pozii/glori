use std::path::PathBuf;

use super::{Input, Installer, Plan, common};
use crate::{
    archive::{self, Layout},
    error::Result,
    libs::{self, SystemLibs, elf::Class},
    operation::Operation,
    registry::Kind,
};

pub struct Tarball;

impl Installer for Tarball {
    fn plan(&self, input: &Input) -> Result<Plan> {
        let file = common::canonical(input.file)?;
        let identity = common::identify(&file, input.name)?;
        let paths = input.paths;
        common::ensure_free(paths, &identity.name)?;
        let inspection = archive::inspect(&file, input.format)?;
        let layout = inspection.layout;
        let dest = paths.app_dir(&identity.name);
        let launchers = launchers(&layout, &identity.name);
        let mut links = Vec::new();
        for launcher in &launchers {
            let link = paths
                .bin_dir()
                .join(launcher.file_name().unwrap_or_default());
            common::absent(&link)?;
            links.push((dest.join(launcher), link));
        }
        let mut files = vec![dest.clone()];
        files.extend(links.iter().map(|(_, link)| link.clone()));
        let mut operations = vec![Operation::Extract {
            archive: file.clone(),
            format: input.format,
            dest: dest.clone(),
            flatten: layout.top.is_some(),
        }];
        operations.extend(links.iter().map(|(target, link)| Operation::Link {
            target: target.clone(),
            link: link.clone(),
        }));
        let graphical = layout.icon.is_some() || layout.has_desktop_file;
        if let (true, Some((target, _))) = (graphical, primary(&links, &identity.name)) {
            let icon = layout.icon.as_ref().map(|icon| dest.join(icon));
            operations.push(common::desktop_entry(
                paths,
                &identity,
                target,
                icon.as_deref(),
            )?);
            files.push(paths.desktop_file(&identity.name));
        }
        operations.push(common::register(
            &identity,
            Kind::Archive,
            common::file_name(&file),
            files,
        ));
        let host = libs::host_machine();
        let mut libraries = libs::analyze(
            &inspection.binaries,
            &launchers,
            &inspection.files,
            &SystemLibs::load(host),
            host,
        );
        if let (Some(manager), false) = (input.manager, libraries.missing.is_empty()) {
            let class = if cfg!(target_pointer_width = "64") {
                Class::Bits64
            } else {
                Class::Bits32
            };
            libs::resolve_packages(&mut libraries, manager, class);
        }
        Ok(Plan {
            operations,
            libraries,
        })
    }
}

fn launchers(layout: &Layout, name: &str) -> Vec<PathBuf> {
    if !layout.bin_executables.is_empty() {
        return layout.bin_executables.clone();
    }
    let named: Vec<PathBuf> = layout
        .root_executables
        .iter()
        .filter(|path| file_name_is(path, name))
        .cloned()
        .collect();
    match (named.is_empty(), layout.root_executables.as_slice()) {
        (false, _) => named,
        (true, [only]) => vec![only.clone()],
        _ => Vec::new(),
    }
}

fn file_name_is(path: &std::path::Path, name: &str) -> bool {
    path.file_name()
        .is_some_and(|file| file.to_string_lossy().eq_ignore_ascii_case(name))
}

fn primary<'a>(links: &'a [(PathBuf, PathBuf)], name: &str) -> Option<&'a (PathBuf, PathBuf)> {
    links
        .iter()
        .find(|(_, link)| file_name_is(link, name))
        .or_else(|| links.first())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout(bin: &[&str], root: &[&str]) -> Layout {
        Layout {
            bin_executables: bin.iter().map(PathBuf::from).collect(),
            root_executables: root.iter().map(PathBuf::from).collect(),
            ..Layout::default()
        }
    }

    #[test]
    fn bin_directory_executables_win() {
        let picked = launchers(&layout(&["bin/a", "bin/b"], &["other"]), "x");
        assert_eq!(picked, [PathBuf::from("bin/a"), PathBuf::from("bin/b")]);
    }

    #[test]
    fn a_root_executable_matching_the_name_is_chosen() {
        let picked = launchers(&layout(&[], &["helper", "App"]), "app");
        assert_eq!(picked, [PathBuf::from("App")]);
    }

    #[test]
    fn a_lone_root_executable_is_chosen() {
        assert_eq!(
            launchers(&layout(&[], &["run"]), "app"),
            [PathBuf::from("run")]
        );
    }

    #[test]
    fn ambiguous_root_executables_are_not_guessed() {
        assert!(launchers(&layout(&[], &["a", "b"]), "app").is_empty());
    }
}
