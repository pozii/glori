#![allow(dead_code)]

use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
};

use tempfile::TempDir;

pub struct Sandbox {
    pub home: TempDir,
    pub downloads: TempDir,
}

impl Sandbox {
    pub fn new() -> Self {
        Sandbox {
            home: tempfile::tempdir().unwrap(),
            downloads: tempfile::tempdir().unwrap(),
        }
    }

    pub fn glori(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_glori"))
            .args(args)
            .env("HOME", self.home.path())
            .env_remove("XDG_DATA_HOME")
            .env("NO_COLOR", "1")
            .stdin(Stdio::null())
            .output()
            .expect("failed to run glori")
    }

    pub fn glori_env(&self, args: &[&str], env: &[(&str, &Path)]) -> Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_glori"));
        command
            .args(args)
            .env("HOME", self.home.path())
            .env_remove("XDG_DATA_HOME")
            .env("NO_COLOR", "1")
            .stdin(Stdio::null());
        for (key, value) in env {
            command.env(key, value);
        }
        command.output().expect("failed to run glori")
    }

    pub fn install(&self, file: &Path, extra: &[&str]) -> Output {
        let mut args = vec!["install", file.to_str().unwrap(), "--yes"];
        args.extend_from_slice(extra);
        self.glori(&args)
    }

    pub fn download(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let path = self.downloads.path().join(name);
        fs::write(&path, bytes).unwrap();
        path
    }

    pub fn bin(&self, name: &str) -> PathBuf {
        self.home.path().join(".local/bin").join(name)
    }

    pub fn opt(&self, name: &str) -> PathBuf {
        self.home.path().join(".local/opt").join(name)
    }

    pub fn desktop(&self, name: &str) -> PathBuf {
        self.home
            .path()
            .join(".local/share/applications")
            .join(format!("{name}.desktop"))
    }

    pub fn registry(&self) -> PathBuf {
        self.home.path().join(".local/share/glori/registry.json")
    }

    pub fn nothing_was_installed(&self) -> bool {
        !self.home.path().join(".local").exists()
    }
}

pub fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

pub fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

pub fn tarball(build: impl FnOnce(&mut tar::Builder<Vec<u8>>)) -> Vec<u8> {
    let mut builder = tar::Builder::new(Vec::new());
    build(&mut builder);
    let raw = builder.into_inner().unwrap();
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    encoder.write_all(&raw).unwrap();
    encoder.finish().unwrap()
}

pub fn add_file(builder: &mut tar::Builder<Vec<u8>>, path: &str, data: &[u8], mode: u32) {
    let mut header = tar::Header::new_gnu();
    header.set_size(data.len() as u64);
    header.set_mode(mode);
    header.set_cksum();
    builder.append_data(&mut header, path, data).unwrap();
}

pub fn add_dir(builder: &mut tar::Builder<Vec<u8>>, path: &str) {
    let mut header = tar::Header::new_gnu();
    header.set_entry_type(tar::EntryType::Directory);
    header.set_mode(0o755);
    header.set_size(0);
    header.set_cksum();
    builder
        .append_data(&mut header, path, std::io::empty())
        .unwrap();
}

pub fn add_raw(builder: &mut tar::Builder<Vec<u8>>, path: &[u8], data: &[u8]) {
    let mut header = tar::Header::new_gnu();
    header.as_old_mut().name[..path.len()].copy_from_slice(path);
    header.set_size(data.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    builder.append(&header, data).unwrap();
}

pub fn cli_tool() -> Vec<u8> {
    tarball(|b| {
        add_dir(b, "rg-1.0/");
        add_file(b, "rg-1.0/rg", b"#!/bin/sh\necho ripgrep-ok\n", 0o755);
        add_file(b, "rg-1.0/README", b"docs", 0o644);
    })
}

pub const CLI_TOOL: &str = "rg-1.0-x86_64-unknown-linux-musl.tar.gz";

pub fn appimage_bytes() -> Vec<u8> {
    let mut bytes = b"\x7fELF".to_vec();
    bytes.extend_from_slice(&[2, 1, 1, 0, b'A', b'I', 2, 0]);
    bytes.extend_from_slice(b"#!/bin/sh\necho appimage-ok\n");
    bytes
}
