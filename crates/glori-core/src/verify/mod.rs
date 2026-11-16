pub mod checksum;
mod gpg;
mod rpm;

use std::path::Path;

use serde::Serialize;

use crate::format::Format;

const NO_EVIDENCE: &str =
    "no .sha256 or .sig file found next to it; use --sha256 to check against a known hash";

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
            Trust::Unverifiable => "not verified",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Verdict {
    pub trust: Trust,
    pub detail: Option<String>,
}

impl Verdict {
    pub fn new(trust: Trust, detail: impl Into<String>) -> Self {
        Verdict {
            trust,
            detail: Some(detail.into()),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Options {
    pub sha256: Option<String>,
}

pub fn check(format: Format, file: &Path, options: &Options) -> Verdict {
    let mut verdicts = Vec::new();
    if format == Format::Rpm {
        verdicts.push(rpm::check(file));
    }
    verdicts.extend(checksum_verdict(file, options));
    verdicts.extend(signature_verdict(file));
    if verdicts.is_empty() {
        return Verdict::new(Trust::Unverifiable, NO_EVIDENCE);
    }
    combine(verdicts)
}

fn checksum_verdict(file: &Path, options: &Options) -> Option<Verdict> {
    let (expected, source) = match &options.sha256 {
        Some(digest) => (digest.to_lowercase(), "--sha256".to_string()),
        None => {
            let found = checksum::discover(file)?;
            (found.digest, found.source)
        }
    };
    Some(match checksum::sha256_of(file) {
        Ok(actual) if actual == expected => {
            Verdict::new(Trust::Verified, format!("sha256 matches {source}"))
        }
        Ok(actual) => Verdict::new(
            Trust::Invalid,
            format!("sha256 does not match {source}: expected {expected}, got {actual}"),
        ),
        Err(error) => Verdict::new(Trust::Unverifiable, error.to_string()),
    })
}

fn signature_verdict(file: &Path) -> Option<Verdict> {
    let signature = gpg::find_signature(file)?;
    let name = signature
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    Some(match gpg::verify(&signature, file) {
        gpg::Outcome::Good(signer) => Verdict::new(
            Trust::Verified,
            format!("good gpg signature by {signer} ({name})"),
        ),
        gpg::Outcome::Bad(reason) => Verdict::new(Trust::Invalid, format!("{reason} ({name})")),
        gpg::Outcome::NotOpenPgp => Verdict::new(
            Trust::Unverifiable,
            format!("{name} is not an OpenPGP signature"),
        ),
        gpg::Outcome::NotInstalled => Verdict::new(
            Trust::Unverifiable,
            format!("found {name} but gpg is not installed"),
        ),
    })
}

fn combine(verdicts: Vec<Verdict>) -> Verdict {
    let trust = [Trust::Invalid, Trust::Unsigned, Trust::Verified]
        .into_iter()
        .find(|wanted| verdicts.iter().any(|verdict| verdict.trust == *wanted))
        .unwrap_or(Trust::Unverifiable);
    let details: Vec<String> = verdicts
        .into_iter()
        .filter(|verdict| trust != Trust::Invalid || verdict.trust == Trust::Invalid)
        .filter_map(|verdict| verdict.detail)
        .collect();
    Verdict {
        trust,
        detail: (!details.is_empty()).then(|| details.join("; ")),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    const ABC: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

    fn verdict(trust: Trust, detail: &str) -> Verdict {
        Verdict::new(trust, detail)
    }

    fn abc_file(dir: &Path) -> std::path::PathBuf {
        let file = dir.join("x.tar.gz");
        fs::write(&file, "abc").unwrap();
        file
    }

    #[test]
    fn nothing_to_go_on_is_not_verified_and_says_how_to_fix_it() {
        let dir = tempfile::tempdir().unwrap();
        let result = check(Format::TarGz, &abc_file(dir.path()), &Options::default());
        assert_eq!(result.trust, Trust::Unverifiable);
        assert!(result.detail.unwrap().contains("--sha256"));
    }

    #[test]
    fn a_matching_flag_verifies() {
        let dir = tempfile::tempdir().unwrap();
        let options = Options {
            sha256: Some(ABC.to_uppercase()),
        };
        let result = check(Format::TarGz, &abc_file(dir.path()), &options);
        assert_eq!(result.trust, Trust::Verified);
        assert!(result.detail.unwrap().contains("--sha256"));
    }

    #[test]
    fn a_wrong_flag_fails_and_shows_both_digests() {
        let dir = tempfile::tempdir().unwrap();
        let options = Options {
            sha256: Some("0".repeat(64)),
        };
        let result = check(Format::TarGz, &abc_file(dir.path()), &options);
        assert_eq!(result.trust, Trust::Invalid);
        let detail = result.detail.unwrap();
        assert!(detail.contains(&"0".repeat(64)) && detail.contains(ABC));
    }

    #[test]
    fn a_sidecar_is_used_without_any_flag() {
        let dir = tempfile::tempdir().unwrap();
        let file = abc_file(dir.path());
        fs::write(
            dir.path().join("x.tar.gz.sha256"),
            format!("{ABC}  x.tar.gz\n"),
        )
        .unwrap();
        let result = check(Format::TarGz, &file, &Options::default());
        assert_eq!(result.trust, Trust::Verified);
        assert!(result.detail.unwrap().contains("x.tar.gz.sha256"));
    }

    #[test]
    fn the_flag_takes_precedence_over_a_sidecar() {
        let dir = tempfile::tempdir().unwrap();
        let file = abc_file(dir.path());
        fs::write(
            dir.path().join("x.tar.gz.sha256"),
            format!("{}  x\n", "0".repeat(64)),
        )
        .unwrap();
        let options = Options {
            sha256: Some(ABC.into()),
        };
        assert_eq!(check(Format::TarGz, &file, &options).trust, Trust::Verified);
    }

    #[test]
    fn a_tampered_file_fails_against_its_sidecar() {
        let dir = tempfile::tempdir().unwrap();
        let file = abc_file(dir.path());
        fs::write(
            dir.path().join("x.tar.gz.sha256"),
            format!("{ABC}  x.tar.gz\n"),
        )
        .unwrap();
        fs::write(&file, "abd").unwrap();
        assert_eq!(
            check(Format::TarGz, &file, &Options::default()).trust,
            Trust::Invalid
        );
    }

    #[test]
    fn any_failure_outranks_any_success() {
        let combined = combine(vec![
            verdict(Trust::Verified, "good"),
            verdict(Trust::Invalid, "bad"),
            verdict(Trust::Unsigned, "unsigned"),
        ]);
        assert_eq!(combined.trust, Trust::Invalid);
        assert_eq!(combined.detail.as_deref(), Some("bad"));
    }

    #[test]
    fn unsigned_outranks_a_checksum_match() {
        let combined = combine(vec![
            verdict(Trust::Verified, "sha256 matches"),
            verdict(Trust::Unsigned, "no signature"),
        ]);
        assert_eq!(combined.trust, Trust::Unsigned);
    }

    #[test]
    fn a_success_is_not_masked_by_a_check_that_could_not_run() {
        let combined = combine(vec![
            verdict(Trust::Unverifiable, "gpg is not installed"),
            verdict(Trust::Verified, "sha256 matches"),
        ]);
        assert_eq!(combined.trust, Trust::Verified);
        assert_eq!(
            combined.detail.as_deref(),
            Some("gpg is not installed; sha256 matches")
        );
    }

    #[test]
    fn labels() {
        assert_eq!(Trust::Unverifiable.label(), "not verified");
        assert_eq!(Trust::Invalid.label(), "verification failed");
    }
}
