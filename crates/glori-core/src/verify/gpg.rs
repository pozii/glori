use std::{
    path::{Path, PathBuf},
    process::Command,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Good(String),
    Bad(String),
    NotOpenPgp,
    NotInstalled,
}

pub fn find_signature(file: &Path) -> Option<PathBuf> {
    let name = file.file_name()?.to_string_lossy().into_owned();
    let dir = file
        .parent()
        .filter(|dir| !dir.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    ["sig", "asc"]
        .iter()
        .map(|extension| dir.join(format!("{name}.{extension}")))
        .find(|candidate| candidate.is_file())
}

pub fn verify(signature: &Path, file: &Path) -> Outcome {
    let absolute = |path: &Path| std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    Command::new("gpg")
        .args(["--batch", "--no-tty", "--status-fd=1", "--verify"])
        .arg(absolute(signature))
        .arg(absolute(file))
        .env("LC_ALL", "C")
        .output()
        .map_or(Outcome::NotInstalled, |output| {
            parse_status(&String::from_utf8_lossy(&output.stdout))
        })
}

pub fn parse_status(status: &str) -> Outcome {
    let mut good = None;
    let mut reason = None;
    let mut errsig = false;
    let mut no_data = false;
    for line in status.lines() {
        let Some(rest) = line.strip_prefix("[GNUPG:] ") else {
            continue;
        };
        let mut parts = rest.splitn(3, ' ');
        let keyword = parts.next().unwrap_or_default();
        let key_id = parts.next().unwrap_or_default();
        match keyword {
            "GOODSIG" => good = Some(parts.next().map_or(key_id, |user| user).to_string()),
            "BADSIG" => reason = Some("the signature does not match the file".to_string()),
            "EXPKEYSIG" => reason = Some("the signing key has expired".to_string()),
            "EXPSIG" => reason = Some("the signature has expired".to_string()),
            "REVKEYSIG" => reason = Some("the signing key was revoked".to_string()),
            "NO_PUBKEY" => {
                reason = Some(format!("the signing key {key_id} is not in your keyring"));
            }
            "ERRSIG" => errsig = true,
            "NODATA" => no_data = true,
            _ => {}
        }
    }
    if let Some(reason) = reason {
        Outcome::Bad(reason)
    } else if errsig {
        Outcome::Bad("the signature could not be checked".to_string())
    } else if no_data {
        Outcome::NotOpenPgp
    } else if let Some(signer) = good {
        Outcome::Good(signer)
    } else {
        Outcome::Bad("no valid signature was found".to_string())
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    const GOOD: &str = "[GNUPG:] NEWSIG
[GNUPG:] KEY_CONSIDERED 3EA510E9A0D6AB5D0FECB0F49BB5D4B222F48EE6 0
[GNUPG:] SIG_ID Qt1/JKVlvYjnF+QU0/6h/Pv1//Q 2026-11-16 1794857809
[GNUPG:] GOODSIG 9BB5D4B222F48EE6 Glori Test <test@example.invalid>
[GNUPG:] VALIDSIG 3EA510E9A0D6AB5D0FECB0F49BB5D4B222F48EE6 2026-11-16 1794857809 0 4 0 22 10 00 3EA510E9A0D6AB5D0FECB0F49BB5D4B222F48EE6
[GNUPG:] TRUST_ULTIMATE 0 pgp
";

    const BAD: &str = "[GNUPG:] NEWSIG
[GNUPG:] KEY_CONSIDERED 3EA510E9A0D6AB5D0FECB0F49BB5D4B222F48EE6 0
[GNUPG:] BADSIG 9BB5D4B222F48EE6 Glori Test <test@example.invalid>
[GNUPG:] FAILURE gpg-exit 33554433
";

    const UNKNOWN_KEY: &str = "[GNUPG:] NEWSIG
[GNUPG:] ERRSIG 9BB5D4B222F48EE6 22 10 00 1794857809 9 3EA510E9A0D6AB5D0FECB0F49BB5D4B222F48EE6
[GNUPG:] NO_PUBKEY 9BB5D4B222F48EE6
[GNUPG:] FAILURE gpg-exit 33554433
";

    const NOT_OPENPGP: &str = "[GNUPG:] NODATA 1
[GNUPG:] NODATA 2
[GNUPG:] FAILURE gpg-exit 33554433
";

    #[test]
    fn a_good_signature_names_the_signer() {
        assert_eq!(
            parse_status(GOOD),
            Outcome::Good("Glori Test <test@example.invalid>".into())
        );
    }

    #[test]
    fn a_bad_signature_is_bad() {
        assert!(
            matches!(parse_status(BAD), Outcome::Bad(reason) if reason.contains("does not match"))
        );
    }

    #[test]
    fn an_unknown_key_is_reported_with_its_id() {
        assert!(matches!(
            parse_status(UNKNOWN_KEY),
            Outcome::Bad(reason) if reason.contains("9BB5D4B222F48EE6") && reason.contains("keyring")
        ));
    }

    #[test]
    fn something_that_is_not_openpgp_is_not_blamed_on_the_file() {
        assert_eq!(parse_status(NOT_OPENPGP), Outcome::NotOpenPgp);
    }

    #[test]
    fn a_bad_line_wins_over_a_good_one() {
        let mixed = format!("{GOOD}{BAD}");
        assert!(matches!(parse_status(&mixed), Outcome::Bad(_)));
    }

    #[test]
    fn expired_and_revoked_keys_are_bad() {
        for (line, word) in [
            ("[GNUPG:] EXPKEYSIG AAAA user", "expired"),
            ("[GNUPG:] EXPSIG AAAA user", "expired"),
            ("[GNUPG:] REVKEYSIG AAAA user", "revoked"),
        ] {
            assert!(
                matches!(parse_status(line), Outcome::Bad(r) if r.contains(word)),
                "{line}"
            );
        }
    }

    #[test]
    fn empty_output_is_not_a_valid_signature() {
        assert!(matches!(parse_status(""), Outcome::Bad(_)));
    }

    #[test]
    fn signatures_are_found_next_to_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("x.tar.gz");
        fs::write(&file, "d").unwrap();
        assert_eq!(find_signature(&file), None);
        fs::write(dir.path().join("x.tar.gz.asc"), "s").unwrap();
        assert_eq!(find_signature(&file), Some(dir.path().join("x.tar.gz.asc")));
        fs::write(dir.path().join("x.tar.gz.sig"), "s").unwrap();
        assert_eq!(find_signature(&file), Some(dir.path().join("x.tar.gz.sig")));
    }
}
