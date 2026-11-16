mod common;

use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};

use common::*;
use glori_core::libs::elf::testing::{Spec, build};

const UNKNOWN_DISTRO: &str = "ID=nixos\nPRETTY_NAME=\"NixOS\"\n";
const FEDORA: &str = "ID=fedora\nPRETTY_NAME=\"Fedora Linux\"\n";

fn elf(needed: &[&str], runpath: Option<&str>, machine: u16) -> Vec<u8> {
    build(&Spec {
        needed,
        runpath,
        machine,
        ..Spec::default()
    })
}

fn host_machine() -> u16 {
    if cfg!(target_arch = "aarch64") {
        183
    } else {
        62
    }
}

fn foreign_machine() -> u16 {
    if cfg!(target_arch = "aarch64") {
        62
    } else {
        183
    }
}

fn app(sandbox: &Sandbox, entries: &[(&str, Vec<u8>, u32)]) -> PathBuf {
    sandbox.download(
        "gizmo-2.0-linux.tar.gz",
        &tarball(|builder| {
            add_dir(builder, "gizmo-2.0/");
            for (path, data, mode) in entries {
                add_file(builder, path, data, *mode);
            }
        }),
    )
}

fn os_release(sandbox: &Sandbox, content: &str) -> PathBuf {
    sandbox.download("os-release", content.as_bytes())
}

fn plan(
    sandbox: &Sandbox,
    file: &Path,
    os: &Path,
    extra: &[&str],
    path: Option<&Path>,
) -> serde_json::Value {
    let mut args = vec!["install", file.to_str().unwrap(), "--dry-run", "--json"];
    args.extend_from_slice(extra);
    let mut env: Vec<(&str, &Path)> = vec![("GLORI_OS_RELEASE", os)];
    if let Some(path) = path {
        env.push(("PATH", path));
    }
    let output = sandbox.glori_env(&args, &env);
    assert!(output.status.success(), "{}", stderr(&output));
    serde_json::from_str(&stdout(&output)).unwrap()
}

fn fake_dnf(answer: &str) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("calls.log");
    let script = format!(
        "#!/bin/sh\necho \"$@\" >> '{}'\nprintf '%s\\n' '{answer}'\n",
        log.display()
    );
    let program = dir.path().join("dnf");
    fs::write(&program, script).unwrap();
    fs::set_permissions(&program, fs::Permissions::from_mode(0o755)).unwrap();
    (dir, log)
}

#[test]
fn a_missing_library_is_reported_with_who_needs_it() {
    let sandbox = Sandbox::new();
    let file = app(
        &sandbox,
        &[(
            "gizmo-2.0/bin/gizmo",
            elf(
                &["libc.so.6", "libglori-missing.so.9"],
                None,
                host_machine(),
            ),
            0o755,
        )],
    );
    let os = os_release(&sandbox, UNKNOWN_DISTRO);
    let report = plan(&sandbox, &file, &os, &[], None);
    let missing = report["libraries"]["missing"].as_array().unwrap();
    assert_eq!(missing.len(), 1, "{report}");
    assert_eq!(missing[0]["soname"], "libglori-missing.so.9");
    assert_eq!(missing[0]["needed_by"][0], "gizmo");
    assert!(missing[0]["package"].is_null());

    let human = sandbox.glori_env(
        &["install", file.to_str().unwrap(), "--dry-run"],
        &[("GLORI_OS_RELEASE", os.as_path())],
    );
    let text = stdout(&human);
    assert!(text.contains("Missing system libraries"), "{text}");
    assert!(text.contains("libglori-missing.so.9"), "{text}");
    assert!(text.contains("could not find which packages"), "{text}");
}

#[test]
fn a_bundled_library_found_through_the_runpath_is_not_missing() {
    let sandbox = Sandbox::new();
    let file = app(
        &sandbox,
        &[
            (
                "gizmo-2.0/bin/gizmo",
                elf(
                    &["libc.so.6", "libglori-bundled.so.1"],
                    Some("$ORIGIN/../lib"),
                    host_machine(),
                ),
                0o755,
            ),
            (
                "gizmo-2.0/lib/libglori-bundled.so.1",
                elf(&["libc.so.6"], None, host_machine()),
                0o644,
            ),
        ],
    );
    let os = os_release(&sandbox, UNKNOWN_DISTRO);
    let report = plan(&sandbox, &file, &os, &[], None);
    assert!(
        report["libraries"]["missing"]
            .as_array()
            .unwrap()
            .is_empty(),
        "{report}"
    );
    assert_eq!(report["libraries"]["scanned"], 2);
}

#[test]
fn a_bundled_library_that_needs_a_missing_one_is_followed() {
    let sandbox = Sandbox::new();
    let file = app(
        &sandbox,
        &[
            (
                "gizmo-2.0/bin/gizmo",
                elf(
                    &["libglori-bundled.so.1"],
                    Some("$ORIGIN/../lib"),
                    host_machine(),
                ),
                0o755,
            ),
            (
                "gizmo-2.0/lib/libglori-bundled.so.1",
                elf(&["libglori-deep.so.4"], None, host_machine()),
                0o644,
            ),
        ],
    );
    let os = os_release(&sandbox, UNKNOWN_DISTRO);
    let report = plan(&sandbox, &file, &os, &[], None);
    let missing = report["libraries"]["missing"].as_array().unwrap();
    assert_eq!(missing[0]["soname"], "libglori-deep.so.4");
    assert_eq!(missing[0]["needed_by"][0], "libglori-bundled.so.1");
}

#[test]
fn a_program_built_for_another_architecture_is_called_out_but_still_installable() {
    let sandbox = Sandbox::new();
    let file = app(
        &sandbox,
        &[(
            "gizmo-2.0/bin/gizmo",
            elf(&[], None, foreign_machine()),
            0o755,
        )],
    );
    let os = os_release(&sandbox, UNKNOWN_DISTRO);
    let report = plan(&sandbox, &file, &os, &[], None);
    let incompatible = report["libraries"]["incompatible"][0].as_str().unwrap();
    assert!(
        incompatible.contains("bin/gizmo is built for"),
        "{incompatible}"
    );
    let installed = sandbox.glori_env(
        &["install", file.to_str().unwrap(), "--yes"],
        &[("GLORI_OS_RELEASE", os.as_path())],
    );
    assert!(installed.status.success(), "{}", stderr(&installed));
    assert!(stdout(&installed).contains("Cannot run here"));
}

#[test]
fn scripts_and_data_files_are_not_mistaken_for_binaries() {
    let sandbox = Sandbox::new();
    let file = app(
        &sandbox,
        &[
            (
                "gizmo-2.0/bin/gizmo",
                b"#!/bin/sh\necho hi\n".to_vec(),
                0o755,
            ),
            ("gizmo-2.0/share/data.bin", vec![0x7f, b'E', b'L'], 0o755),
        ],
    );
    let os = os_release(&sandbox, UNKNOWN_DISTRO);
    let report = plan(&sandbox, &file, &os, &[], None);
    assert_eq!(report["libraries"]["scanned"], 0);
    assert!(
        report["libraries"]["missing"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn packages_are_looked_up_with_the_distribution_package_manager() {
    let sandbox = Sandbox::new();
    let file = app(
        &sandbox,
        &[(
            "gizmo-2.0/bin/gizmo",
            elf(&["libglori-missing.so.9"], None, host_machine()),
            0o755,
        )],
    );
    let os = os_release(&sandbox, FEDORA);
    let (bin, log) = fake_dnf("fake-package");
    let report = plan(&sandbox, &file, &os, &[], Some(bin.path()));
    assert_eq!(
        report["libraries"]["missing"][0]["package"], "fake-package",
        "{report}"
    );
    let asked = fs::read_to_string(&log).unwrap();
    assert!(
        asked.contains("--whatprovides libglori-missing.so.9()(64bit)"),
        "{asked}"
    );
    assert_eq!(report["operations"][0]["op"], "extract");
}

#[test]
fn install_deps_puts_the_package_install_before_everything_else() {
    let sandbox = Sandbox::new();
    let file = app(
        &sandbox,
        &[(
            "gizmo-2.0/bin/gizmo",
            elf(&["libglori-missing.so.9"], None, host_machine()),
            0o755,
        )],
    );
    let os = os_release(&sandbox, FEDORA);
    let (bin, _) = fake_dnf("fake-package");
    let report = plan(&sandbox, &file, &os, &["--install-deps"], Some(bin.path()));
    let first = &report["operations"][0];
    assert_eq!(first["op"], "run");
    assert_eq!(first["program"], "dnf");
    assert_eq!(first["privileged"], true);
    assert_eq!(
        first["args"],
        serde_json::json!(["install", "-y", "fake-package"])
    );
    assert_eq!(report["operations"][1]["op"], "extract");
}

#[test]
fn install_deps_without_a_known_package_installs_nothing_and_says_so() {
    let sandbox = Sandbox::new();
    let file = app(
        &sandbox,
        &[(
            "gizmo-2.0/bin/gizmo",
            elf(&["libglori-missing.so.9"], None, host_machine()),
            0o755,
        )],
    );
    let os = os_release(&sandbox, UNKNOWN_DISTRO);
    let output = sandbox.glori_env(
        &[
            "install",
            file.to_str().unwrap(),
            "--dry-run",
            "--install-deps",
        ],
        &[("GLORI_OS_RELEASE", os.as_path())],
    );
    let text = stdout(&output);
    assert!(
        text.contains("--install-deps has nothing to install"),
        "{text}"
    );
}

#[test]
fn nothing_is_looked_up_when_nothing_is_missing() {
    let sandbox = Sandbox::new();
    let file = app(
        &sandbox,
        &[(
            "gizmo-2.0/bin/gizmo",
            elf(&["libc.so.6"], None, host_machine()),
            0o755,
        )],
    );
    let os = os_release(&sandbox, FEDORA);
    let (bin, log) = fake_dnf("should-not-be-asked");
    let report = plan(&sandbox, &file, &os, &[], Some(bin.path()));
    assert!(
        report["libraries"]["missing"]
            .as_array()
            .unwrap()
            .is_empty(),
        "{report}"
    );
    assert!(!log.exists(), "the package manager was queried needlessly");
}

#[test]
fn a_package_that_cannot_be_found_is_reported_as_such() {
    let sandbox = Sandbox::new();
    let file = app(
        &sandbox,
        &[(
            "gizmo-2.0/bin/gizmo",
            elf(&["libglori-missing.so.9"], None, host_machine()),
            0o755,
        )],
    );
    let os = os_release(&sandbox, FEDORA);
    let (bin, _) = fake_dnf("");
    let output = sandbox.glori_env(
        &["install", file.to_str().unwrap(), "--dry-run"],
        &[("GLORI_OS_RELEASE", os.as_path()), ("PATH", bin.path())],
    );
    assert!(
        stdout(&output).contains("(package not found)"),
        "{}",
        stdout(&output)
    );
}

#[test]
fn a_clean_tarball_prints_no_library_section() {
    let sandbox = Sandbox::new();
    let file = sandbox.download(CLI_TOOL, &cli_tool());
    let output = sandbox.glori(&["install", file.to_str().unwrap(), "--dry-run"]);
    let text = stdout(&output);
    assert!(!text.contains("Missing system libraries"));
    assert!(!text.contains("Cannot run here"));
}
