use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
};

fn glori(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_glori"))
        .args(args)
        .env("NO_COLOR", "1")
        .output()
        .expect("failed to run glori")
}

fn fixture(name: &str, bytes: &[u8]) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("glori-test-{}", std::process::id()));
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
