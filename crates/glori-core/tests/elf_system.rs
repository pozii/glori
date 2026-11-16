use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::Command,
};

use glori_core::libs::elf;

const LIMIT: usize = 6000;

fn candidates() -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut stack: Vec<PathBuf> = ["/usr/bin", "/usr/lib64", "/usr/lib", "/usr/libexec"]
        .iter()
        .map(PathBuf::from)
        .collect();
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            let path = entry.path();
            if kind.is_dir() {
                stack.push(path);
            } else if kind.is_file() && is_elf(&path) {
                found.push(path);
                if found.len() >= LIMIT {
                    return found;
                }
            }
        }
    }
    found
}

fn is_elf(path: &Path) -> bool {
    let mut magic = [0u8; 4];
    fs::File::open(path)
        .and_then(|mut file| file.read_exact(&mut magic))
        .is_ok()
        && &magic == b"\x7fELF"
}

struct Expected {
    needed: Vec<String>,
    soname: Option<String>,
    rpath: Vec<String>,
    runpath: Vec<String>,
}

fn readelf(path: &Path) -> Option<Expected> {
    let output = Command::new("readelf")
        .args(["-d", "--wide"])
        .arg(path)
        .env("LC_ALL", "C")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout).into_owned();
    let mut expected = Expected {
        needed: Vec::new(),
        soname: None,
        rpath: Vec::new(),
        runpath: Vec::new(),
    };
    for line in text.lines() {
        let Some(start) = line.find('[') else {
            continue;
        };
        let Some(end) = line.rfind(']') else { continue };
        let value = line[start + 1..end].to_string();
        let split = |value: &str| -> Vec<String> {
            value
                .split(':')
                .filter(|p| !p.is_empty())
                .map(str::to_string)
                .collect()
        };
        if line.contains("(NEEDED)") {
            expected.needed.push(value);
        } else if line.contains("(SONAME)") {
            expected.soname = Some(value);
        } else if line.contains("(RUNPATH)") {
            expected.runpath = split(&value);
        } else if line.contains("(RPATH)") {
            expected.rpath = split(&value);
        }
    }
    Some(expected)
}

#[test]
#[ignore = "compares the parser with readelf on the files installed on this machine"]
fn the_parser_agrees_with_readelf_on_real_system_files() {
    if Command::new("readelf").arg("--version").output().is_err() {
        eprintln!("skipped: readelf is not installed");
        return;
    }
    let files = candidates();
    assert!(files.len() > 50, "found only {} ELF files", files.len());
    let mut compared = 0;
    let mut with_needed = 0;
    let mut mismatches = Vec::new();
    for path in &files {
        let Some(expected) = readelf(path) else {
            continue;
        };
        let Ok(data) = fs::read(path) else { continue };
        let Some(parsed) = elf::parse(&data) else {
            mismatches.push(format!("{}: parser returned None", path.display()));
            continue;
        };
        compared += 1;
        if !expected.needed.is_empty() {
            with_needed += 1;
        }
        let same = parsed.needed == expected.needed
            && parsed.soname == expected.soname
            && parsed.rpath == expected.rpath
            && parsed.runpath == expected.runpath;
        if !same {
            mismatches.push(format!(
                "{}\n  ours:    needed={:?} soname={:?} rpath={:?} runpath={:?}\n  readelf: needed={:?} soname={:?} rpath={:?} runpath={:?}",
                path.display(),
                parsed.needed, parsed.soname, parsed.rpath, parsed.runpath,
                expected.needed, expected.soname, expected.rpath, expected.runpath
            ));
        }
    }
    eprintln!(
        "compared {compared} files ({with_needed} with dependencies), {} mismatches",
        mismatches.len()
    );
    assert!(compared > 50);
    assert!(
        mismatches.is_empty(),
        "{}",
        mismatches
            .iter()
            .take(10)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}
