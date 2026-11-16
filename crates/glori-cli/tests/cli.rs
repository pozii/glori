use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
};

static FIXTURES: AtomicUsize = AtomicUsize::new(0);

fn glori(args: &[&str]) -> Output {
    run(args, None)
}

fn glori_on(os_release: &str, args: &[&str]) -> Output {
    let path = fixture("os-release", os_release.as_bytes());
    run(args, Some(path))
}

fn run(args: &[&str], os_release: Option<PathBuf>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_glori"));
    command.args(args).env("NO_COLOR", "1").stdin(Stdio::null());
    if let Some(path) = os_release {
        command.env("GLORI_OS_RELEASE", path);
    }
    command.output().expect("failed to run glori")
}

const FEDORA: &str = "ID=fedora\nPRETTY_NAME=\"Fedora Linux\"\n";
const DEBIAN: &str = "ID=debian\nPRETTY_NAME=\"Debian GNU/Linux\"\n";
const ARCH: &str = "ID=arch\nPRETTY_NAME=\"Arch Linux\"\n";
const RPM_HEADER: &[u8] = &[0xED, 0xAB, 0xEE, 0xDB, 3, 0];
const DEB_HEADER: &[u8] = b"!<arch>\ndebian-binary   rest";
const ZSTD_HEADER: &[u8] = &[0x28, 0xB5, 0x2F, 0xFD, 0];

fn dry_run_plan(os_release: &str, file: &Path) -> serde_json::Value {
    let output = glori_on(
        os_release,
        &["install", file.to_str().unwrap(), "--dry-run", "--json"],
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_str(&stdout(&output)).unwrap()
}

fn fixture(name: &str, bytes: &[u8]) -> PathBuf {
    let id = FIXTURES.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("glori-test-{}-{id}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    fs::write(&path, bytes).unwrap();
    path
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn info_json_reports_the_format() {
    let file = fixture("pkg.deb", b"!<arch>\ndebian-binary   rest");
    let output = glori(&["info", file.to_str().unwrap(), "--json"]);
    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_str(&stdout(&output)).unwrap();
    assert_eq!(json["format"], "deb");
    assert!(json["strategy"]["kind"].is_string());
}

#[test]
fn info_human_output_names_the_format() {
    let file = fixture("app.tar.gz", &[0x1F, 0x8B, 8, 0]);
    let output = glori(&["info", file.to_str().unwrap()]);
    assert!(output.status.success());
    assert!(stdout(&output).contains("gzip tarball"));
}

#[test]
fn info_rejects_unknown_files() {
    let file = fixture("notes.txt", b"hello world");
    let output = glori(&["info", file.to_str().unwrap()]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("unrecognized file format"));
}

#[test]
fn info_reports_missing_files() {
    let output = glori(&["info", "/nonexistent/glori-file"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("cannot read"));
}

#[test]
fn doctor_json_lists_checks() {
    let output = glori(&["doctor", "--json"]);
    let json: serde_json::Value = serde_json::from_str(&stdout(&output)).unwrap();
    assert!(json["checks"].is_array());
    assert!(json["distro"]["id"].is_string());
}

#[test]
fn completions_generate_a_script() {
    let output = glori(&["completions", "bash"]);
    assert!(output.status.success());
    assert!(stdout(&output).contains("glori"));
}

#[test]
fn no_arguments_shows_help() {
    let output = glori(&[]);
    assert!(!output.status.success());
}

#[test]
fn dry_run_on_fedora_plans_dnf_for_rpm() {
    let file = fixture("app.rpm", RPM_HEADER);
    let plan = dry_run_plan(FEDORA, &file);
    assert_eq!(plan["strategy"]["manager"], "dnf");
    assert_eq!(plan["operations"][0]["program"], "dnf");
    assert_eq!(plan["operations"][0]["privileged"], true);
    let args = plan["operations"][0]["args"].as_array().unwrap();
    assert_eq!(args[0], "install");
    assert!(args.last().unwrap().as_str().unwrap().ends_with("app.rpm"));
}

#[test]
fn dry_run_on_debian_plans_apt_for_deb() {
    let file = fixture("app.deb", DEB_HEADER);
    let plan = dry_run_plan(DEBIAN, &file);
    assert_eq!(plan["operations"][0]["program"], "apt-get");
}

#[test]
fn dry_run_on_arch_plans_pacman_for_arch_packages() {
    let file = fixture("app-1-1-x86_64.pkg.tar.zst", ZSTD_HEADER);
    let plan = dry_run_plan(ARCH, &file);
    assert_eq!(plan["operations"][0]["program"], "pacman");
    assert_eq!(plan["operations"][0]["args"][0], "-U");
}

#[test]
fn dry_run_human_output_prints_the_command_and_changes_nothing() {
    let file = fixture("human.rpm", RPM_HEADER);
    let output = glori_on(FEDORA, &["install", file.to_str().unwrap(), "--dry-run"]);
    assert!(output.status.success());
    let text = stdout(&output);
    assert!(text.contains("dnf install -y"));
    assert!(text.contains("nothing was changed"));
}

#[test]
fn foreign_packages_are_refused_with_a_clear_message() {
    let file = fixture("foreign.deb", DEB_HEADER);
    let output = glori_on(FEDORA, &["install", file.to_str().unwrap(), "--dry-run"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("not supported yet"));
}

#[test]
fn install_without_confirmation_stops_before_changing_anything() {
    let file = fixture("confirm.deb", DEB_HEADER);
    let output = glori_on(DEBIAN, &["install", file.to_str().unwrap()]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("--yes"));
}

#[test]
fn dry_run_json_reports_the_signature_state() {
    let file = fixture("sig.deb", DEB_HEADER);
    let plan = dry_run_plan(DEBIAN, &file);
    assert_eq!(plan["signature"], "unverifiable");
}

#[test]
fn info_shows_the_signature_row() {
    let file = fixture("sig-info.deb", DEB_HEADER);
    let output = glori_on(DEBIAN, &["info", file.to_str().unwrap()]);
    assert!(stdout(&output).contains("Signature"));
}

#[test]
fn json_without_dry_run_is_rejected() {
    let file = fixture("json.rpm", RPM_HEADER);
    let output = glori_on(FEDORA, &["install", file.to_str().unwrap(), "--json"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("--dry-run"));
}

#[test]
fn install_reports_missing_files() {
    let output = glori_on(FEDORA, &["install", "/nonexistent/a.rpm", "--dry-run"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("cannot read"));
}
