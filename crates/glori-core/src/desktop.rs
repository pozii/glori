use std::path::Path;

pub fn entry(display_name: &str, exec: &Path, icon: Option<&Path>) -> String {
    let mut lines = vec![
        "[Desktop Entry]".to_string(),
        "Type=Application".to_string(),
        format!("Name={}", escape_value(display_name)),
        format!("Exec={} %U", quote_exec(&exec.to_string_lossy())),
    ];
    if let Some(icon) = icon {
        lines.push(format!("Icon={}", escape_value(&icon.to_string_lossy())));
    }
    lines.extend([
        "Terminal=false".to_string(),
        "Categories=Utility;".to_string(),
        "X-Glori-Managed=true".to_string(),
    ]);
    lines.join("\n") + "\n"
}

fn escape_value(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

fn quote_exec(path: &str) -> String {
    let plain = !path.is_empty()
        && path
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "/-_.+,:@".contains(c));
    if plain {
        return path.to_string();
    }
    let escaped: String = path
        .chars()
        .flat_map(|c| match c {
            '"' | '`' | '$' | '\\' => vec!['\\', c],
            '%' => vec!['%', '%'],
            other => vec![other],
        })
        .collect();
    format!("\"{escaped}\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimal_entry() {
        let text = entry("Rip", Path::new("/h/.local/bin/rg"), None);
        assert!(text.starts_with("[Desktop Entry]\nType=Application\nName=Rip\n"));
        assert!(text.contains("Exec=/h/.local/bin/rg %U\n"));
        assert!(!text.contains("Icon="));
        assert!(text.ends_with("X-Glori-Managed=true\n"));
    }

    #[test]
    fn icon_is_included() {
        let text = entry("A", Path::new("/a"), Some(Path::new("/i/a.png")));
        assert!(text.contains("Icon=/i/a.png\n"));
    }

    #[test]
    fn paths_with_spaces_are_quoted_in_exec() {
        let text = entry("A", Path::new("/my apps/a b"), None);
        assert!(text.contains("Exec=\"/my apps/a b\" %U\n"));
    }

    #[test]
    fn special_characters_are_escaped_in_exec() {
        assert_eq!(quote_exec("/a$b\"c"), "\"/a\\$b\\\"c\"");
        assert_eq!(quote_exec("/100%/x"), "\"/100%%/x\"");
    }

    #[test]
    fn newlines_cannot_inject_keys() {
        let text = entry("Evil\nExec=/bin/sh", Path::new("/a"), None);
        assert_eq!(text.lines().filter(|l| l.starts_with("Exec=")).count(), 1);
    }
}
