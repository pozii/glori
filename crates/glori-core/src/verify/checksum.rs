use std::{
    fs::{self, File},
    io::Read,
    path::Path,
};

use sha2::{Digest, Sha256};

use crate::error::{Error, Result};

const MAX_SIDECAR_BYTES: u64 = 4 * 1024 * 1024;
const SIDECAR_SUFFIXES: [&str; 3] = [".sha256", ".sha256sum", ".sha256.txt"];
const SHARED_FILES: [&str; 5] = [
    "SHA256SUMS",
    "SHA256SUMS.txt",
    "sha256sums",
    "sha256sums.txt",
    "checksums.txt",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    PerFile,
    Shared,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    pub digest: String,
    pub source: String,
}

pub fn is_sha256(text: &str) -> bool {
    text.len() == 64 && text.chars().all(|c| c.is_ascii_hexdigit())
}

pub fn sha256_of(file: &Path) -> Result<String> {
    let mut reader = File::open(file).map_err(Error::io(file))?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 64 * 1024];
    loop {
        let read = reader.read(&mut buffer).map_err(Error::io(file))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

pub fn parse_expected(content: &str, file_name: &str, scope: Scope) -> Option<String> {
    let entries: Vec<(String, Option<String>)> = content.lines().filter_map(parse_line).collect();
    if let (Scope::PerFile, [(digest, _)]) = (scope, entries.as_slice()) {
        return Some(digest.clone());
    }
    entries
        .into_iter()
        .find(|(_, name)| {
            name.as_deref()
                .is_some_and(|name| same_file(name, file_name))
        })
        .map(|(digest, _)| digest)
}

fn parse_line(line: &str) -> Option<(String, Option<String>)> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }
    if is_sha256(line) {
        return Some((line.to_lowercase(), None));
    }
    if let Some(rest) = line.strip_prefix("SHA256 (") {
        let (name, digest) = rest.rsplit_once(") = ")?;
        return is_sha256(digest.trim())
            .then(|| (digest.trim().to_lowercase(), Some(name.to_string())));
    }
    let (digest, rest) = line.split_once(char::is_whitespace)?;
    is_sha256(digest).then(|| {
        (
            digest.to_lowercase(),
            Some(rest.trim().trim_start_matches('*').to_string()),
        )
    })
}

fn same_file(listed: &str, file_name: &str) -> bool {
    Path::new(listed)
        .file_name()
        .is_some_and(|name| name == file_name)
}

pub fn discover(file: &Path) -> Option<Found> {
    let name = file.file_name()?.to_string_lossy().into_owned();
    let dir = file
        .parent()
        .filter(|dir| !dir.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let per_file = SIDECAR_SUFFIXES
        .iter()
        .map(|suffix| (format!("{name}{suffix}"), Scope::PerFile));
    let shared = SHARED_FILES
        .iter()
        .map(|candidate| (candidate.to_string(), Scope::Shared));
    per_file.chain(shared).find_map(|(candidate, scope)| {
        let content = read_small(&dir.join(&candidate))?;
        let digest = parse_expected(&content, &name, scope)?;
        Some(Found {
            digest,
            source: candidate,
        })
    })
}

fn read_small(path: &Path) -> Option<String> {
    let meta = fs::metadata(path).ok()?;
    if !meta.is_file() || meta.len() > MAX_SIDECAR_BYTES {
        return None;
    }
    fs::read_to_string(path).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    const ABC: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
    const OTHER: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

    #[test]
    fn hashes_a_known_input() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("abc");
        fs::write(&file, "abc").unwrap();
        assert_eq!(sha256_of(&file).unwrap(), ABC);
    }

    #[test]
    fn hashes_the_empty_file() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("empty");
        fs::write(&file, "").unwrap();
        assert_eq!(sha256_of(&file).unwrap(), OTHER);
    }

    #[test]
    fn hashes_a_file_larger_than_the_buffer() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("big");
        fs::write(&file, vec![b'a'; 200_000]).unwrap();
        assert_eq!(sha256_of(&file).unwrap().len(), 64);
    }

    #[test]
    fn recognises_digests() {
        assert!(is_sha256(ABC));
        assert!(is_sha256(&ABC.to_uppercase()));
        assert!(!is_sha256("abc"));
        assert!(!is_sha256(&format!("{ABC}0")));
        assert!(!is_sha256(&ABC.replace('a', "g")));
    }

    #[test]
    fn bare_digest_in_a_per_file_sidecar() {
        assert_eq!(
            parse_expected(&format!("{ABC}\n"), "x", Scope::PerFile),
            Some(ABC.into())
        );
    }

    #[test]
    fn gnu_format_binary_and_text_markers() {
        let content = format!("{ABC}  x.tar.gz\n{OTHER} *y.tar.gz\n");
        assert_eq!(
            parse_expected(&content, "x.tar.gz", Scope::Shared),
            Some(ABC.into())
        );
        assert_eq!(
            parse_expected(&content, "y.tar.gz", Scope::Shared),
            Some(OTHER.into())
        );
    }

    #[test]
    fn paths_in_listings_are_reduced_to_the_file_name() {
        let content = format!("{ABC}  ./dist/x.tar.gz\n");
        assert_eq!(
            parse_expected(&content, "x.tar.gz", Scope::Shared),
            Some(ABC.into())
        );
    }

    #[test]
    fn bsd_format() {
        let content = format!("SHA256 (x.tar.gz) = {ABC}\n");
        assert_eq!(
            parse_expected(&content, "x.tar.gz", Scope::Shared),
            Some(ABC.into())
        );
    }

    #[test]
    fn uppercase_digests_are_normalised() {
        let content = format!("{}  x\n", ABC.to_uppercase());
        assert_eq!(
            parse_expected(&content, "x", Scope::Shared),
            Some(ABC.into())
        );
    }

    #[test]
    fn a_shared_listing_never_matches_another_file() {
        let content = format!("{ABC}  other.tar.gz\n");
        assert_eq!(parse_expected(&content, "x.tar.gz", Scope::Shared), None);
    }

    #[test]
    fn a_bare_digest_is_not_accepted_in_a_shared_listing() {
        assert_eq!(
            parse_expected(&format!("{ABC}\n"), "x", Scope::Shared),
            None
        );
    }

    #[test]
    fn a_per_file_sidecar_may_name_the_original_file() {
        let content = format!("{ABC}  original-name.tar.gz\n");
        assert_eq!(
            parse_expected(&content, "renamed.tar.gz", Scope::PerFile),
            Some(ABC.into())
        );
    }

    #[test]
    fn a_per_file_sidecar_with_many_entries_still_needs_a_name_match() {
        let content = format!("{ABC}  a\n{OTHER}  b\n");
        assert_eq!(
            parse_expected(&content, "b", Scope::PerFile),
            Some(OTHER.into())
        );
        assert_eq!(parse_expected(&content, "zzz", Scope::PerFile), None);
    }

    #[test]
    fn comments_blank_lines_and_junk_are_ignored() {
        let content = format!("# release\n\nnot a checksum line\n{ABC}  x\n");
        assert_eq!(
            parse_expected(&content, "x", Scope::Shared),
            Some(ABC.into())
        );
    }

    #[test]
    fn truncated_digests_are_ignored() {
        assert_eq!(parse_expected("abcd1234  x\n", "x", Scope::Shared), None);
    }

    #[test]
    fn discovers_a_per_file_sidecar_first() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("x.tar.gz");
        fs::write(&file, "data").unwrap();
        fs::write(
            dir.path().join("x.tar.gz.sha256"),
            format!("{ABC}  x.tar.gz\n"),
        )
        .unwrap();
        fs::write(
            dir.path().join("SHA256SUMS"),
            format!("{OTHER}  x.tar.gz\n"),
        )
        .unwrap();
        let found = discover(&file).unwrap();
        assert_eq!(found.digest, ABC);
        assert_eq!(found.source, "x.tar.gz.sha256");
    }

    #[test]
    fn falls_back_to_a_shared_listing() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("x.tar.gz");
        fs::write(&file, "data").unwrap();
        fs::write(
            dir.path().join("SHA256SUMS"),
            format!("{OTHER}  y\n{ABC}  x.tar.gz\n"),
        )
        .unwrap();
        let found = discover(&file).unwrap();
        assert_eq!(found.digest, ABC);
        assert_eq!(found.source, "SHA256SUMS");
    }

    #[test]
    fn nothing_is_found_when_nothing_is_there() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("x.tar.gz");
        fs::write(&file, "data").unwrap();
        assert_eq!(discover(&file), None);
    }

    #[test]
    fn oversized_sidecars_are_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("x");
        fs::write(&file, "data").unwrap();
        let mut huge = format!("{ABC}  x\n");
        huge.push_str(&"#".repeat(5 * 1024 * 1024));
        fs::write(dir.path().join("x.sha256"), huge).unwrap();
        assert_eq!(discover(&file), None);
    }
}
