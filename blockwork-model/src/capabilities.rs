//! What a macro can do, by block and by reporter. One table feeds both the
//! import review (everything a macro can do) and the run-time permission
//! check for restricted macros (`permissions::require`).

use crate::input::value::Value;
use crate::macros::{BlockKind, InstructionKind, Macro, Role};
use std::collections::HashSet;

/// Every capability, in display order: (kind, label, whether it deserves a warning).
pub const CAPABILITIES: [(&str, &str, bool); 13] = [
    ("commands", "Runs commands and opens or closes apps", true),
    ("secrets", "Reads your saved secrets", true),
    ("network", "Goes online", true),
    (
        "browser",
        "Controls your web browser (clicks, types and reads pages)",
        true,
    ),
    ("files_write", "Writes to files", true),
    ("files_read", "Reads files and folders", false),
    ("input", "Presses keys, clicks and moves the mouse", false),
    ("clipboard", "Reads or changes the clipboard", false),
    ("windows", "Reads or controls other apps and windows", false),
    (
        "triggers",
        "Starts on its own (schedules, keys, typing, apps, idle, windows, files and more)",
        false,
    ),
    (
        "screen",
        "Reads the screen (colors, images and text)",
        false,
    ),
    ("macros", "Runs or stops your other macros", false),
    ("globals", "Reads or changes global variables", false),
];

/// What a block itself does (its row in `InstructionKind::info`).
pub fn block_capabilities(kind: &InstructionKind) -> &'static [&'static str] {
    kind.info().capabilities
}

/// Capabilities the runner doesn't ask a restricted macro about as a block
/// runs. Their reporters still ask (see `input::value`).
const NOT_ASKED_BY_BLOCKS: [&str; 3] = ["clipboard", "windows", "files_read"];

/// What the runner checks before running this block in a restricted macro.
pub fn checked_when_run(kind: &InstructionKind) -> impl Iterator<Item = &'static str> {
    block_capabilities(kind)
        .iter()
        .copied()
        .filter(|k| !NOT_ASKED_BY_BLOCKS.contains(k))
}

/// Blockwork reporters that read nothing guarded. Every reporter in
/// `input/value.rs` is either here or in [`operator_capability`].
pub const UNGUARDED_OPERATORS: &[&str] = &[
    "BatteryPercentage",
    "PluggedIn",
    "JsonGet",
    "Detection",
    "ColorsMatch",
    "MousePosition",
    "ScreenSize",
    "MonitorCount",
    "MonitorInfo",
    "IdleSeconds",
    "ScreenLocked",
    "NetworkConnected",
    "UsbConnected",
    "BrowserConnected",
    "TextCheck",
    "TextReplace",
    "RegexReplace",
    "RegexFind",
    "SplitPart",
    "SplitCount",
    "Substring",
    "Trim",
    "FormatNumber",
    "Timestamp",
    "FormatDate",
    "ParseDate",
    "DatePart",
    "LastError",
];

/// The capability a reporter needs, by operator name.
pub fn operator_capability(op: &str) -> Option<&'static str> {
    Some(match op {
        "ShellOutput" | "ShellSucceeded" => "commands",
        "HttpGet" => "network",
        "ReadFile" | "FileExists" | "ListDirectory" => "files_read",
        "ClipboardText" | "ClipboardHasImage" | "ClipboardHasFiles" => "clipboard",
        "ActiveWindowTitle" | "ActiveWindowApp" | "WindowExists" | "WindowInfo" | "AppRunning" => {
            "windows"
        }
        "MacroRunning" => "macros",
        "AndroidElementExists"
        | "AndroidElementText"
        | "PixelColor"
        | "AverageColor"
        | "ScreenText" => "screen",
        "GlobalVariable" => "globals",
        "Secret" => "secrets",
        "PageUrl" | "PageTitle" | "ElementText" | "ElementExists" | "ElementCount" => "browser",
        _ => return None,
    })
}

fn value_capabilities(value: &Value, found: &mut HashSet<&'static str>) {
    match value {
        Value::Op { op, args, .. } => {
            found.extend(operator_capability(op.name()));
            args.iter().for_each(|a| value_capabilities(a, found));
        }
        Value::Call { args, .. } => args.iter().for_each(|a| value_capabilities(a, found)),
        _ => {}
    }
}

/// Everything `mac` can do anywhere, nested blocks and reporters included.
/// A header other than "when run" or a custom block's means it starts on its own.
pub fn macro_capabilities(mac: &Macro) -> HashSet<&'static str> {
    let mut found = HashSet::new();
    mac.graph.walk_instructions(&mut |ins| {
        found.extend(block_capabilities(&ins.kind).iter().copied());
        if ins.kind.info().role == Role::Trigger {
            found.insert("triggers");
        }
        let mut kind = ins.kind.clone();
        kind.visit_values_mut(&mut |value, _| value_capabilities(value, &mut found));
    });
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_block_capability_is_listed() {
        let kinds: HashSet<&str> = CAPABILITIES.iter().map(|(k, ..)| *k).collect();
        let blocks = every_block();
        assert!(!blocks.is_empty());
        for kind in blocks {
            for cap in block_capabilities(&kind) {
                assert!(kinds.contains(cap), "{cap} has no label");
            }
        }
    }

    /// Every block from the wire round-trip test's fixture.
    fn every_block() -> Vec<InstructionKind> {
        let fixtures: serde_json::Value =
            serde_json::from_str(include_str!("../tests/fixtures/wire_blocks.json")).unwrap();
        let blocks = fixtures.as_array().unwrap().iter().map(|block| {
            let dto: crate::frontend::InstructionDto =
                serde_json::from_value(block.clone()).unwrap();
            crate::frontend::dto_to_instruction(&dto).unwrap().kind
        });
        blocks.collect()
    }
}
