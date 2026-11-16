mod common;

use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    process::{Command, Output, Stdio},
};

use common::*;
use tempfile::TempDir;

const ARCH: &str = std::env::consts::ARCH;

fn script(version: &str) -> Vec<u8> {
    format!("#!/bin/sh\necho glori {version}\n").into_bytes()
}

fn release_tarball(version: &str, binary: &[u8]) -> Vec<u8> {
    tarball(|builder| {
        add_dir(builder, &format!("glori-{version}/"));
        add_file(builder, &format!("glori-{version}/glori"), binary, 0o755);
        add_file(
            builder,
            &format!("glori-{version}/README.md"),
            b"docs",
            0o644,
        );
    })
}

struct Release {
    tag: String,
    archive: Vec<u8>,
    listed_arch: String,
    sums: Option<String>,
}

impl Release {
    fn new(version: &str) -> Self {
        Self::with_binary(version, &script(version))
    }

    fn with_binary(version: &str, binary: &[u8]) -> Self {
        let archive = release_tarball(version, binary);
        Release {
            tag: format!("v{version}"),
            sums: Some(format!(
                "{}  glori-{version}-{ARCH}-unknown-linux-musl.tar.gz\n",
                sha256_hex(&archive)
            )),
            archive,
            listed_arch: ARCH.to_string(),
        }
    }

    fn serve(self) -> Server {
        let version = self.tag.trim_start_matches('v').to_string();
        Server::start_with(move |base| {
            let name = format!(
                "glori-{version}-{}-unknown-linux-musl.tar.gz",
                self.listed_arch
            );
            let metadata = serde_json::json!({
                "tag_name": self.tag,
                "assets": [{
                    "name": name,
                    "browser_download_url": format!("{base}/download/{name}")
                }]
            });
            let mut files = vec![
                ("/latest".to_string(), metadata.to_string().into_bytes()),
                (format!("/download/{name}"), self.archive),
            ];
            if let Some(sums) = self.sums {
                files.push(("/download/SHA256SUMS".to_string(), sums.into_bytes()));
            }
            files
        })
    }
}

struct Installed {
    dir: TempDir,
}

impl Installed {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let installed = Installed { dir };
        fs::copy(env!("CARGO_BIN_EXE_glori"), installed.exe()).unwrap();
        installed
    }

    fn exe(&self) -> PathBuf {
        self.dir.path().join("glori")
    }

    fn run(&self, server: &Server, args: &[&str]) -> Output {
        Command::new(self.exe())
            .args(args)
            .env("HOME", self.dir.path())
            .env("GLORI_UPDATE_URL", server.url("/latest"))
            .env("NO_COLOR", "1")
            .stdin(Stdio::null())
            .output()
            .unwrap()
    }

    fn bytes(&self) -> Vec<u8> {
        fs::read(self.exe()).unwrap()
    }

    fn files(&self) -> usize {
        fs::read_dir(self.dir.path()).unwrap().count()
    }
}

#[test]
fn check_reports_a_newer_release_without_changing_anything() {
    let installed = Installed::new();
    let before = installed.bytes();
    let server = Release::new("99.0.0").serve();
    let output = installed.run(&server, &["self-update", "--check", "--json"]);
    assert!(output.status.success(), "{}", stderr(&output));
    let report: serde_json::Value = serde_json::from_str(&stdout(&output)).unwrap();
    assert_eq!(report["latest"], "99.0.0");
    assert_eq!(report["update_available"], true);
    assert_eq!(installed.bytes(), before);
}

#[test]
fn check_in_plain_text_points_to_the_update_command() {
    let installed = Installed::new();
    let server = Release::new("99.0.0").serve();
    let output = installed.run(&server, &["self-update", "--check"]);
    assert!(stdout(&output).contains("99.0.0"));
    assert!(stdout(&output).contains("glori self-update"));
}

#[test]
fn an_older_or_equal_release_means_up_to_date() {
    let installed = Installed::new();
    let server = Release::new("0.0.1").serve();
    let output = installed.run(&server, &["self-update", "--yes"]);
    assert!(output.status.success());
    assert!(stdout(&output).contains("up to date"));
    let report = installed.run(&server, &["self-update", "--check", "--json"]);
    let report: serde_json::Value = serde_json::from_str(&stdout(&report)).unwrap();
    assert_eq!(report["update_available"], false);
}

#[test]
fn updates_the_binary_in_place() {
    let installed = Installed::new();
    let server = Release::new("99.0.0").serve();
    let output = installed.run(&server, &["self-update", "--yes"]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(stdout(&output).contains("Updated Glori to 99.0.0"));
    assert_eq!(installed.bytes(), script("99.0.0"));
    let mode = fs::metadata(installed.exe()).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o755);
    assert_eq!(installed.files(), 1);
    let run = Command::new(installed.exe())
        .arg("--version")
        .output()
        .unwrap();
    assert!(stdout(&run).contains("99.0.0"));
}

#[test]
fn a_release_without_checksums_is_refused() {
    let installed = Installed::new();
    let before = installed.bytes();
    let mut release = Release::new("99.0.0");
    release.sums = None;
    let server = release.serve();
    let output = installed.run(&server, &["self-update", "--yes"]);
    assert!(!output.status.success());
    assert!(stderr(&output).contains("could not be verified"));
    assert_eq!(installed.bytes(), before);
    assert_eq!(installed.files(), 1);
}

#[test]
fn a_tampered_download_is_refused() {
    let installed = Installed::new();
    let before = installed.bytes();
    let mut release = Release::new("99.0.0");
    release.sums = Some(format!(
        "{}  glori-99.0.0-{ARCH}-unknown-linux-musl.tar.gz\n",
        "0".repeat(64)
    ));
    let server = release.serve();
    let output = installed.run(&server, &["self-update", "--yes"]);
    assert!(!output.status.success());
    assert!(stderr(&output).contains("could not be verified"));
    assert_eq!(installed.bytes(), before);
}

#[test]
fn a_binary_reporting_another_version_is_refused() {
    let installed = Installed::new();
    let before = installed.bytes();
    let server = Release::with_binary("99.0.0", &script("1.2.3")).serve();
    let output = installed.run(&server, &["self-update", "--yes"]);
    assert!(!output.status.success());
    assert!(stderr(&output).contains("did not report version 99.0.0"));
    assert_eq!(installed.bytes(), before);
    assert_eq!(installed.files(), 1);
}

#[test]
fn a_release_for_another_architecture_is_reported() {
    let installed = Installed::new();
    let mut release = Release::new("99.0.0");
    release.listed_arch = "riscv64".to_string();
    let server = release.serve();
    let output = installed.run(&server, &["self-update", "--yes"]);
    assert!(!output.status.success());
    assert!(stderr(&output).contains("has no glori-99.0.0"));
}

#[test]
fn unreadable_release_data_fails_cleanly() {
    let installed = Installed::new();
    let before = installed.bytes();
    let server = Server::start(vec![("/latest", b"<html>rate limited</html>".to_vec())]);
    let output = installed.run(&server, &["self-update", "--yes"]);
    assert!(!output.status.success());
    assert!(stderr(&output).contains("unexpected release data"));
    assert_eq!(installed.bytes(), before);
}

#[test]
fn an_unreachable_server_fails_cleanly() {
    let installed = Installed::new();
    let server = Server::start(vec![]);
    let output = installed.run(&server, &["self-update", "--check"]);
    assert!(!output.status.success());
    assert!(stderr(&output).contains("download of"));
}

#[test]
fn it_appears_in_the_help() {
    let sandbox = Sandbox::new();
    assert!(stdout(&sandbox.glori(&["--help"])).contains("self-update"));
}
