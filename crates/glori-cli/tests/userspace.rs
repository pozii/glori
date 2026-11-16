use std::{
    fs,
    io::Write,
    os::unix::fs::{PermissionsExt, symlink},
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
};

use tempfile::TempDir;

struct Sandbox {
    home: TempDir,
    downloads: TempDir,
}

impl Sandbox {
    fn new() -> Self {
        Sandbox {
            home: tempfile::tempdir().unwrap(),
            downloads: tempfile::tempdir().unwrap(),
        }
    }

    fn glori(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_glori"))
            .args(args)
            .env("HOME", self.home.path())
            .env_remove("XDG_DATA_HOME")
            .env("NO_COLOR", "1")
            .stdin(Stdio::null())
            .output()
            .expect("failed to run glori")
    }

    fn install(&self, file: &Path, extra: &[&str]) -> Output {
        let mut args = vec!["install", file.to_str().unwrap(), "--yes"];
        args.extend_from_slice(extra);
        self.glori(&args)
    }

    fn download(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let path = self.downloads.path().join(name);
        fs::write(&path, bytes).unwrap();
        path
    }

    fn bin(&self, name: &str) -> PathBuf {
        self.home.path().join(".local/bin").join(name)
    }

    fn opt(&self, name: &str) -> PathBuf {
        self.home.path().join(".local/opt").join(name)
    }

    fn desktop(&self, name: &str) -> PathBuf {
        self.home
            .path()
            .join(".local/share/applications")
            .join(format!("{name}.desktop"))
    }

    fn registry(&self) -> PathBuf {
        self.home.path().join(".local/share/glori/registry.json")
    }

    fn nothing_was_installed(&self) -> bool {
        !self.home.path().join(".local").exists()
    }
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn tarball(build: impl FnOnce(&mut tar::Builder<Vec<u8>>)) -> Vec<u8> {
    let mut builder = tar::Builder::new(Vec::new());
    build(&mut builder);
    let raw = builder.into_inner().unwrap();
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    encoder.write_all(&raw).unwrap();
    encoder.finish().unwrap()
}

fn add_file(builder: &mut tar::Builder<Vec<u8>>, path: &str, data: &[u8], mode: u32) {
    let mut header = tar::Header::new_gnu();
    header.set_size(data.len() as u64);
    header.set_mode(mode);
    header.set_cksum();
    builder.append_data(&mut header, path, data).unwrap();
}

fn add_dir(builder: &mut tar::Builder<Vec<u8>>, path: &str) {
    let mut header = tar::Header::new_gnu();
    header.set_entry_type(tar::EntryType::Directory);
    header.set_mode(0o755);
    header.set_size(0);
    header.set_cksum();
    builder
        .append_data(&mut header, path, std::io::empty())
        .unwrap();
}

fn add_raw(builder: &mut tar::Builder<Vec<u8>>, path: &[u8], data: &[u8]) {
    let mut header = tar::Header::new_gnu();
    header.as_old_mut().name[..path.len()].copy_from_slice(path);
    header.set_size(data.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    builder.append(&header, data).unwrap();
}

fn cli_tool() -> Vec<u8> {
    tarball(|b| {
        add_dir(b, "rg-1.0/");
        add_file(b, "rg-1.0/rg", b"#!/bin/sh\necho ripgrep-ok\n", 0o755);
        add_file(b, "rg-1.0/README", b"docs", 0o644);
    })
}

const CLI_TOOL: &str = "rg-1.0-x86_64-unknown-linux-musl.tar.gz";

fn appimage_bytes() -> Vec<u8> {
    let mut bytes = b"\x7fELF".to_vec();
    bytes.extend_from_slice(&[2, 1, 1, 0, b'A', b'I', 2, 0]);
    bytes.extend_from_slice(b"#!/bin/sh\necho appimage-ok\n");
    bytes
}

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
