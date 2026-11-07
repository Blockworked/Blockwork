//! What a macro can do, by block and by reporter. One table feeds both the
//! import review (everything a macro can do) and the run-time permission
//! check for restricted macros (`permissions::require`).

use crate::input::value::Value;
use crate::macros::{BlockKind, InstructionKind, Macro};
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

/// What a block itself does. `OpenApp`'s command can be any shell string in a
/// hand-edited file, so it counts as running commands.
pub fn block_capabilities(kind: &InstructionKind) -> &'static [&'static str] {
    use InstructionKind as K;
    match kind {
        K::Command(_) | K::CommandWait(_) | K::OpenApp { .. } | K::CloseApp { .. } => &["commands"],
        K::HttpRequest { .. } => &["network"],
        K::WriteFile { .. } => &["files_write"],
        K::WhenFileChanged(_) => &["files_read"],
        K::Token(_) | K::WhenTyped { .. } | K::AimStep { .. } => &["input"],
        K::PasteText(_) => &["input", "clipboard"],
        K::SetClipboard(_) | K::WhenClipboardChanged => &["clipboard"],
        K::WindowAction { .. }
        | K::MoveWindow { .. }
        | K::WhenWindow { .. }
        | K::WhenApp { .. } => &["windows"],
        K::RunMacro { .. } | K::StopMacroNamed(_) => &["macros"],
        K::FindImage { .. }
        | K::WaitForImage { .. }
        | K::DetectObjectsInWindow { .. }
        | K::WaitForDetection { .. } => &["screen"],
        K::ClickImage { .. } | K::AimAtDetection { .. } => &["screen", "input"],
        K::SetGlobal { .. } | K::ChangeGlobal { .. } => &["globals"],
        K::BrowserOpen { .. }
        | K::BrowserClick { .. }
        | K::BrowserType { .. }
        | K::BrowserWaitFor { .. }
        | K::BrowserScript { .. } => &["browser"],
        // Listed rather than `_` so a new block has to be decided here.
        K::Wait { .. }
        | K::Comment { .. }
        | K::WhenRan
        | K::WhenBatteryDischargedTo { .. }
        | K::WhenBatteryChargedTo { .. }
        | K::WhenTime { .. }
        | K::WhenPowerPluggedIn
        | K::WhenPowerUnplugged
        | K::WhenKey { .. }
        | K::WhenIdle { .. }
        | K::WhenScreenLock { .. }
        | K::WhenCron { .. }
        | K::WhenNetwork { .. }
        | K::WhenUsb { .. }
        | K::WhenMouseButton { .. }
        | K::WhenAt { .. }
        | K::WhenSignal { .. }
        | K::WhenWebhook { .. }
        | K::SetVariable { .. }
        | K::ChangeVariable { .. }
        | K::AddToList { .. }
        | K::DeleteOfList { .. }
        | K::DeleteAllOfList { .. }
        | K::ShiftList { .. }
        | K::InsertIntoList { .. }
        | K::ReplaceItemOfList { .. }
        | K::ReverseList { .. }
        | K::SplitIntoList { .. }
        | K::BlockHeader { .. }
        | K::CallBlock { .. }
        | K::RunBranch { .. }
        | K::Return { .. }
        | K::If { .. }
        | K::IfElse { .. }
        | K::Repeat { .. }
        | K::Forever { .. }
        | K::While { .. }
        | K::EscapeLoop
        | K::ContinueLoop
        | K::WaitUntil { .. }
        | K::WithLock { .. }
        | K::TryCatch { .. }
        | K::ThrowError { .. }
        | K::Retry { .. }
        | K::StopMacro
        | K::Notify { .. }
        | K::Log { .. }
        | K::AskText { .. }
        | K::AskYesNo { .. }
        | K::AskChoice { .. }
        | K::ShowMessage { .. }
        | K::SendSignal { .. } => &[],
    }
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
        "PixelColor" | "AverageColor" | "ScreenText" => "screen",
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
        if ins.kind.is_header()
            && !matches!(
                ins.kind,
                InstructionKind::WhenRan | InstructionKind::BlockHeader(_)
            )
        {
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
    fn every_block_capability_is_listed_and_every_checked_one_can_ask() {
        let kinds: HashSet<&str> = CAPABILITIES.iter().map(|(k, ..)| *k).collect();
        let blocks = every_block();
        assert!(!blocks.is_empty());
        for kind in blocks {
            for cap in block_capabilities(&kind) {
                assert!(kinds.contains(cap), "{cap} has no label");
            }
            #[cfg(feature = "system")]
            for cap in checked_when_run(&kind) {
                assert!(
                    crate::permissions::GUARDED.iter().any(|(k, _)| *k == cap),
                    "{cap} can't be asked about"
                );
            }
        }
    }

    /// Every Blockwork reporter's operator name, read from `input/value.rs`.
    fn reporter_names() -> Vec<&'static str> {
        let names: Vec<_> = include_str!("input/value.rs")
            .lines()
            .filter_map(|line| line.trim().strip_prefix("op: \"")?.split('"').next())
            .collect();
        assert!(
            names.len() > 40,
            "parsed only {} reporters from input/value.rs",
            names.len()
        );
        names
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

    /// `capabilities.json` is this table for code outside Rust (the community
    /// site's pack review). `BLESS=1 cargo test -p blockwork-core capabilities` rewrites it.
    #[test]
    fn the_exported_table_is_current() {
        use serde_json::{Map, json};
        let mut blocks = Map::new();
        for kind in every_block() {
            let mut caps: Vec<&str> = block_capabilities(&kind).to_vec();
            if kind.is_header()
                && !matches!(
                    kind,
                    InstructionKind::WhenRan | InstructionKind::BlockHeader(_)
                )
            {
                caps.push("triggers");
            }
            // The variant name, as a saved document spells it.
            let name = match serde_json::to_value(&kind).unwrap() {
                serde_json::Value::String(name) => name,
                serde_json::Value::Object(map) => map.keys().next().unwrap().clone(),
                other => panic!("unexpected kind shape {other}"),
            };
            if !caps.is_empty() {
                blocks.insert(name, json!(caps));
            }
        }
        let mut operators = Map::new();
        for name in reporter_names() {
            if let Some(cap) = operator_capability(name) {
                operators.insert(name.to_string(), json!(cap));
            }
        }
        let table = json!({
            "capabilities": CAPABILITIES.iter().map(|(k, label, risky)| json!([k, label, risky])).collect::<Vec<_>>(),
            "blocks": blocks,
            "operators": operators,
        });
        let text = serde_json::to_string_pretty(&table).unwrap() + "\n";
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/capabilities.json");
        if std::env::var_os("BLESS").is_some() {
            std::fs::write(path, &text).unwrap();
        }
        // A Windows checkout may have turned the file's line ends into CRLF.
        let saved = std::fs::read_to_string(path)
            .unwrap_or_default()
            .replace("\r\n", "\n");
        assert!(
            saved == text,
            "capabilities.json is out of date: rerun with BLESS=1"
        );
    }

    #[test]
    fn every_reporter_is_decided() {
        let kinds: HashSet<&str> = CAPABILITIES.iter().map(|(k, ..)| *k).collect();
        for name in reporter_names() {
            match operator_capability(name) {
                Some(cap) => {
                    assert!(kinds.contains(cap), "{cap} has no label");
                    assert!(
                        !UNGUARDED_OPERATORS.contains(&name),
                        "{name} is both guarded and unguarded"
                    );
                }
                None => assert!(
                    UNGUARDED_OPERATORS.contains(&name),
                    "{name} needs an entry in operator_capability or UNGUARDED_OPERATORS"
                ),
            }
        }
    }
}
