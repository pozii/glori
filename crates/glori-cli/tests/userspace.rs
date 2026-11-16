mod common;

use std::{
    fs,
    os::unix::fs::{PermissionsExt, symlink},
    process::Command,
};

use common::*;

#[test]
fn a_cli_tarball_is_installed_linked_listed_and_removed() {
    let sandbox = Sandbox::new();
    let file = sandbox.download(CLI_TOOL, &cli_tool());

    let installed = sandbox.install(&file, &[]);
    assert!(installed.status.success(), "{}", stderr(&installed));
    assert_eq!(fs::read(sandbox.opt("rg").join("README")).unwrap(), b"docs");
    assert_eq!(
        fs::read_link(sandbox.bin("rg")).unwrap(),
        sandbox.opt("rg").join("rg")
    );
    assert!(!sandbox.desktop("rg").exists());

    let run = Command::new(sandbox.bin("rg")).output().unwrap();
    assert_eq!(String::from_utf8_lossy(&run.stdout).trim(), "ripgrep-ok");

    let listed = sandbox.glori(&["list", "--json"]);
    let entries: serde_json::Value = serde_json::from_str(&stdout(&listed)).unwrap();
    assert_eq!(entries[0]["name"], "rg");
    assert_eq!(entries[0]["version"], "1.0");
    assert_eq!(entries[0]["kind"], "archive");
    assert!(stdout(&sandbox.glori(&["list"])).contains("rg"));

    let removed = sandbox.glori(&["remove", "rg", "--yes"]);
    assert!(removed.status.success(), "{}", stderr(&removed));
    assert!(!sandbox.opt("rg").exists());
    assert!(fs::symlink_metadata(sandbox.bin("rg")).is_err());
    assert!(stdout(&sandbox.glori(&["list"])).contains("Nothing installed"));
}

#[test]
fn installing_twice_is_refused() {
    let sandbox = Sandbox::new();
    let file = sandbox.download(CLI_TOOL, &cli_tool());
    assert!(sandbox.install(&file, &[]).status.success());
    let again = sandbox.install(&file, &[]);
    assert!(!again.status.success());
    assert!(stderr(&again).contains("already installed"));
}

#[test]
fn a_graphical_tarball_gets_a_menu_entry_with_its_icon() {
    let sandbox = Sandbox::new();
    let file = sandbox.download(
        "My-App-2.0-linux-x64.tar.gz",
        &tarball(|b| {
            add_dir(b, "my-app-2.0/");
            add_file(b, "my-app-2.0/bin/my-app", b"#!/bin/sh\n", 0o755);
            add_file(
                b,
                "my-app-2.0/share/icons/256x256/my-app.png",
                b"png",
                0o644,
            );
        }),
    );
    let installed = sandbox.install(&file, &[]);
    assert!(installed.status.success(), "{}", stderr(&installed));
    let entry = fs::read_to_string(sandbox.desktop("my-app")).unwrap();
    assert!(entry.contains("Name=My App\n"), "{entry}");
    assert!(entry.contains(&format!(
        "Exec={} %U",
        sandbox.opt("my-app").join("bin/my-app").display()
    )));
    assert!(entry.contains(&format!(
        "Icon={}",
        sandbox.opt("my-app").join("share/icons/256x256/my-app.png").display()
    )));
    assert!(
        sandbox
            .glori(&["remove", "my-app", "--yes"])
            .status
            .success()
    );
    assert!(!sandbox.desktop("my-app").exists());
}

#[test]
fn an_appimage_is_copied_linked_and_given_a_menu_entry() {
    let sandbox = Sandbox::new();
    let file = sandbox.download("Cool-Tool-1.2.3-x86_64.AppImage", &appimage_bytes());
    let installed = sandbox.install(&file, &[]);
    assert!(installed.status.success(), "{}", stderr(&installed));
    let target = sandbox
        .opt("cool-tool")
        .join("Cool-Tool-1.2.3-x86_64.AppImage");
    assert_eq!(
        fs::metadata(&target).unwrap().permissions().mode() & 0o777,
        0o755
    );
    assert_eq!(fs::read_link(sandbox.bin("cool-tool")).unwrap(), target);
    let entry = fs::read_to_string(sandbox.desktop("cool-tool")).unwrap();
    assert!(entry.contains("Name=Cool Tool\n"), "{entry}");
    assert_eq!(fs::read(&target).unwrap(), appimage_bytes());
    assert!(file.exists());
    assert!(
        sandbox
            .glori(&["remove", "cool-tool", "--yes"])
            .status
            .success()
    );
    assert!(!sandbox.opt("cool-tool").exists());
    assert!(!sandbox.desktop("cool-tool").exists());
}

#[test]
fn name_override_is_used() {
    let sandbox = Sandbox::new();
    let file = sandbox.download(CLI_TOOL, &cli_tool());
    assert!(
        sandbox
            .install(&file, &["--name", "grep2"])
            .status
            .success()
    );
    assert!(sandbox.opt("grep2").join("rg").is_file());
    assert!(!sandbox.opt("rg").exists());
}

#[test]
fn dry_run_changes_nothing() {
    let sandbox = Sandbox::new();
    let file = sandbox.download(CLI_TOOL, &cli_tool());
    let output = sandbox.glori(&["install", file.to_str().unwrap(), "--dry-run"]);
    assert!(output.status.success(), "{}", stderr(&output));
    let text = stdout(&output);
    assert!(text.contains("extract"), "{text}");
    assert!(text.contains("link"), "{text}");
    assert!(sandbox.nothing_was_installed());
}

#[test]
fn dry_run_json_lists_operations() {
    let sandbox = Sandbox::new();
    let file = sandbox.download(CLI_TOOL, &cli_tool());
    let output = sandbox.glori(&["install", file.to_str().unwrap(), "--dry-run", "--json"]);
    let plan: serde_json::Value = serde_json::from_str(&stdout(&output)).unwrap();
    let ops: Vec<&str> = plan["operations"]
        .as_array()
        .unwrap()
        .iter()
        .map(|op| op["op"].as_str().unwrap())
        .collect();
    assert_eq!(ops, ["extract", "link", "register"]);
    assert!(sandbox.nothing_was_installed());
}

#[test]
fn path_traversal_in_an_archive_is_rejected_before_anything_is_written() {
    let sandbox = Sandbox::new();
    let file = sandbox.download(
        "evil-1.0.tar.gz",
        &tarball(|b| {
            add_file(b, "evil-1.0/ok", b"fine", 0o644);
            add_raw(b, b"../escaped", b"pwned");
        }),
    );
    let output = sandbox.install(&file, &[]);
    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("unsafe path"),
        "{}",
        stderr(&output)
    );
    assert!(sandbox.nothing_was_installed());
    assert!(!sandbox.home.path().join("escaped").exists());
    assert!(!sandbox.home.path().join(".local/escaped").exists());
}

#[test]
fn a_symlink_cannot_be_used_to_write_outside_the_install_directory() {
    let sandbox = Sandbox::new();
    let outside = tempfile::tempdir().unwrap();
    let target = outside.path().to_str().unwrap().to_string();
    let file = sandbox.download(
        "sneaky-1.0.tar.gz",
        &tarball(|b| {
            add_dir(b, "sneaky-1.0/");
            let mut header = tar::Header::new_gnu();
            header.set_entry_type(tar::EntryType::Symlink);
            header.set_size(0);
            header.set_mode(0o777);
            b.append_link(&mut header, "sneaky-1.0/door", &target)
                .unwrap();
            add_file(b, "sneaky-1.0/door/pwned", b"gotcha", 0o644);
        }),
    );
    let output = sandbox.install(&file, &[]);
    assert!(!output.status.success(), "{}", stdout(&output));
    assert!(!outside.path().join("pwned").exists());
    assert!(!sandbox.opt("sneaky").exists());
}

#[test]
fn an_existing_command_is_never_overwritten() {
    let sandbox = Sandbox::new();
    fs::create_dir_all(sandbox.bin("rg").parent().unwrap()).unwrap();
    fs::write(sandbox.bin("rg"), "mine").unwrap();
    let file = sandbox.download(CLI_TOOL, &cli_tool());
    let output = sandbox.install(&file, &[]);
    assert!(!output.status.success());
    assert!(stderr(&output).contains("already exists"));
    assert_eq!(fs::read_to_string(sandbox.bin("rg")).unwrap(), "mine");
    assert!(!sandbox.opt("rg").exists());
}

#[test]
fn removing_something_unknown_explains_what_to_do() {
    let sandbox = Sandbox::new();
    let output = sandbox.glori(&["remove", "sl", "--yes"]);
    assert!(!output.status.success());
    let message = stderr(&output);
    assert!(message.contains("not installed by Glori"), "{message}");
    assert!(message.contains("remove it with"), "{message}");
}

#[test]
fn a_tampered_registry_cannot_make_remove_delete_other_files() {
    let sandbox = Sandbox::new();
    let precious = sandbox.home.path().join("precious.txt");
    fs::write(&precious, "keep me").unwrap();
    fs::create_dir_all(sandbox.registry().parent().unwrap()).unwrap();
    let registry = serde_json::json!({
        "version": 1,
        "entries": [{
            "name": "x", "version": null, "kind": "archive", "installed_at": 1,
            "source": "x", "files": [precious.to_str().unwrap()]
        }]
    });
    fs::write(sandbox.registry(), registry.to_string()).unwrap();
    let output = sandbox.glori(&["remove", "x", "--yes"]);
    assert!(!output.status.success());
    assert!(stderr(&output).contains("outside the directories Glori manages"));
    assert_eq!(fs::read_to_string(&precious).unwrap(), "keep me");
}

#[test]
fn a_corrupted_registry_is_reported_and_left_alone() {
    let sandbox = Sandbox::new();
    fs::create_dir_all(sandbox.registry().parent().unwrap()).unwrap();
    fs::write(sandbox.registry(), "{ broken").unwrap();
    let file = sandbox.download(CLI_TOOL, &cli_tool());
    let output = sandbox.install(&file, &[]);
    assert!(!output.status.success());
    assert!(stderr(&output).contains("corrupted"));
    assert_eq!(fs::read_to_string(sandbox.registry()).unwrap(), "{ broken");
    assert!(!sandbox.opt("rg").exists());
}

#[test]
fn an_empty_archive_is_rejected() {
    let sandbox = Sandbox::new();
    let file = sandbox.download("empty-1.0.tar.gz", &tarball(|_| {}));
    let output = sandbox.install(&file, &[]);
    assert!(!output.status.success());
    assert!(stderr(&output).contains("empty"));
}

#[test]
fn a_dangling_bin_symlink_blocks_the_install_too() {
    let sandbox = Sandbox::new();
    fs::create_dir_all(sandbox.bin("rg").parent().unwrap()).unwrap();
    symlink("/nonexistent", sandbox.bin("rg")).unwrap();
    let file = sandbox.download(CLI_TOOL, &cli_tool());
    assert!(!sandbox.install(&file, &[]).status.success());
}
