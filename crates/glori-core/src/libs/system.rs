use std::{collections::HashSet, fs, path::Path, process::Command};

const LDCONFIG: [&str; 3] = ["ldconfig", "/sbin/ldconfig", "/usr/sbin/ldconfig"];
const FALLBACK_DIRS: [&str; 6] = [
    "/lib",
    "/lib64",
    "/usr/lib",
    "/usr/lib64",
    "/usr/local/lib",
    "/usr/local/lib64",
];

#[derive(Debug, Default)]
pub struct SystemLibs {
    names: HashSet<String>,
}

impl SystemLibs {
    pub fn from_names<I: IntoIterator<Item = S>, S: Into<String>>(names: I) -> Self {
        SystemLibs {
            names: names.into_iter().map(Into::into).collect(),
        }
    }

    pub fn load(machine: u16) -> Self {
        let names = LDCONFIG
            .iter()
            .find_map(|program| cache(program, machine))
            .unwrap_or_else(scan_directories);
        SystemLibs { names }
    }

    pub fn has(&self, name: &str) -> bool {
        self.names.contains(name)
    }
}

fn cache(program: &str, machine: u16) -> Option<HashSet<String>> {
    let output = Command::new(program)
        .arg("-p")
        .env("LC_ALL", "C")
        .output()
        .ok()?;
    let names = parse_ldconfig(&String::from_utf8_lossy(&output.stdout), machine);
    (output.status.success() && !names.is_empty()).then_some(names)
}

pub fn parse_ldconfig(output: &str, machine: u16) -> HashSet<String> {
    output
        .lines()
        .filter_map(|line| {
            let (name, rest) = line.trim().split_once(" (")?;
            let (flags, _) = rest.split_once(')')?;
            matches_machine(flags, machine).then(|| name.to_string())
        })
        .collect()
}

fn matches_machine(flags: &str, machine: u16) -> bool {
    match machine {
        62 => flags.contains("x86-64"),
        183 => flags.contains("AArch64"),
        3 => !flags.contains("x86-64"),
        _ => true,
    }
}

fn scan_directories() -> HashSet<String> {
    let mut directories: Vec<std::path::PathBuf> =
        FALLBACK_DIRS.iter().map(std::path::PathBuf::from).collect();
    for parent in ["/usr/lib", "/lib"] {
        let Ok(entries) = fs::read_dir(parent) else {
            continue;
        };
        directories.extend(
            entries
                .flatten()
                .filter(|entry| entry.file_name().to_string_lossy().contains("-linux-"))
                .map(|entry| entry.path()),
        );
    }
    directories
        .iter()
        .flat_map(|directory| library_names(directory))
        .collect()
}

fn library_names(directory: &Path) -> Vec<String> {
    fs::read_dir(directory)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.contains(".so"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const CACHE: &str = "5 libs found in cache `/etc/ld.so.cache'
\tlibcrypt.so.1 (libc6,x86-64) => /lib64/libcrypt.so.1
\tlibcrypt.so.1 (libc6) => /lib/libcrypt.so.1
\tlibgtk-3.so.0 (libc6,x86-64) => /lib64/libgtk-3.so.0
\tlibaarch.so.1 (libc6,AArch64) => /lib64/libaarch.so.1
\tld-linux-x86-64.so.2 (libc6,x86-64) => /lib64/ld-linux-x86-64.so.2
";

    #[test]
    fn only_libraries_for_the_requested_machine_are_listed() {
        let names = parse_ldconfig(CACHE, 62);
        assert!(names.contains("libcrypt.so.1"));
        assert!(names.contains("libgtk-3.so.0"));
        assert!(names.contains("ld-linux-x86-64.so.2"));
        assert!(!names.contains("libaarch.so.1"));
        let arm = parse_ldconfig(CACHE, 183);
        assert_eq!(arm.len(), 1);
        assert!(arm.contains("libaarch.so.1"));
    }

    #[test]
    fn a_32_bit_binary_does_not_count_64_bit_libraries() {
        let names = parse_ldconfig(CACHE, 3);
        assert!(names.contains("libcrypt.so.1"));
        assert!(!names.contains("libgtk-3.so.0"));
    }

    #[test]
    fn the_header_line_and_junk_are_ignored() {
        assert!(
            parse_ldconfig("3 libs found in cache `/etc/ld.so.cache'\nnonsense\n", 62).is_empty()
        );
    }

    #[test]
    fn this_machine_has_libc() {
        let machine = crate::libs::host_machine();
        assert!(SystemLibs::load(machine).has("libc.so.6") || cfg!(target_env = "musl"));
    }

    #[test]
    fn from_names_is_a_simple_set() {
        let libs = SystemLibs::from_names(["a.so.1"]);
        assert!(libs.has("a.so.1"));
        assert!(!libs.has("b.so.1"));
    }
}
