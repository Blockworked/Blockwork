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
    ("browser", "Controls your web browser (clicks, types and reads pages)", true),
    ("files_write", "Writes to files", true),
    ("files_read", "Reads files and folders", false),
    ("input", "Presses keys, clicks and moves the mouse", false),
    ("clipboard", "Reads or changes the clipboard", false),
    ("windows", "Reads or controls other apps and windows", false),
    ("triggers", "Starts on its own (schedules, keys, typing, apps, idle, windows, files and more)", false),
    ("screen", "Reads the screen (colors, images and text)", false),
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
        K::WindowAction { .. } | K::MoveWindow { .. } | K::WhenWindow { .. } | K::WhenApp { .. } => &["windows"],
        K::RunMacro { .. } | K::StopMacroNamed(_) => &["macros"],
        K::FindImage { .. } | K::WaitForImage { .. } | K::DetectObjectsInWindow { .. } | K::WaitForDetection { .. } => &["screen"],
        K::ClickImage { .. } | K::AimAtDetection { .. } => &["screen", "input"],
        K::SetGlobal { .. } | K::ChangeGlobal { .. } => &["globals"],
        K::BrowserOpen { .. } | K::BrowserClick { .. } | K::BrowserType { .. } | K::BrowserWaitFor { .. } | K::BrowserScript { .. } => &["browser"],
        // Listed rather than `_` so a new block has to be decided here.
        K::Wait { .. } | K::Comment { .. } | K::WhenRan { .. } | K::WhenBatteryDischargedTo { .. } |
        K::WhenBatteryChargedTo { .. } | K::WhenTime { .. } | K::WhenPowerPluggedIn { .. } |
        K::WhenPowerUnplugged { .. } | K::WhenKey { .. } | K::WhenIdle { .. } | K::WhenScreenLock { .. } |
        K::WhenCron { .. } | K::WhenNetwork { .. } | K::WhenUsb { .. } | K::WhenMouseButton { .. } |
        K::WhenAt { .. } | K::WhenSignal { .. } | K::WhenWebhook { .. } | K::SetVariable { .. } |
        K::ChangeVariable { .. } | K::AddToList { .. } | K::DeleteOfList { .. } | K::DeleteAllOfList { .. } |
        K::ShiftList { .. } | K::InsertIntoList { .. } | K::ReplaceItemOfList { .. } | K::ReverseList { .. } |
        K::SplitIntoList { .. } | K::BlockHeader { .. } | K::CallBlock { .. } | K::RunBranch { .. } |
        K::Return { .. } | K::If { .. } | K::IfElse { .. } | K::Repeat { .. } | K::Forever { .. } |
        K::While { .. } | K::EscapeLoop { .. } | K::ContinueLoop { .. } | K::WaitUntil { .. } |
        K::WithLock { .. } | K::TryCatch { .. } | K::ThrowError { .. } | K::Retry { .. } | K::StopMacro { .. } |
        K::Notify { .. } | K::Log { .. } | K::AskText { .. } | K::AskYesNo { .. } | K::AskChoice { .. } |
        K::ShowMessage { .. } | K::SendSignal { .. } => &[],
    }
}

/// Capabilities the runner doesn't ask a restricted macro about as a block
/// runs. Their reporters still ask (see `input::value`).
const NOT_ASKED_BY_BLOCKS: [&str; 3] = ["clipboard", "windows", "files_read"];

/// What the runner checks before running this block in a restricted macro.
pub fn checked_when_run(kind: &InstructionKind) -> impl Iterator<Item = &'static str> {
    block_capabilities(kind).iter().copied().filter(|k| !NOT_ASKED_BY_BLOCKS.contains(k))
}

/// The capability a reporter needs, by operator name.
pub fn operator_capability(op: &str) -> Option<&'static str> {
    Some(match op {
        "ShellOutput" | "ShellSucceeded" => "commands",
        "HttpGet" => "network",
        "ReadFile" | "FileExists" | "ListDirectory" => "files_read",
        "ClipboardText" | "ClipboardHasImage" | "ClipboardHasFiles" => "clipboard",
        "ActiveWindowTitle" | "ActiveWindowApp" | "WindowExists" | "WindowInfo" | "AppRunning" => "windows",
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
        if ins.kind.is_header() && !matches!(ins.kind, InstructionKind::WhenRan | InstructionKind::BlockHeader(_)) {
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
        // Every block, from the wire round-trip test's fixture.
        let fixtures: serde_json::Value = serde_json::from_str(include_str!("../tests/fixtures/wire_blocks.json")).unwrap();
        let mut seen = 0;
        for block in fixtures.as_array().unwrap() {
            let dto: crate::frontend::InstructionDto = serde_json::from_value(block.clone()).unwrap();
            let kind = crate::frontend::dto_to_instruction(&dto).unwrap().kind;
            seen += 1;
            for cap in block_capabilities(&kind) {
                assert!(kinds.contains(cap), "{cap} has no label");
            }
            #[cfg(feature = "system")]
            for cap in checked_when_run(&kind) {
                assert!(crate::permissions::GUARDED.iter().any(|(k, _)| *k == cap), "{cap} can't be asked about");
            }
        }
        assert!(seen > 0);
    }
}
