mod common;

use common::*;

const ZERO: &str = "0000000000000000000000000000000000000000000000000000000000000000";

fn path_of(name: &str) -> String {
    format!("/dl/{name}")
}

#[test]
fn installs_a_tarball_from_a_url() {
    let sandbox = Sandbox::new();
    let server = Server::start(vec![(&path_of(CLI_TOOL), cli_tool())]);
    let output = sandbox.glori(&["install", &server.url(&path_of(CLI_TOOL)), "--yes"]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(sandbox.bin("rg").exists());
}

#[test]
fn a_remote_sha256_sidecar_verifies_the_download() {
    let sandbox = Sandbox::new();
    let body = cli_tool();
    let sidecar = format!("{}  {CLI_TOOL}\n", sha256_hex(&body));
    let name = path_of(CLI_TOOL);
    let server = Server::start(vec![
        (&name, body),
        (&format!("{name}.sha256"), sidecar.into_bytes()),
    ]);
    let output = sandbox.glori(&["info", &server.url(&name), "--json"]);
    let info: serde_json::Value = serde_json::from_str(&stdout(&output)).unwrap();
    assert_eq!(info["verification"], "verified");
    assert!(
        info["verification_detail"]
            .as_str()
            .unwrap()
            .contains(".sha256")
    );
}

#[test]
fn a_remote_shared_listing_is_used() {
    let sandbox = Sandbox::new();
    let body = cli_tool();
    let listing = format!("{ZERO}  other.tar.gz\n{}  {CLI_TOOL}\n", sha256_hex(&body));
    let server = Server::start(vec![
        (&path_of(CLI_TOOL), body),
        ("/dl/SHA256SUMS", listing.into_bytes()),
    ]);
    let output = sandbox.glori(&["info", &server.url(&path_of(CLI_TOOL)), "--json"]);
    let info: serde_json::Value = serde_json::from_str(&stdout(&output)).unwrap();
    assert_eq!(info["verification"], "verified");
}

#[test]
fn a_mismatching_remote_sidecar_blocks_the_install() {
    let sandbox = Sandbox::new();
    let name = path_of(CLI_TOOL);
    let server = Server::start(vec![
        (&name, cli_tool()),
        (
            &format!("{name}.sha256"),
            format!("{ZERO}  {CLI_TOOL}\n").into_bytes(),
        ),
    ]);
    let output = sandbox.glori(&["install", &server.url(&name), "--yes"]);
    assert!(!output.status.success());
    assert!(stderr(&output).contains("verification failed"));
    assert!(sandbox.nothing_was_installed());
}

#[test]
fn the_sha256_flag_is_checked_against_the_downloaded_bytes() {
    let sandbox = Sandbox::new();
    let body = cli_tool();
    let good = sha256_hex(&body);
    let name = path_of(CLI_TOOL);
    let server = Server::start(vec![(&name, body)]);
    let url = server.url(&name);
    assert!(
        !sandbox
            .glori(&["install", &url, "--yes", "--sha256", ZERO])
            .status
            .success()
    );
    assert!(sandbox.nothing_was_installed());
    assert!(
        sandbox
            .glori(&["install", &url, "--yes", "--sha256", &good])
            .status
            .success()
    );
}

#[test]
fn a_missing_file_is_reported_and_nothing_is_installed() {
    let sandbox = Sandbox::new();
    let server = Server::start(vec![]);
    let output = sandbox.glori(&["install", &server.url("/dl/missing.tar.gz"), "--yes"]);
    assert!(!output.status.success());
    assert!(stderr(&output).contains("download of"));
    assert!(sandbox.nothing_was_installed());
}

#[test]
fn a_url_without_a_file_name_is_rejected() {
    let sandbox = Sandbox::new();
    let output = sandbox.glori(&["install", "http://127.0.0.1:9/", "--yes"]);
    assert!(!output.status.success());
    assert!(stderr(&output).contains("cannot derive a file name"));
}

#[test]
fn an_unreachable_server_fails_cleanly() {
    let sandbox = Sandbox::new();
    let output = sandbox.glori(&["install", "http://127.0.0.1:9/a.tar.gz", "--yes"]);
    assert!(!output.status.success());
    assert!(stderr(&output).contains("download of"));
    assert!(sandbox.nothing_was_installed());
}

#[test]
fn dry_run_json_from_a_url_is_clean() {
    let sandbox = Sandbox::new();
    let name = path_of(CLI_TOOL);
    let server = Server::start(vec![(&name, cli_tool())]);
    let output = sandbox.glori(&["install", &server.url(&name), "--dry-run", "--json"]);
    assert!(output.status.success(), "{}", stderr(&output));
    let plan: serde_json::Value = serde_json::from_str(&stdout(&output)).unwrap();
    assert_eq!(plan["format"], "tar_gz");
    assert!(sandbox.nothing_was_installed());
}
