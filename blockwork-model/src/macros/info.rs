//! What each block is, in one table: whether it heads a strand, which runner
//! file runs it, and what it can do. `is_header`, `run_block`'s routing and
//! the capability table all read it, so a new block is decided here once.

use super::InstructionKind;

/// What a block is to the runner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    /// Heads a strand that a run or a call starts (`WhenRan`, `BlockHeader`).
    Header,
    /// Heads a strand that starts on its own; a watcher in the app fires it.
    Trigger,
    /// Does nothing when reached.
    Comment,
    /// Run by `runner/control.rs`.
    Control,
    /// Run by `runner/data.rs`.
    Data,
    /// Run by `runner/dialogs.rs`.
    Dialogs,
    /// Run by `runner/input.rs`.
    Input,
    /// Run by `runner/system.rs`.
    System,
    /// Run by `runner/vision.rs`.
    Vision,
    /// Run by `runner/browser.rs`.
    Browser,
}

/// One block's row in the table.
#[derive(Clone, Copy, Debug)]
pub struct BlockInfo {
    pub role: Role,
    /// Keys from `capabilities::CAPABILITIES`: what the block itself does.
    pub capabilities: &'static [&'static str],
}

const fn of(role: Role, capabilities: &'static [&'static str]) -> BlockInfo {
    BlockInfo { role, capabilities }
}

impl InstructionKind {
    /// This block's row. Every variant is listed, so a new one has to be
    /// decided here. `OpenApp`'s command can be any shell string in a
    /// hand-edited file, so it counts as running commands.
    pub fn info(&self) -> BlockInfo {
        use InstructionKind as K;
        use Role::*;
        match self {
            K::WhenRan | K::BlockHeader(_) => of(Header, &[]),
            K::Comment(_) => of(Comment, &[]),
            K::NativeUiAction { .. } => of(System, &["ui_automation"]),
            K::WhenNativeUiChanged { .. } => of(Trigger, &["ui_automation"]),

            K::WhenSystemStarts
            | K::WhenBatteryDischargedTo(_)
            | K::WhenBatteryChargedTo(_)
            | K::WhenTime(..)
            | K::WhenPowerPluggedIn
            | K::WhenPowerUnplugged
            | K::WhenKey { block: false, .. }
            | K::WhenIdle(_)
            | K::WhenTrayItem(_)
            | K::WhenScreenLock { .. }
            | K::WhenCron(_)
            | K::WhenNetwork { .. }
            | K::WhenUsb { .. }
            | K::WhenMouseButton { block: false, .. }
            | K::WhenAt(_)
            | K::WhenSignal(_)
            | K::WhenWebhook { .. } => of(Trigger, &[]),
            K::WhenClipboardChanged => of(Trigger, &["clipboard"]),
            K::WhenWindow { .. } | K::WhenApp { .. } => of(Trigger, &["windows"]),
            K::WhenFileChanged(_) => of(Trigger, &["files_read"]),
            K::WhenTyped { .. }
            | K::WhenKey { block: true, .. }
            | K::WhenMouseButton { block: true, .. }
            | K::RemapKey { .. }
            | K::WhenGamepadButton { .. }
            | K::WhenGamepadConnection { .. }
            | K::WhenGamepadAxis { .. } => of(Trigger, &["input"]),

            K::Wait(_)
            | K::WithLock { .. }
            | K::Return(_)
            | K::EscapeLoop
            | K::ContinueLoop
            | K::Stop { .. }
            | K::ThrowError(_)
            | K::TryCatch { .. }
            | K::Retry { .. }
            | K::WaitUntil { .. }
            | K::CallBlock { .. }
            | K::RunBranch(_)
            | K::If { .. }
            | K::IfElse { .. }
            | K::Repeat { .. }
            | K::Forever { .. }
            | K::While { .. }
            | K::ForEachItem { .. }
            | K::ForEachKey { .. }
            | K::ForEachPart { .. } => of(Control, &[]),
            K::ForEachFile { .. } => of(Control, &["files_read"]),

            K::SetVariable { .. }
            | K::ChangeVariable { .. }
            | K::AddToList { .. }
            | K::DeleteOfList { .. }
            | K::DeleteAllOfList { .. }
            | K::ShiftList { .. }
            | K::InsertIntoList { .. }
            | K::ReplaceItemOfList { .. }
            | K::ReverseList { .. }
            | K::SplitIntoList { .. }
            | K::SetDictKey { .. }
            | K::DeleteDictKey { .. }
            | K::ClearDict { .. }
            | K::DictFromJson { .. } => of(Data, &[]),
            K::SetClipboard(_) => of(Data, &["clipboard"]),

            K::AskText { .. }
            | K::AskYesNo { .. }
            | K::AskChoice { .. }
            | K::AskFile { .. }
            | K::ShowMenu { .. }
            | K::ShowTooltip { .. }
            | K::ShowMessage(_)
            | K::ShowWindow { .. } => of(Dialogs, &[]),
            // Read by the `ShowWindow` around them, never run on their own.
            K::WindowTextBox { .. }
            | K::WindowCheckbox { .. }
            | K::WindowChoice { .. }
            | K::WindowButton { .. }
            | K::WindowNote(_) => of(Comment, &[]),

            K::Token(_)
            | K::TouchGesture { .. }
            | K::TouchSequence { .. }
            | K::GamepadButton { .. }
            | K::GamepadStick { .. }
            | K::GamepadTrigger { .. }
            | K::ResetGamepad => of(Input, &["input"]),
            K::WaitForKey { .. } | K::ReadKeys { .. } | K::WithInputBlocked { .. } => {
                of(Control, &["input"])
            }
            K::WaitForApp { .. } => of(Control, &["windows"]),
            K::WaitForClipboard { .. } => of(Control, &["clipboard"]),

            K::SendSignal { .. } | K::Log(_) | K::Notify { .. } => of(System, &[]),
            K::Command(_) | K::CommandWait(_) | K::OpenApp { .. } | K::CloseApp { .. } => {
                of(System, &["commands"])
            }
            K::HttpRequest { .. } => of(System, &["network"]),
            K::WriteFile { .. } | K::CopyFile { .. } | K::DeleteFile(_) | K::MakeFolder(_) => {
                of(System, &["files_write"])
            }
            K::KillProcess(_) | K::Power { .. } => of(System, &["commands"]),
            K::SetVolume(_) | K::SetMute { .. } => of(System, &[]),
            K::PlaySound { .. } => of(System, &["files_read"]),
            K::SaveClipboard | K::RestoreClipboard => of(Data, &["clipboard"]),
            K::PasteText(_) => of(System, &["input", "clipboard"]),
            K::WindowAction { .. } | K::MoveWindow { .. } | K::SetWindow { .. } => {
                of(System, &["windows"])
            }
            K::UseCoordinates { .. } => of(Control, &["windows"]),
            K::RunMacro { .. } | K::StopMacroNamed(_) => of(System, &["macros"]),
            K::SetGlobal { .. } | K::ChangeGlobal { .. } => of(System, &["globals"]),

            K::FindImage { .. }
            | K::WaitForImage { .. }
            | K::DetectObjectsInWindow { .. }
            | K::WaitForDetection { .. }
            | K::FindColor { .. } => of(Vision, &["screen"]),
            K::ClickImage { .. } | K::AimAtDetection { .. } => of(Vision, &["screen", "input"]),
            K::AimStep { .. } => of(Vision, &["input"]),

            K::BrowserOpen { .. }
            | K::BrowserClick { .. }
            | K::BrowserType { .. }
            | K::BrowserWaitFor { .. }
            | K::BrowserScript { .. } => of(Browser, &["browser"]),
        }
    }
}
