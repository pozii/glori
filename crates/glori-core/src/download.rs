use std::{
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
};

use tempfile::TempDir;

use crate::{
    error::{Error, Result},
    tools,
};

const MAX_SIDECAR_BYTES: &str = "4194304";
const CHECKSUM_SUFFIXES: [&str; 2] = [".sha256", ".sha256sum"];
const SIGNATURE_SUFFIXES: [&str; 2] = [".asc", ".sig"];
const SHARED_CHECKSUMS: [&str; 2] = ["SHA256SUMS", "sha256sums"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Progress {
    Show,
    Hide,
}

#[derive(Debug)]
pub struct Download {
    _dir: TempDir,
    file: PathBuf,
}

impl Download {
    pub fn path(&self) -> &Path {
        &self.file
    }
}

pub fn is_url(text: &str) -> bool {
    let lower = text.get(..8).unwrap_or(text).to_ascii_lowercase();
    lower.starts_with("https://") || lower.starts_with("http://")
}

pub fn file_name(url: &str) -> Result<String> {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    let path = rest
        .split(['?', '#'])
        .next()
        .and_then(|authority_and_path| authority_and_path.split_once('/'))
        .map_or("", |(_, path)| path);
    let name = path
        .rsplit('/')
        .next()
        .map(percent_decode)
        .unwrap_or_default();
    let invalid =
        name.is_empty() || name == "." || name == ".." || name.contains(['/', '\0', '\\']);
    if invalid {
        return Err(Error::NoFileName(url.to_string()));
    }
    Ok(name)
}

pub fn fetch(url: &str, progress: Progress, has_checksum: bool) -> Result<Download> {
    let name = file_name(url)?;
    let dir = tempfile::Builder::new()
        .prefix("glori-download-")
        .tempdir()
        .map_err(Error::io(&std::env::temp_dir()))?;
    let file = dir.path().join(&name);
    curl(url, &file, progress, None)?;
    if !has_checksum {
        let checksums = CHECKSUM_SUFFIXES
            .iter()
            .map(|suffix| format!("{name}{suffix}"))
            .chain(SHARED_CHECKSUMS.iter().map(|shared| shared.to_string()));
        fetch_first(url, dir.path(), checksums);
    }
    let signatures = SIGNATURE_SUFFIXES
        .iter()
        .map(|suffix| format!("{name}{suffix}"));
    fetch_first(url, dir.path(), signatures);
    Ok(Download { _dir: dir, file })
}

fn fetch_first(url: &str, dir: &Path, candidates: impl Iterator<Item = String>) {
    for candidate in candidates {
        let Ok(sibling) = sibling_url(url, &candidate) else {
            return;
        };
        let target = dir.join(&candidate);
        if curl(&sibling, &target, Progress::Hide, Some(MAX_SIDECAR_BYTES)).is_ok() {
            return;
        }
        let _ = std::fs::remove_file(&target);
    }
}

fn sibling_url(url: &str, name: &str) -> Result<String> {
    let base = url.split(['?', '#']).next().unwrap_or(url);
    let (prefix, _) = base
        .rsplit_once('/')
        .ok_or_else(|| Error::NoFileName(url.to_string()))?;
    Ok(format!("{prefix}/{name}"))
}

fn command(url: &str) -> Result<Command> {
    let curl = tools::find("curl").ok_or_else(|| Error::Download {
        url: url.to_string(),
        reason: "curl was not found; install it to download from URLs".to_string(),
    })?;
    let mut command = Command::new(curl);
    command
        .args(["--fail", "--location", "--max-redirs", "10"])
        .args(["--connect-timeout", "30"])
        .args(["--proto", "=http,https", "--proto-redir", "=http,https"])
        .stdin(Stdio::null());
    Ok(command)
}

fn run(mut command: Command, url: &str) -> Result<Output> {
    let output = command
        .arg("--")
        .arg(url)
        .output()
        .map_err(|source| Error::Spawn {
            command: "curl".to_string(),
            source,
        })?;
    if output.status.success() {
        return Ok(output);
    }
    let reason = String::from_utf8_lossy(&output.stderr).trim().to_string();
    Err(Error::Download {
        url: url.to_string(),
        reason: if reason.is_empty() {
            format!("curl exited with {}", output.status)
        } else {
            reason
        },
    })
}

fn curl(url: &str, target: &Path, progress: Progress, max_bytes: Option<&str>) -> Result<()> {
    let mut command = command(url)?;
    command.arg("--output").arg(target);
    match progress {
        Progress::Show => command.arg("--progress-bar").stderr(Stdio::inherit()),
        Progress::Hide => command
            .args(["--silent", "--show-error"])
            .stderr(Stdio::piped()),
    };
    if let Some(max_bytes) = max_bytes {
        command.args(["--max-filesize", max_bytes]);
    }
    run(command, url).map(drop)
}

pub fn get(url: &str, max_bytes: u64) -> Result<Vec<u8>> {
    let mut command = command(url)?;
    command
        .args(["--silent", "--show-error"])
        .args(["--max-filesize", &max_bytes.to_string()])
        .args(["--header", "Accept: application/vnd.github+json"])
        .stderr(Stdio::piped());
    run(command, url).map(|output| output.stdout)
}

fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        let hex = bytes
            .get(index + 1..index + 3)
            .and_then(|pair| std::str::from_utf8(pair).ok())
            .and_then(|pair| u8::from_str_radix(pair, 16).ok());
        match (bytes[index], hex) {
            (b'%', Some(value)) => {
                decoded.push(value);
                index += 3;
            }
            (byte, _) => {
                decoded.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8_lossy(&decoded).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognises_http_and_https_urls_only() {
        assert!(is_url("https://example.com/a.deb"));
        assert!(is_url("HTTP://example.com/a.deb"));
        assert!(!is_url("ftp://example.com/a.deb"));
        assert!(!is_url("./a.deb"));
        assert!(!is_url("/tmp/https://a"));
        assert!(!is_url("https"));
    }

    #[test]
    fn takes_the_last_path_segment() {
        assert_eq!(
            file_name("https://h.org/a/b/tool-1.2.tar.gz").unwrap(),
            "tool-1.2.tar.gz"
        );
    }

    #[test]
    fn ignores_query_and_fragment() {
        assert_eq!(
            file_name("https://h.org/dl/app.deb?token=a/b#x").unwrap(),
            "app.deb"
        );
    }

    #[test]
    fn decodes_percent_escapes() {
        assert_eq!(
            file_name("https://h.org/My%20App.AppImage").unwrap(),
            "My App.AppImage"
        );
    }

    #[test]
    fn leaves_broken_escapes_alone() {
        assert_eq!(file_name("https://h.org/a%zz.deb").unwrap(), "a%zz.deb");
        assert_eq!(file_name("https://h.org/a%2").unwrap(), "a%2");
    }

    #[test]
    fn rejects_urls_without_a_file_name() {
        for url in [
            "https://h.org",
            "https://h.org/",
            "https://h.org/dir/",
            "https://h.org/?file=a.deb",
            "https://h.org/..",
            "https://h.org/a%2F..%2F..",
            "https://h.org/%2e%2e",
            "https://h.org/a%00b",
        ] {
            assert!(file_name(url).is_err(), "{url}");
        }
    }

    #[test]
    fn encoded_separators_never_escape_the_directory() {
        assert!(file_name("https://h.org/..%2Fetc%2Fpasswd").is_err());
    }

    #[test]
    fn builds_sibling_urls_next_to_the_file() {
        assert_eq!(
            sibling_url("https://h.org/a/app.deb?x=1", "app.deb.sha256").unwrap(),
            "https://h.org/a/app.deb.sha256"
        );
    }
}
