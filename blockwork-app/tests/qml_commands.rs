//! Every command the editor's QML sends by name has an arm in
//! `src/dispatch.rs`, so a typo fails here rather than at run time.

use std::collections::BTreeSet;
use std::path::Path;

/// Call sites that send a command: `invoke("name", ...)` and friends.
const CALLS: &[&str] = &["invoke(", "daemonCall(", "invokeCommand(", "call("];

/// The source of a call's first argument, up to its comma or closing paren.
fn first_argument(code: &str) -> &str {
    let mut depth = 0;
    let mut in_string = false;
    for (i, c) in code.char_indices() {
        match c {
            '"' => in_string = !in_string,
            _ if in_string => {}
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' if depth == 0 => return code[..i].trim(),
            ')' | ']' | '}' => depth -= 1,
            ',' if depth == 0 => return code[..i].trim(),
            _ => {}
        }
    }
    code.trim()
}

/// The names a literal, or a `cond ? "a" : "b"`, can send.
fn literal_names(arg: &str) -> Option<Vec<String>> {
    let is_name = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_lowercase() || c == '_');
    let quoted = |s: &str| {
        s.trim()
            .strip_prefix('"')?
            .strip_suffix('"')
            .filter(|n| is_name(n))
            .map(str::to_owned)
    };
    if let Some(name) = quoted(arg) {
        return Some(vec![name]);
    }
    let (_, branches) = arg.split_once('?')?;
    let (a, b) = branches.split_once(':')?;
    Some(vec![quoted(a)?, quoted(b)?])
}

/// Every `"name"` before `=>` in the dispatcher's match.
fn dispatched() -> BTreeSet<String> {
    let source =
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/dispatch.rs")).unwrap();
    let mut names = BTreeSet::new();
    for line in source.lines() {
        let trimmed = line.trim_start();
        let indent = line.len() - trimmed.len();
        if indent != 12 || !(trimmed.starts_with('"') || trimmed.starts_with("| \"")) {
            continue;
        }
        let pattern = trimmed.split("=>").next().unwrap();
        names.extend(pattern.split('"').skip(1).step_by(2).map(str::to_owned));
    }
    names
}

#[test]
fn every_command_the_qml_sends_is_dispatched() {
    let qml = Path::new(env!("CARGO_MANIFEST_DIR")).join("../blockwork-qt/qml");
    let handled = dispatched();
    let (mut sent, mut problems) = (0, Vec::new());
    let mut files: Vec<_> = std::fs::read_dir(&qml)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .collect();
    files.retain(|p| p.extension().is_some_and(|e| e == "qml"));
    files.sort();
    for path in files {
        let file = path.file_name().unwrap().to_string_lossy().into_owned();
        let code = std::fs::read_to_string(&path).unwrap();
        let mut names: Vec<(String, usize)> = Vec::new();
        for (number, line) in code.lines().enumerate() {
            let line_no = number + 1;
            if line.trim_start().starts_with("//") {
                continue;
            }
            // Rows that name the command they run, like the onboarding toggles.
            for part in line.split("command: \"").skip(1) {
                if let Some(name) = part
                    .split('"')
                    .next()
                    .filter(|n| literal_names(&format!("\"{n}\"")).is_some())
                {
                    names.push((name.to_owned(), line_no));
                }
            }
            for call in CALLS {
                let mut rest = line;
                while let Some(at) = rest.find(call) {
                    let before = rest[..at].chars().last();
                    rest = &rest[at + call.len()..];
                    if before.is_some_and(|c| c.is_alphanumeric() || c == '_') {
                        continue; // part of a longer name, like `submitCall(`
                    }
                    let arg = first_argument(rest);
                    match literal_names(arg) {
                        Some(found) => found.into_iter().for_each(|n| names.push((n, line_no))),
                        // Forwarders pass a caller's name on; it was checked where it was written.
                        None if matches!(
                            arg,
                            "command" | "key" | "" | "preferenceRow.modelData.command"
                        ) => {}
                        None => problems.push(format!(
                            "{file}:{line_no}: can't tell which command `{arg}` sends"
                        )),
                    }
                }
            }
        }
        for (name, line) in names {
            sent += 1;
            if !handled.contains(&name) {
                problems.push(format!("{file}:{line}: {name} has no arm in dispatch.rs"));
            }
        }
    }
    assert!(
        sent > 100,
        "found only {sent} commands; is the scan still reading the QML?"
    );
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}
