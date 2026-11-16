#![allow(dead_code)]

use std::{
    collections::HashMap,
    fs,
    io::{BufRead, BufReader, Write},
    net::TcpListener,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    thread,
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

pub struct Server {
    base: String,
}

impl Server {
    pub fn start(files: Vec<(&str, Vec<u8>)>) -> Self {
        Self::start_with(|_| {
            files
                .into_iter()
                .map(|(path, body)| (path.to_string(), body))
                .collect()
        })
    }

    pub fn start_with(build: impl FnOnce(&str) -> Vec<(String, Vec<u8>)>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let files: HashMap<String, Vec<u8>> = build(&base).into_iter().collect();
        thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let mut reader = BufReader::new(stream);
                let mut request = String::new();
                if reader.read_line(&mut request).is_err() {
                    continue;
                }
                let path = request.split_whitespace().nth(1).unwrap_or("/").to_string();
                let mut stream = reader.into_inner();
                let _ = match files.get(&path) {
                    Some(body) => {
                        let head = format!(
                            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            body.len()
                        );
                        stream
                            .write_all(head.as_bytes())
                            .and_then(|()| stream.write_all(body))
                    }
                    None => stream.write_all(
                        b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                    ),
                };
            }
        });
        Server { base }
    }

    pub fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base)
    }
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut child = Command::new("sha256sum")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(bytes).unwrap();
    let output = child.wait_with_output().unwrap();
    String::from_utf8_lossy(&output.stdout)
        .split_whitespace()
        .next()
        .unwrap()
        .to_string()
}
