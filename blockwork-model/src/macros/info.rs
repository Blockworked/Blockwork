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

            K::WhenBatteryDischargedTo(_)
            | K::WhenBatteryChargedTo(_)
            | K::WhenTime(..)
            | K::WhenPowerPluggedIn
            | K::WhenPowerUnplugged
            | K::WhenKey(..)
            | K::WhenIdle(_)
            | K::WhenScreenLock { .. }
            | K::WhenCron(_)
            | K::WhenNetwork { .. }
            | K::WhenUsb { .. }
            | K::WhenMouseButton { .. }
            | K::WhenAt(_)
            | K::WhenSignal(_)
            | K::WhenWebhook { .. } => of(Trigger, &[]),
            K::WhenClipboardChanged => of(Trigger, &["clipboard"]),
            K::WhenWindow { .. } | K::WhenApp { .. } => of(Trigger, &["windows"]),
            K::WhenFileChanged(_) => of(Trigger, &["files_read"]),
            K::WhenTyped { .. } => of(Trigger, &["input"]),

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
            | K::While { .. } => of(Control, &[]),

            K::SetVariable { .. }
            | K::ChangeVariable { .. }
            | K::AddToList { .. }
            | K::DeleteOfList { .. }
            | K::DeleteAllOfList { .. }
            | K::ShiftList { .. }
            | K::InsertIntoList { .. }
            | K::ReplaceItemOfList { .. }
            | K::ReverseList { .. }
            | K::SplitIntoList { .. } => of(Data, &[]),
            K::SetClipboard(_) => of(Data, &["clipboard"]),

            K::AskText { .. } | K::AskYesNo { .. } | K::AskChoice { .. } | K::ShowMessage(_) => {
                of(Dialogs, &[])
            }

            K::WhenForegroundApp { .. } => of(Trigger, &["screen"]),
            K::LaunchAndroidApp { .. } => of(System, &["commands"]),
            K::Touch { .. } => of(Input, &["input"]),
            K::Element {
                action:
                    crate::input::element::ElementAction::Exists
                    | crate::input::element::ElementAction::ReadText
                    | crate::input::element::ElementAction::Wait,
                ..
            } => of(Input, &["screen"]),
            K::Element { .. } => of(Input, &["screen", "input"]),
            K::Token(_) => of(Input, &["input"]),

            K::SendSignal { .. } | K::Log(_) | K::Notify { .. } => of(System, &[]),
            K::Command(_) | K::CommandWait(_) | K::OpenApp { .. } | K::CloseApp { .. } => {
                of(System, &["commands"])
            }
            K::HttpRequest { .. } => of(System, &["network"]),
            K::WriteFile { .. } => of(System, &["files_write"]),
            K::PasteText(_) => of(System, &["input", "clipboard"]),
            K::WindowAction { .. } | K::MoveWindow { .. } => of(System, &["windows"]),
            K::RunMacro { .. } | K::StopMacroNamed(_) => of(System, &["macros"]),
            K::SetGlobal { .. } | K::ChangeGlobal { .. } => of(System, &["globals"]),

            K::FindImage { .. }
            | K::WaitForImage { .. }
            | K::DetectObjectsInWindow { .. }
            | K::WaitForDetection { .. } => of(Vision, &["screen"]),
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
