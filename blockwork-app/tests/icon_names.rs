//! Every Lucide icon the editor names is in the set it ships, since a missing
//! one quietly draws the "info" fallback instead.

use std::path::Path;

/// Quoted names after `key` (`icon: "play"`, `iconName:"x"`, `icon: "x",` in Rust).
fn names_after<'a>(code: &'a str, key: &str) -> Vec<&'a str> {
    let mut out = Vec::new();
    for (at, _) in code.match_indices(key) {
        let rest = code[at + key.len()..].trim_start();
        let Some(rest) = rest.strip_prefix(':') else {
            continue;
        };
        let Some(rest) = rest.trim_start().strip_prefix('"') else {
            continue;
        };
        if let Some(end) = rest.find('"') {
            out.push(&rest[..end]);
        }
    }
    out
}

#[test]
fn every_named_icon_ships() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut files: Vec<_> = std::fs::read_dir(root.join("blockwork-qt/qml"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "qml"))
        .collect();
    files.push(root.join("blockwork-app/src/templates.rs"));
    let mut missing = Vec::new();
    for file in files {
        let code = std::fs::read_to_string(&file).unwrap();
        for key in ["iconName", "icon"] {
            for name in names_after(&code, key) {
                let looks_like_icon = !name.is_empty()
                    && name
                        .chars()
                        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
                if looks_like_icon && !lucide_qt::has_icon(name) {
                    missing.push(format!(
                        "{}: {name}",
                        file.file_name().unwrap().to_string_lossy()
                    ));
                }
            }
        }
    }
    assert!(
        missing.is_empty(),
        "icons not in the shipped Lucide set: {missing:?}"
    );
}
