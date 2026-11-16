use std::{fs, path::Path, process::Command};

use serde::Serialize;

use crate::format::Format;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Trust {
    Verified,
    Unsigned,
    Invalid,
    Unverifiable,
}

impl Trust {
    pub fn label(self) -> &'static str {
        match self {
            Trust::Verified => "verified",
            Trust::Unsigned => "not signed",
            Trust::Invalid => "verification failed",
            Trust::Unverifiable => "cannot be checked for this format",
        }
    }
}

pub fn check(format: Format, file: &Path) -> Trust {
    match format {
        Format::Rpm => check_rpm(file),
        _ => Trust::Unverifiable,
    }
}

fn check_rpm(file: &Path) -> Trust {
    let path = std::path::absolute(file).unwrap_or_else(|_| file.to_path_buf());
    let canonical = fs::canonicalize(&path).unwrap_or(path);
    Command::new("rpm")
        .arg("-K")
        .arg(canonical)
        .env("LC_ALL", "C")
        .output()
        .map_or(Trust::Unverifiable, |output| {
            parse_rpm_checksig(
                &String::from_utf8_lossy(&output.stdout),
                &String::from_utf8_lossy(&output.stderr),
                output.status.success(),
            )
        })
}

pub fn parse_rpm_checksig(stdout: &str, stderr: &str, success: bool) -> Trust {
    let text = format!("{stdout} {stderr}").to_lowercase();
    if !success || text.contains("not ok") {
        Trust::Invalid
    } else if text.contains("signatures ok") {
        Trust::Verified
    } else if text.contains("digests ok") {
        Trust::Unsigned
    } else {
        Trust::Unverifiable
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signed_package_is_verified() {
        assert_eq!(
            parse_rpm_checksig("sl.rpm: digests signatures OK\n", "", true),
            Trust::Verified
        );
    }

    #[test]
    fn package_without_signature_is_unsigned() {
        assert_eq!(
            parse_rpm_checksig("sl.rpm: digests OK\n", "", true),
            Trust::Unsigned
        );
    }

    #[test]
    fn corrupted_or_unknown_key_is_invalid() {
        assert_eq!(
            parse_rpm_checksig("sl.rpm: DIGESTS SIGNATURES NOT OK\n", "", false),
            Trust::Invalid
        );
    }

    #[test]
    fn unreadable_package_is_invalid() {
        assert_eq!(parse_rpm_checksig("fake.rpm:", "", false), Trust::Invalid);
    }

    #[test]
    fn not_ok_wins_even_when_the_exit_code_looks_fine() {
        assert_eq!(
            parse_rpm_checksig("x: digests signatures NOT OK", "", true),
            Trust::Invalid
        );
    }

    #[test]
    fn unexpected_output_is_unverifiable() {
        assert_eq!(parse_rpm_checksig("", "", true), Trust::Unverifiable);
    }

    #[test]
    fn formats_without_embedded_signatures_are_unverifiable() {
        assert_eq!(check(Format::Deb, Path::new("a.deb")), Trust::Unverifiable);
        assert_eq!(check(Format::AppImage, Path::new("a")), Trust::Unverifiable);
        assert_eq!(
            check(Format::TarGz, Path::new("a.tar.gz")),
            Trust::Unverifiable
        );
    }
}
