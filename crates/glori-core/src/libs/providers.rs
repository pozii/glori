use std::process::Command;

use crate::{distro::PackageManager, tools};

const APT_FILE_MISSING: &str = "Install apt-file and run `sudo apt-file update` so Glori can tell which packages provide the libraries";
const APT_FILE_EMPTY: &str =
    "Run `sudo apt-file update` so Glori can tell which packages provide the libraries";
const PACMAN_FILES_MISSING: &str =
    "Run `sudo pacman -Fy` to download the file database, then try again";

const IGNORED_SUFFIXES: [&str; 5] = ["-dev", "-devel", "-dbg", "-debuginfo", "-debugsource"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Lookup {
    Found(String),
    NotFound,
    Unavailable(&'static str),
}

pub fn package_for(manager: PackageManager, soname: &str, wide: bool) -> Lookup {
    if manager == PackageManager::Apt && tools::find("apt-file").is_none() {
        return Lookup::Unavailable(APT_FILE_MISSING);
    }
    let Some((program, args)) = lookup_command(manager, soname, wide) else {
        return Lookup::NotFound;
    };
    let Ok(output) = Command::new(program)
        .args(&args)
        .env("LC_ALL", "C")
        .output()
    else {
        return Lookup::NotFound;
    };
    if let Some(reason) = unavailable_reason(manager, &String::from_utf8_lossy(&output.stderr)) {
        return Lookup::Unavailable(reason);
    }
    match parse(manager, soname, &String::from_utf8_lossy(&output.stdout)) {
        Some(package) => Lookup::Found(package),
        None => Lookup::NotFound,
    }
}

fn unavailable_reason(manager: PackageManager, text: &str) -> Option<&'static str> {
    match manager {
        PackageManager::Apt if text.contains("cache is empty") => Some(APT_FILE_EMPTY),
        PackageManager::Pacman if text.contains("database file for") => Some(PACMAN_FILES_MISSING),
        _ => None,
    }
}

fn lookup_command(
    manager: PackageManager,
    soname: &str,
    wide: bool,
) -> Option<(&'static str, Vec<String>)> {
    let capability = if wide {
        format!("{soname}()(64bit)")
    } else {
        soname.to_string()
    };
    match manager {
        PackageManager::Dnf => Some((
            "dnf",
            [
                "repoquery",
                "-q",
                "--qf",
                "%{name}\n",
                "--whatprovides",
                &capability,
            ]
            .map(String::from)
            .to_vec(),
        )),
        PackageManager::Apt => tools::find("apt-file").map(|_| {
            (
                "apt-file",
                vec![
                    "-x".to_string(),
                    "search".to_string(),
                    format!("(^|/){}$", regex_escape(soname)),
                ],
            )
        }),
        PackageManager::Pacman => Some((
            "pacman",
            ["-F", "--machinereadable", soname]
                .map(String::from)
                .to_vec(),
        )),
        PackageManager::Zypper => Some((
            "zypper",
            [
                "--non-interactive",
                "--quiet",
                "search",
                "--provides",
                "--match-exact",
                &capability,
            ]
            .map(String::from)
            .to_vec(),
        )),
    }
}

fn regex_escape(text: &str) -> String {
    text.chars()
        .flat_map(|c| match c {
            '.' | '+' | '*' | '?' | '(' | ')' | '[' | ']' | '{' | '}' | '|' | '^' | '$' | '\\' => {
                vec!['\\', c]
            }
            other => vec![other],
        })
        .collect()
}

pub fn parse(manager: PackageManager, soname: &str, output: &str) -> Option<String> {
    let candidates = match manager {
        PackageManager::Dnf => parse_lines(output),
        PackageManager::Apt => parse_apt_file(output),
        PackageManager::Pacman => parse_pacman(soname, output),
        PackageManager::Zypper => parse_zypper(output),
    };
    preferred(candidates)
}

fn parse_lines(output: &str) -> Vec<String> {
    output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect()
}

fn parse_apt_file(output: &str) -> Vec<String> {
    output
        .lines()
        .filter_map(|line| line.split_once(": "))
        .filter(|(_, path)| in_library_directory(path.trim()))
        .map(|(package, _)| package.trim().to_string())
        .collect()
}

fn parse_pacman(soname: &str, output: &str) -> Vec<String> {
    output
        .lines()
        .filter_map(|line| {
            let mut fields = line.split('\0');
            let (_repo, package, _version, path) = (
                fields.next()?,
                fields.next()?,
                fields.next()?,
                fields.next()?,
            );
            let is_the_library = path.rsplit('/').next().is_some_and(|name| name == soname);
            (is_the_library && in_library_directory(path)).then(|| package.to_string())
        })
        .collect()
}

fn in_library_directory(path: &str) -> bool {
    let Some((directory, _)) = path.trim_start_matches('/').rsplit_once('/') else {
        return false;
    };
    const PLAIN: [&str; 6] = [
        "lib",
        "lib64",
        "usr/lib",
        "usr/lib64",
        "usr/local/lib",
        "usr/local/lib64",
    ];
    const ROOTS: [&str; 3] = ["lib/", "usr/lib/", "usr/local/lib/"];
    PLAIN.contains(&directory)
        || ROOTS.iter().any(|root| {
            directory
                .strip_prefix(root)
                .is_some_and(|rest| !rest.contains('/') && rest.contains("-linux-"))
        })
}

fn parse_zypper(output: &str) -> Vec<String> {
    output
        .lines()
        .skip_while(|line| !line.starts_with("---"))
        .skip(1)
        .filter_map(|line| line.split('|').nth(1).map(|name| name.trim().to_string()))
        .filter(|name| !name.is_empty())
        .collect()
}

fn preferred(mut candidates: Vec<String>) -> Option<String> {
    candidates.retain(|name| !IGNORED_SUFFIXES.iter().any(|suffix| name.ends_with(suffix)));
    candidates.sort();
    candidates.dedup();
    candidates.into_iter().next()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dnf_prints_one_package_name_per_line() {
        assert_eq!(
            parse(PackageManager::Dnf, "libcrypt.so.1", "libxcrypt-compat\n"),
            Some("libxcrypt-compat".into())
        );
        assert_eq!(parse(PackageManager::Dnf, "libx.so.9", ""), None);
    }

    #[test]
    fn apt_file_lines_name_the_package_before_the_colon() {
        let output = "libgtk2.0-0t64: /usr/lib/x86_64-linux-gnu/libgdk-x11-2.0.so.0\n";
        assert_eq!(
            parse(PackageManager::Apt, "libgdk-x11-2.0.so.0", output),
            Some("libgtk2.0-0t64".into())
        );
        assert_eq!(parse(PackageManager::Apt, "libx.so.9", ""), None);
    }

    #[test]
    fn a_test_package_with_its_own_copy_is_not_chosen() {
        let output = "dbus-tests: /usr/lib/dbus-1.0/debug-build/lib/libdbus-1.so.3\nlibdbus-1-3: /usr/lib/x86_64-linux-gnu/libdbus-1.so.3\n";
        assert_eq!(
            parse(PackageManager::Apt, "libdbus-1.so.3", output),
            Some("libdbus-1-3".into())
        );
        let only_the_copy = "dbus-tests: /usr/lib/dbus-1.0/debug-build/lib/libdbus-1.so.3\n";
        assert_eq!(
            parse(PackageManager::Apt, "libdbus-1.so.3", only_the_copy),
            None
        );
    }

    #[test]
    fn standard_library_directories() {
        for path in [
            "/usr/lib/libx.so.1",
            "usr/lib/libx.so.1",
            "/lib64/libx.so.1",
            "/usr/lib64/libx.so.1",
            "/usr/lib/x86_64-linux-gnu/libx.so.1",
            "/lib/aarch64-linux-gnu/libx.so.1",
            "/usr/local/lib/libx.so.1",
        ] {
            assert!(in_library_directory(path), "{path}");
        }
        for path in [
            "libx.so.1",
            "/usr/lib/dbus-1.0/debug-build/lib/libx.so.1",
            "/usr/lib/x86_64-linux-gnu/sub/libx.so.1",
            "/opt/app/lib/libx.so.1",
            "/usr/share/libx.so.1",
            "/usr/lib/python3/libx.so.1",
        ] {
            assert!(!in_library_directory(path), "{path}");
        }
    }

    #[test]
    fn an_unsynced_package_database_is_recognised_from_the_real_messages() {
        assert_eq!(
            unavailable_reason(
                PackageManager::Apt,
                "E: The cache is empty. You need to run \"apt-file update\" first.\n"
            ),
            Some(APT_FILE_EMPTY)
        );
        assert_eq!(
            unavailable_reason(
                PackageManager::Pacman,
                "warning: database file for 'core' does not exist (use '-Fy' to download)\n"
            ),
            Some(PACMAN_FILES_MISSING)
        );
        assert_eq!(
            unavailable_reason(PackageManager::Dnf, "cache is empty"),
            None
        );
        assert_eq!(unavailable_reason(PackageManager::Apt, ""), None);
    }

    #[test]
    fn pacman_machine_readable_output_is_nul_separated() {
        let output = "core\x00libxcrypt-compat\x004.5.2-1\x00usr/lib/libcrypt.so.1\n";
        assert_eq!(
            parse(PackageManager::Pacman, "libcrypt.so.1", output),
            Some("libxcrypt-compat".into())
        );
    }

    #[test]
    fn pacman_matches_the_file_name_not_a_longer_path() {
        let output = "extra\x00other\x001-1\x00usr/share/doc/libcrypt.so.1.txt\n";
        assert_eq!(parse(PackageManager::Pacman, "libcrypt.so.1", output), None);
        let wrong_directory = "extra\x00other\x001-1\x00usr/share/x/libcrypt.so.1\n";
        assert_eq!(
            parse(PackageManager::Pacman, "libcrypt.so.1", wrong_directory),
            None
        );
    }

    #[test]
    fn zypper_prints_a_table() {
        let output = "\nS  | Name      | Summary                                                  | Type\n---+-----------+----------------------------------------------------------+--------\ni  | libcrypt1 | Extended crypt library for DES, MD5, Blowfish and others | package\n";
        assert_eq!(
            parse(PackageManager::Zypper, "libcrypt.so.1", output),
            Some("libcrypt1".into())
        );
        assert_eq!(
            parse(
                PackageManager::Zypper,
                "libx.so.9",
                "No matching items found.\n"
            ),
            None
        );
    }

    #[test]
    fn development_and_debug_packages_are_never_suggested() {
        let output = "libfoo-devel\nlibfoo-debuginfo\nlibfoo\n";
        assert_eq!(
            parse(PackageManager::Dnf, "libfoo.so.1", output),
            Some("libfoo".into())
        );
        assert_eq!(
            parse(PackageManager::Dnf, "libfoo.so.1", "libfoo-devel\n"),
            None
        );
    }

    #[test]
    fn the_choice_is_deterministic() {
        assert_eq!(
            parse(PackageManager::Dnf, "x", "zeta\nalpha\nalpha\n"),
            Some("alpha".into())
        );
    }

    #[test]
    fn dnf_asks_for_the_64_bit_capability_only_for_64_bit_files() {
        let (_, wide) = lookup_command(PackageManager::Dnf, "libcrypt.so.1", true).unwrap();
        assert_eq!(wide.last().unwrap(), "libcrypt.so.1()(64bit)");
        let (_, narrow) = lookup_command(PackageManager::Dnf, "libcrypt.so.1", false).unwrap();
        assert_eq!(narrow.last().unwrap(), "libcrypt.so.1");
    }

    #[test]
    fn apt_file_patterns_escape_the_dots() {
        let escaped = regex_escape("libstdc++.so.6");
        assert_eq!(escaped, "libstdc\\+\\+\\.so\\.6");
    }
}
