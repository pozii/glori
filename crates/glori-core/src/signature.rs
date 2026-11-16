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
        .arg("-Kv")
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
    let text = format!("{stdout}\n{stderr}").to_lowercase();
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    let flagged = |marker: &str| lines.iter().any(|line| line.contains(marker));
    let passed: Vec<&str> = lines
        .iter()
        .copied()
        .filter(|line| line.ends_with(": ok"))
        .collect();
    if flagged(": bad") || flagged("nokey") || flagged("notrusted") || flagged("not ok") {
        Trust::Invalid
    } else if passed.iter().any(|line| line.contains("signature")) {
        Trust::Verified
    } else if !passed.is_empty() {
        Trust::Unsigned
    } else if success {
        Trust::Unverifiable
    } else {
        Trust::Invalid
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIGNED: &str = "sl.rpm:
    Header OpenPGP V4 RSA/SHA256 signature, key fingerprint: 4f50a6114cd5c6976a7f1179655a4b02f577861e: OK
    Header SHA256 digest: OK
    Payload SHA256 digest: OK
";

    const UNSIGNED: &str = "unsigned.rpm:
    Header OpenPGP DSA signature: NOTFOUND
    Header OpenPGP RSA signature: NOTFOUND
    Header OpenPGP signature: NOTFOUND
    Header SHA256 digest: OK
    Payload SHA256 digest: OK
    Legacy OpenPGP DSA signature: NOTFOUND
    Legacy OpenPGP RSA signature: NOTFOUND
";

    const CORRUPTED: &str = "bad.rpm:
    Header OpenPGP V4 RSA/SHA256 signature, key fingerprint: 4f50a6114cd5c6976a7f1179655a4b02f577861e: OK
    Header SHA256 digest: OK
    Payload SHA512 digest: NOTFOUND
    Payload SHA256 digest: BAD (Expected 74444c != 4b8fb2)
";

    const LEGACY_SIGNED: &str = "old.rpm:
    Header V4 RSA/SHA256 Signature, key ID 8e1431d5: OK
    Header SHA1 digest: OK
    Payload SHA256 digest: OK
    V4 RSA/SHA256 Signature, key ID 8e1431d5: OK
    MD5 digest: OK
";

    const LEGACY_UNSIGNED: &str = "old.rpm:
    Header SHA1 digest: OK
    Payload SHA256 digest: OK
    MD5 digest: OK
";

    const UNKNOWN_KEY: &str = "third.rpm:
    Header V4 RSA/SHA256 Signature, key ID 12345678: NOKEY
    Header SHA1 digest: OK
    Payload SHA256 digest: OK
";

    #[test]
    fn signed_package_is_verified() {
        assert_eq!(parse_rpm_checksig(SIGNED, "", true), Trust::Verified);
        assert_eq!(parse_rpm_checksig(LEGACY_SIGNED, "", true), Trust::Verified);
    }

    #[test]
    fn package_without_signature_is_unsigned_even_when_rpm_exits_with_an_error() {
        assert_eq!(parse_rpm_checksig(UNSIGNED, "", false), Trust::Unsigned);
        assert_eq!(
            parse_rpm_checksig(LEGACY_UNSIGNED, "", true),
            Trust::Unsigned
        );
    }

    #[test]
    fn a_bad_digest_is_invalid_even_when_the_signature_line_is_ok() {
        assert_eq!(parse_rpm_checksig(CORRUPTED, "", false), Trust::Invalid);
    }

    #[test]
    fn an_unknown_key_is_invalid() {
        assert_eq!(parse_rpm_checksig(UNKNOWN_KEY, "", false), Trust::Invalid);
    }

    #[test]
    fn unreadable_package_is_invalid() {
        assert_eq!(parse_rpm_checksig("fake.rpm:\n", "", false), Trust::Invalid);
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
