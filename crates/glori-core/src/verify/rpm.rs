use std::{fs, path::Path, process::Command};

use super::{Trust, Verdict};

pub fn check(file: &Path) -> Verdict {
    let path = std::path::absolute(file).unwrap_or_else(|_| file.to_path_buf());
    let canonical = fs::canonicalize(&path).unwrap_or(path);
    Command::new("rpm")
        .arg("-Kv")
        .arg(canonical)
        .env("LC_ALL", "C")
        .output()
        .map_or_else(
            |_| {
                Verdict::new(
                    Trust::Unverifiable,
                    "rpm is not installed, so its signature was not checked",
                )
            },
            |output| {
                let trust = parse_rpm_checksig(
                    &String::from_utf8_lossy(&output.stdout),
                    &String::from_utf8_lossy(&output.stderr),
                    output.status.success(),
                );
                Verdict::new(trust, describe(trust))
            },
        )
}

fn describe(trust: Trust) -> &'static str {
    match trust {
        Trust::Verified => "RPM signature is valid",
        Trust::Unsigned => "the RPM carries no signature",
        Trust::Invalid => "the RPM signature or digests do not check out",
        Trust::Unverifiable => "rpm gave no usable answer",
    }
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

    const LEGACY_SIGNED: &str = "signed.rpm:
    Header V4 RSA/SHA256 Signature, key ID 350d275d: OK
    Header SHA256 digest: OK
    Header SHA1 digest: OK
    Payload SHA256 digest: OK
    MD5 digest: OK
";

    const LEGACY_UNSIGNED: &str = "unsigned.rpm:
    Header SHA256 digest: OK
    Header SHA1 digest: OK
    Payload SHA256 digest: OK
    MD5 digest: OK
";

    const LEGACY_CORRUPTED: &str = "bad.rpm:
    Header V4 RSA/SHA256 Signature, key ID 350d275d: BAD
    Header SHA256 digest: BAD (Expected 27dfb1 != 2dc274)
    Header SHA1 digest: BAD (Expected 5aaeec != d23cad)
    Payload SHA256 digest: OK
    MD5 digest: BAD (Expected 86571c != 576db3)
";

    const UNKNOWN_KEY: &str = "nokey.rpm:
    Header V4 RSA/SHA256 Signature, key ID 350d275d: NOKEY
    Header SHA256 digest: OK
    Header SHA1 digest: OK
    Payload SHA256 digest: OK
    MD5 digest: OK
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
        assert_eq!(
            parse_rpm_checksig(LEGACY_CORRUPTED, "", false),
            Trust::Invalid
        );
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
}
