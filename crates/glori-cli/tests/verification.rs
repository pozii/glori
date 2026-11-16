mod common;

use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use common::*;

const ZERO: &str = "0000000000000000000000000000000000000000000000000000000000000000";

fn sha256(file: &Path) -> String {
    let output = Command::new("sha256sum").arg(file).output().unwrap();
    String::from_utf8_lossy(&output.stdout)
        .split_whitespace()
        .next()
        .unwrap()
        .to_string()
}

fn tool(sandbox: &Sandbox) -> PathBuf {
    sandbox.download(CLI_TOOL, &cli_tool())
}

fn info_json(sandbox: &Sandbox, file: &Path, extra: &[&str]) -> serde_json::Value {
    let mut args = vec!["info", file.to_str().unwrap(), "--json"];
    args.extend_from_slice(extra);
    serde_json::from_str(&stdout(&sandbox.glori(&args))).unwrap()
}

#[test]
fn a_sha256_sidecar_verifies_the_download() {
    let sandbox = Sandbox::new();
    let file = tool(&sandbox);
    let digest = sha256(&file);
    sandbox.download(
        &format!("{CLI_TOOL}.sha256"),
        format!("{digest}  {CLI_TOOL}\n").as_bytes(),
    );
    let info = info_json(&sandbox, &file, &[]);
    assert_eq!(info["verification"], "verified");
    assert!(
        info["verification_detail"]
            .as_str()
            .unwrap()
            .contains(".sha256")
    );
    assert!(stdout(&sandbox.glori(&["info", file.to_str().unwrap()])).contains("verified"));
    assert!(sandbox.install(&file, &[]).status.success());
}

#[test]
fn a_shared_checksum_listing_is_found() {
    let sandbox = Sandbox::new();
    let file = tool(&sandbox);
    let listing = format!("{ZERO}  other.tar.gz\n{}  {CLI_TOOL}\n", sha256(&file));
    sandbox.download("SHA256SUMS", listing.as_bytes());
    assert_eq!(info_json(&sandbox, &file, &[])["verification"], "verified");
}

#[test]
fn a_wrong_checksum_blocks_the_install_even_with_yes() {
    let sandbox = Sandbox::new();
    let file = tool(&sandbox);
    sandbox.download(
        &format!("{CLI_TOOL}.sha256"),
        format!("{ZERO}  {CLI_TOOL}\n").as_bytes(),
    );
    let output = sandbox.install(&file, &[]);
    assert!(!output.status.success());
    let message = stderr(&output);
    assert!(message.contains("verification failed"), "{message}");
    assert!(sandbox.nothing_was_installed());
    let shown = stdout(&output);
    assert!(shown.contains("does not match"), "{shown}");
}

#[test]
fn allow_untrusted_overrides_a_checksum_mismatch() {
    let sandbox = Sandbox::new();
    let file = tool(&sandbox);
    sandbox.download(
        &format!("{CLI_TOOL}.sha256"),
        format!("{ZERO}  {CLI_TOOL}\n").as_bytes(),
    );
    assert!(
        sandbox
            .install(&file, &["--allow-untrusted"])
            .status
            .success()
    );
    assert!(sandbox.opt("rg").exists());
}

#[test]
fn a_download_changed_after_its_checksum_was_published_is_caught() {
    let sandbox = Sandbox::new();
    let file = tool(&sandbox);
    let digest = sha256(&file);
    sandbox.download(
        &format!("{CLI_TOOL}.sha256"),
        format!("{digest}  {CLI_TOOL}\n").as_bytes(),
    );
    let mut bytes = fs::read(&file).unwrap();
    let last = bytes.len() - 1;
    bytes[last] ^= 0xff;
    fs::write(&file, bytes).unwrap();
    assert_eq!(info_json(&sandbox, &file, &[])["verification"], "invalid");
    assert!(!sandbox.install(&file, &[]).status.success());
}

#[test]
fn the_sha256_flag_accepts_the_right_hash_and_rejects_the_wrong_one() {
    let sandbox = Sandbox::new();
    let file = tool(&sandbox);
    let digest = sha256(&file);
    let good = info_json(&sandbox, &file, &["--sha256", &digest.to_uppercase()]);
    assert_eq!(good["verification"], "verified");
    assert!(
        good["verification_detail"]
            .as_str()
            .unwrap()
            .contains("--sha256")
    );
    let bad = sandbox.install(&file, &["--sha256", ZERO]);
    assert!(!bad.status.success());
    assert!(sandbox.nothing_was_installed());
    assert!(
        sandbox
            .install(&file, &["--sha256", &digest])
            .status
            .success()
    );
}

#[test]
fn a_malformed_sha256_flag_is_rejected_up_front() {
    let sandbox = Sandbox::new();
    let file = tool(&sandbox);
    let output = sandbox.install(&file, &["--sha256", "abc123"]);
    assert!(!output.status.success());
    assert!(stderr(&output).contains("64 hexadecimal"));
    assert!(sandbox.nothing_was_installed());
}

#[test]
fn without_any_evidence_the_install_still_works_but_says_so() {
    let sandbox = Sandbox::new();
    let file = tool(&sandbox);
    let info = info_json(&sandbox, &file, &[]);
    assert_eq!(info["verification"], "unverifiable");
    assert!(
        info["verification_detail"]
            .as_str()
            .unwrap()
            .contains("--sha256")
    );
    assert!(sandbox.install(&file, &[]).status.success());
}

#[test]
fn an_appimage_can_be_checked_too() {
    let sandbox = Sandbox::new();
    let file = sandbox.download("Cool-Tool-1.0-x86_64.AppImage", &appimage_bytes());
    let digest = sha256(&file);
    assert_eq!(
        info_json(&sandbox, &file, &["--sha256", &digest])["verification"],
        "verified"
    );
    assert!(!sandbox.install(&file, &["--sha256", ZERO]).status.success());
}

#[test]
fn a_missing_gpg_is_reported_without_blocking_the_install() {
    let sandbox = Sandbox::new();
    let file = tool(&sandbox);
    sandbox.download(
        &format!("{CLI_TOOL}.asc"),
        b"-----BEGIN PGP SIGNATURE-----\n",
    );
    let empty = tempfile::tempdir().unwrap();
    let output = sandbox.glori_env(
        &["info", file.to_str().unwrap(), "--json"],
        &[("PATH", empty.path())],
    );
    let info: serde_json::Value = serde_json::from_str(&stdout(&output)).unwrap();
    assert_eq!(info["verification"], "unverifiable");
    assert!(
        info["verification_detail"]
            .as_str()
            .unwrap()
            .contains("gpg is not installed")
    );
}

struct Keyring {
    home: tempfile::TempDir,
}

impl Keyring {
    fn new() -> Option<Self> {
        let available = Command::new("gpg")
            .arg("--version")
            .stdout(Stdio::null())
            .status()
            .is_ok_and(|status| status.success());
        if !available {
            eprintln!("skipped: gpg is not installed");
            return None;
        }
        let home = tempfile::tempdir().unwrap();
        fs::set_permissions(
            home.path(),
            std::os::unix::fs::PermissionsExt::from_mode(0o700),
        )
        .unwrap();
        let keyring = Keyring { home };
        let status = keyring
            .gpg()
            .args([
                "--quick-generate-key",
                "Glori Test <test@example.invalid>",
                "default",
                "default",
                "never",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap();
        assert!(status.success(), "could not generate a test key");
        Some(keyring)
    }

    fn gpg(&self) -> Command {
        let mut command = Command::new("gpg");
        command
            .args(["--batch", "--quiet", "--no-tty", "--passphrase", ""])
            .env("GNUPGHOME", self.home.path());
        command
    }

    fn sign(&self, file: &Path, extension: &str) -> PathBuf {
        let signature = PathBuf::from(format!("{}.{extension}", file.display()));
        let mut command = self.gpg();
        if extension == "asc" {
            command.arg("--armor");
        }
        let status = command
            .arg("--detach-sign")
            .arg("--output")
            .arg(&signature)
            .arg(file)
            .stderr(Stdio::null())
            .status()
            .unwrap();
        assert!(status.success());
        signature
    }

    fn path(&self) -> &Path {
        self.home.path()
    }
}

impl Drop for Keyring {
    fn drop(&mut self) {
        let _ = Command::new("gpgconf")
            .args(["--kill", "gpg-agent"])
            .env("GNUPGHOME", self.home.path())
            .status();
    }
}

fn empty_keyring() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    fs::set_permissions(
        dir.path(),
        std::os::unix::fs::PermissionsExt::from_mode(0o700),
    )
    .unwrap();
    dir
}

#[test]
fn a_good_gpg_signature_verifies_and_names_the_signer() {
    let Some(keyring) = Keyring::new() else {
        return;
    };
    let sandbox = Sandbox::new();
    let file = tool(&sandbox);
    for extension in ["asc", "sig"] {
        keyring.sign(&file, extension);
        let output = sandbox.glori_env(
            &["info", file.to_str().unwrap(), "--json"],
            &[("GNUPGHOME", keyring.path())],
        );
        let info: serde_json::Value = serde_json::from_str(&stdout(&output)).unwrap();
        assert_eq!(info["verification"], "verified", "{extension}");
        let detail = info["verification_detail"].as_str().unwrap();
        assert!(detail.contains("Glori Test"), "{detail}");
        fs::remove_file(format!("{}.{extension}", file.display())).unwrap();
    }
    keyring.sign(&file, "asc");
    let installed = sandbox.glori_env(
        &["install", file.to_str().unwrap(), "--yes"],
        &[("GNUPGHOME", keyring.path())],
    );
    assert!(installed.status.success(), "{}", stderr(&installed));
}

#[test]
fn a_file_changed_after_signing_is_refused() {
    let Some(keyring) = Keyring::new() else {
        return;
    };
    let sandbox = Sandbox::new();
    let file = tool(&sandbox);
    keyring.sign(&file, "asc");
    let mut bytes = fs::read(&file).unwrap();
    bytes.push(0);
    fs::write(&file, bytes).unwrap();
    let output = sandbox.glori_env(
        &["install", file.to_str().unwrap(), "--yes"],
        &[("GNUPGHOME", keyring.path())],
    );
    assert!(!output.status.success());
    assert!(stderr(&output).contains("verification failed"));
    assert!(stdout(&output).contains("does not match the file"));
    assert!(sandbox.nothing_was_installed());
}

#[test]
fn a_signature_from_an_unknown_key_is_refused_and_explained() {
    let Some(keyring) = Keyring::new() else {
        return;
    };
    let sandbox = Sandbox::new();
    let file = tool(&sandbox);
    keyring.sign(&file, "asc");
    let stranger = empty_keyring();
    let output = sandbox.glori_env(
        &["install", file.to_str().unwrap(), "--yes"],
        &[("GNUPGHOME", stranger.path())],
    );
    assert!(!output.status.success());
    assert!(
        stdout(&output).contains("not in your keyring"),
        "{}",
        stdout(&output)
    );
    assert!(sandbox.nothing_was_installed());
}

#[test]
fn a_signature_file_that_is_not_openpgp_does_not_block_the_install() {
    if Keyring::new().is_none() {
        return;
    }
    let sandbox = Sandbox::new();
    let file = tool(&sandbox);
    sandbox.download(
        &format!("{CLI_TOOL}.sig"),
        b"untrusted comment: minisign public key\nRWQabc\n",
    );
    let stranger = empty_keyring();
    let output = sandbox.glori_env(
        &["install", file.to_str().unwrap(), "--yes"],
        &[("GNUPGHOME", stranger.path())],
    );
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(stdout(&output).contains("not an OpenPGP signature"));
}

#[test]
fn a_good_signature_and_a_wrong_checksum_still_fail() {
    let Some(keyring) = Keyring::new() else {
        return;
    };
    let sandbox = Sandbox::new();
    let file = tool(&sandbox);
    keyring.sign(&file, "asc");
    let output = sandbox.glori_env(
        &["install", file.to_str().unwrap(), "--yes", "--sha256", ZERO],
        &[("GNUPGHOME", keyring.path())],
    );
    assert!(!output.status.success());
    assert!(sandbox.nothing_was_installed());
}
