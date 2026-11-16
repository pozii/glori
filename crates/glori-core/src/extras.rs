use std::path::PathBuf;

use crate::paths::Paths;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Man(u8),
    Bash,
    Fish,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Extra {
    pub kind: Kind,
    pub path: PathBuf,
}

impl Extra {
    pub fn destination(&self, paths: &Paths) -> Option<PathBuf> {
        let file = self.path.file_name()?.to_string_lossy().into_owned();
        Some(match self.kind {
            Kind::Man(section) => paths.man_dir(section).join(file),
            Kind::Bash => paths.bash_completion_dir().join(bash_name(&file)),
            Kind::Fish => paths.fish_completion_dir().join(file),
        })
    }
}

pub fn classify(parts: &[String]) -> Option<Kind> {
    let (file, dirs) = parts.split_last()?;
    if file.contains(".so") {
        return None;
    }
    if let Some(section) = man_section(file)
        && dirs.iter().any(|dir| is_man_dir(dir))
    {
        return Some(Kind::Man(section));
    }
    if file.ends_with(".bash") || file.ends_with(".bash-completion") {
        return Some(Kind::Bash);
    }
    if file.ends_with(".fish") && dirs.iter().any(|dir| dir.contains("complet")) {
        return Some(Kind::Fish);
    }
    None
}

fn man_section(file: &str) -> Option<u8> {
    let name = file.strip_suffix(".gz").unwrap_or(file);
    let (stem, section) = name.rsplit_once('.')?;
    let digit = section.chars().next().filter(char::is_ascii_digit)?;
    let valid = !stem.is_empty() && matches!(digit, '1'..='8') && section.len() <= 2;
    valid.then(|| digit as u8 - b'0')
}

fn is_man_dir(dir: &str) -> bool {
    dir == "man"
        || dir == "doc"
        || dir == "docs"
        || dir
            .strip_prefix("man")
            .is_some_and(|rest| rest.len() == 1 && rest.chars().all(|c| c.is_ascii_digit()))
}

fn bash_name(file: &str) -> String {
    file.strip_suffix(".bash-completion")
        .or_else(|| file.strip_suffix(".bash"))
        .unwrap_or(file)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parts(path: &str) -> Vec<String> {
        path.split('/').map(str::to_string).collect()
    }

    #[test]
    fn man_pages_are_found_by_section() {
        assert_eq!(classify(&parts("doc/rg.1")), Some(Kind::Man(1)));
        assert_eq!(
            classify(&parts("share/man/man5/tool.conf.5.gz")),
            Some(Kind::Man(5))
        );
        assert_eq!(classify(&parts("man/tool.8")), Some(Kind::Man(8)));
    }

    #[test]
    fn libraries_and_stray_numbers_are_not_man_pages() {
        assert_eq!(classify(&parts("lib/libz.so.1")), None);
        assert_eq!(classify(&parts("doc/version.9")), None);
        assert_eq!(classify(&parts("src/data.1")), None);
        assert_eq!(classify(&parts("doc/.1")), None);
    }

    #[test]
    fn completions_are_found_by_shell() {
        assert_eq!(classify(&parts("complete/rg.bash")), Some(Kind::Bash));
        assert_eq!(classify(&parts("complete/rg.fish")), Some(Kind::Fish));
        assert_eq!(classify(&parts("rg.fish")), None);
    }

    #[test]
    fn bash_completions_lose_their_suffix() {
        assert_eq!(bash_name("rg.bash"), "rg");
        assert_eq!(bash_name("rg.bash-completion"), "rg");
    }
}
