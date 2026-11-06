//! Every command the editor's QML sends by name has an arm in
//! `src/dispatch.rs`, and every argument key it sends in an object literal is
//! one that arm reads, so a typo fails here rather than at run time.

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
            // Rows that name the command they run, like the onboarding toggles,
            // and pickers told which command creates a name (`createCommand`).
            let named = line
                .split("command: \"")
                .skip(1)
                .chain(line.split("createCommand: \"").skip(1));
            for part in named {
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

/// Per command, the argument keys its arm reads, or `None` when the arm hands
/// `args` on whole and the keys can't be listed.
fn read_keys() -> std::collections::HashMap<String, Option<BTreeSet<String>>> {
    let source =
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/dispatch.rs")).unwrap();
    let draft = [
        "pieces",
        "shape",
        "color",
        "icon",
        "identifier",
        "description",
    ];
    let body_start = source.find("        match cmd {").unwrap();
    let mut arms: Vec<(Vec<String>, String)> = Vec::new();
    for line in source[body_start..].lines().skip(1) {
        let trimmed = line.trim_start();
        let indent = line.len() - trimmed.len();
        if indent == 12 && (trimmed.starts_with('"') || trimmed.starts_with('_')) {
            let pattern = trimmed.split("=>").next().unwrap();
            let names = pattern.split('"').skip(1).step_by(2).map(str::to_owned);
            arms.push((names.collect(), String::new()));
        }
        if let Some((_, body)) = arms.last_mut() {
            body.push_str(line);
        }
    }
    let mut keys = std::collections::HashMap::new();
    for (names, body) in arms {
        let code: String = body.chars().filter(|c| !c.is_whitespace()).collect();
        let mut read = Some(BTreeSet::new());
        let mut at = 0;
        while let Some(found) = code[at..].find("args") {
            let start = at + found;
            at = start + 4;
            let word = |c: Option<char>| c.is_some_and(|c| c.is_alphanumeric() || c == '_');
            if word(code[..start].chars().last()) || word(code[at..].chars().next()) {
                continue;
            }
            let (before, after) = (&code[..start], &code[at..]);
            let quoted = |s: &str| s.split('"').nth(1).map(str::to_owned);
            let key = if before.ends_with("block_draft(&") {
                read.iter_mut()
                    .for_each(|r| r.extend(draft.map(String::from)));
                continue;
            } else if before.ends_with('&') && after.starts_with(",\"") {
                // `arg(&args, "key")` and `arg::<T>(&args, "key")`
                let call = before.trim_end_matches('&').trim_end_matches('(');
                (call.ends_with("arg") || call.ends_with('>'))
                    .then(|| quoted(after))
                    .flatten()
            } else if after.starts_with(".get(\"") || after.starts_with("[\"") {
                quoted(after)
            } else {
                None
            };
            match (key, read.as_mut()) {
                (Some(key), Some(set)) => {
                    set.insert(key);
                }
                (None, _) => read = None,
                _ => {}
            }
        }
        for name in names {
            keys.insert(name, read.clone());
        }
    }
    keys
}

/// The top-level keys of a `{ ... }` literal at the start of `code`, or `None`
/// when it isn't one or spreads another object in.
fn object_keys(code: &str) -> Option<Vec<String>> {
    let inner = code.trim_start().strip_prefix('{')?;
    let (mut depth, mut in_string, mut keys, mut entry) = (0, None, Vec::new(), String::new());
    for c in inner.chars() {
        match (in_string, c) {
            (Some(q), _) if c == q => in_string = None,
            (Some(_), _) => continue,
            (None, '"' | '\'') => in_string = Some(c),
            (None, '(' | '[' | '{') => depth += 1,
            (None, '}') if depth == 0 => {
                keys.push(std::mem::take(&mut entry));
                break;
            }
            (None, ')' | ']' | '}') => depth -= 1,
            (None, ',') if depth == 0 => keys.push(std::mem::take(&mut entry)),
            _ if depth == 0 => entry.push(c),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for entry in keys {
        let entry = entry.trim();
        if entry.is_empty() {
            continue;
        }
        if entry.starts_with("...") {
            return None;
        }
        let key = entry.split(':').next().unwrap().trim();
        if !key.chars().all(|c| c.is_alphanumeric() || c == '_') {
            return None;
        }
        out.push(key.to_owned());
    }
    Some(out)
}

#[test]
fn every_argument_the_qml_sends_is_read() {
    let qml = Path::new(env!("CARGO_MANIFEST_DIR")).join("../blockwork-qt/qml");
    let keys = read_keys();
    let (mut checked, mut problems) = (0, Vec::new());
    let mut files: Vec<_> = std::fs::read_dir(&qml)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "qml"))
        .collect();
    files.sort();
    for path in files {
        let file = path.file_name().unwrap().to_string_lossy().into_owned();
        let code = std::fs::read_to_string(&path).unwrap();
        for call in CALLS {
            for (at, _) in code.match_indices(call) {
                if code[..at]
                    .chars()
                    .last()
                    .is_some_and(|c| c.is_alphanumeric() || c == '_')
                {
                    continue;
                }
                let rest = &code[at + call.len()..];
                let first = first_argument(rest);
                let Some(names) = literal_names(first) else {
                    continue;
                };
                let Some(after) = rest[first.len()..].trim_start().strip_prefix(',') else {
                    continue;
                };
                let Some(sent) = object_keys(after) else {
                    continue;
                };
                let line = code[..at].lines().count();
                for name in names {
                    let Some(Some(read)) = keys.get(&name) else {
                        continue;
                    };
                    for key in &sent {
                        checked += 1;
                        if !read.contains(key) {
                            problems.push(format!("{file}:{line}: {name} never reads `{key}`"));
                        }
                    }
                }
            }
        }
    }
    assert!(
        checked > 100,
        "checked only {checked} arguments; is the scan still reading the QML?"
    );
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}
