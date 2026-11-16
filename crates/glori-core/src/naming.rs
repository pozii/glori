const EXTENSIONS: [&str; 8] = [
    ".tar.gz",
    ".tgz",
    ".tar.xz",
    ".txz",
    ".tar.zst",
    ".tar.bz2",
    ".tbz2",
    ".appimage",
];

const PLATFORM_WORDS: [&str; 24] = [
    "linux",
    "linux64",
    "linux32",
    "x86_64",
    "x86",
    "amd64",
    "x64",
    "aarch64",
    "arm64",
    "armv7",
    "armhf",
    "i386",
    "i686",
    "musl",
    "gnu",
    "unknown",
    "universal",
    "portable",
    "appimage",
    "bin",
    "static",
    "64bit",
    "32bit",
    "release",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identity {
    pub name: String,
    pub display: String,
    pub version: Option<String>,
}

pub fn identify(file_name: &str, override_name: Option<&str>) -> Identity {
    let stem = strip_extension(file_name);
    let segments: Vec<&str> = stem
        .split(['-', '_', ' '])
        .filter(|segment| !segment.is_empty())
        .collect();
    let boundary = segments
        .iter()
        .position(|segment| is_version(segment) || is_platform(segment))
        .unwrap_or(segments.len());
    let version = segments
        .iter()
        .find(|segment| is_version(segment))
        .map(|segment| segment.trim_start_matches(['v', 'V']).to_string());
    let words = if boundary == 0 {
        &segments[..]
    } else {
        &segments[..boundary]
    };
    let display = words.join(" ");
    let name = override_name.map_or_else(|| sanitize(&words.join("-")), sanitize);
    let display = if display.is_empty() {
        name.clone()
    } else {
        display
    };
    Identity {
        name,
        display,
        version,
    }
}

fn strip_extension(file_name: &str) -> &str {
    let lower = file_name.to_lowercase();
    EXTENSIONS
        .iter()
        .find(|extension| lower.ends_with(*extension))
        .map_or(file_name, |extension| {
            &file_name[..file_name.len() - extension.len()]
        })
}

fn is_version(segment: &str) -> bool {
    let rest = segment.trim_start_matches(['v', 'V']);
    let starts_with_digit = rest.chars().next().is_some_and(|c| c.is_ascii_digit());
    starts_with_digit && !is_platform(segment)
}

fn is_platform(segment: &str) -> bool {
    PLATFORM_WORDS.contains(&segment.to_lowercase().as_str())
}

pub fn sanitize(text: &str) -> String {
    let cleaned: String = text
        .to_lowercase()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '+' | '-' | '_') {
                c
            } else {
                '-'
            }
        })
        .collect();
    cleaned.trim_matches(['-', '.', '_']).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(file: &str) -> (String, Option<String>) {
        let identity = identify(file, None);
        (identity.name, identity.version)
    }

    #[test]
    fn release_tarballs() {
        assert_eq!(
            id("ripgrep-14.1.1-x86_64-unknown-linux-musl.tar.gz"),
            ("ripgrep".into(), Some("14.1.1".into()))
        );
        assert_eq!(
            id("node-v20.1.0-linux-x64.tar.xz"),
            ("node".into(), Some("20.1.0".into()))
        );
        assert_eq!(
            id("firefox-130.0.tar.bz2"),
            ("firefox".into(), Some("130.0".into()))
        );
    }

    #[test]
    fn appimages_keep_their_display_name() {
        let identity = identify("Obsidian-1.5.3.AppImage", None);
        assert_eq!(identity.name, "obsidian");
        assert_eq!(identity.display, "Obsidian");
        assert_eq!(identity.version.as_deref(), Some("1.5.3"));
    }

    #[test]
    fn multi_word_names_are_joined() {
        assert_eq!(
            id("my-cool-app_2.0_amd64.tar.zst"),
            ("my-cool-app".into(), Some("2.0".into()))
        );
    }

    #[test]
    fn no_version_keeps_the_whole_stem() {
        assert_eq!(id("tool.tar.gz"), ("tool".into(), None));
        assert_eq!(id("Some App.AppImage"), ("some-app".into(), None));
    }

    #[test]
    fn leading_version_falls_back_to_the_stem() {
        assert_eq!(id("1.2.3.tar.gz").0, "1.2.3");
    }

    #[test]
    fn override_wins_and_is_sanitized() {
        let identity = identify("whatever-1.0.tar.gz", Some("My App!"));
        assert_eq!(identity.name, "my-app");
        assert_eq!(identity.version.as_deref(), Some("1.0"));
    }

    #[test]
    fn names_never_contain_path_separators() {
        assert_eq!(sanitize("../../etc/passwd"), "etc-passwd");
        assert!(!sanitize("a/b").contains('/'));
    }
}
