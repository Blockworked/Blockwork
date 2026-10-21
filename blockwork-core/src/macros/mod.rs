//! Blockwork's block vocabulary - the instructions its canvas speaks - and
//! the `Macro` document wrapped around blockstitch's [`BlockGraph`].
//! [`InstructionKind`]'s [`BlockKind`] impl is the hinge between the two.

use crate::input::schedule::{LegacyTimeSchedule, TimeSchedule};
use crate::input::types::{Direction, InputToken, MacroButton};
use crate::input::value::{Op, Value};
use serde::{Deserialize, Serialize};
use std::ops::{Deref, DerefMut};
use uuid::Uuid;

pub mod backend;
pub mod calls;
pub mod conditions;
pub mod debug;
pub mod fields;
pub mod locks;
pub mod migrate;
pub mod loop_control;
pub mod priority;
pub mod gdr_import;
pub mod host_clock;
pub mod run_registry;
pub mod runner;
pub mod thread_pool;
pub mod trace;

pub use blockstitch_core::graph::{
    BlockDef, BlockGraph, BlockKind, BlockPiece, BlockShape, Comment, DictDef, FloatingValue,
    InputValueType, ListDef, ListItem, VariableDef, default_block_color, normalize_block_color,
};
pub use fields::FieldId;

/// One instruction on the canvas: Blockwork's [`InstructionKind`] plus the
/// stable id blockstitch tracks it by.
pub type Instruction = blockstitch_core::graph::Instruction<InstructionKind>;
/// One draggable stack of Blockwork instructions.
pub type Strand = blockstitch_core::graph::Strand<InstructionKind>;
/// Everything on a macro's canvas - see [`Macro`], which owns one.
pub type MacroGraph = BlockGraph<InstructionKind>;

fn default_macro_id() -> String {
    Uuid::new_v4().simple().to_string()
}

fn default_speed_multiplier() -> f64 {
    1.0
}

/// Id of the single implicit strand used before "When Ran" blocks existed.
/// Kept only so loading an old save file can find and migrate that strand.
const LEGACY_ROOT_STRAND_ID: &str = "root";

/// Blockwork's half of the block-editor contract: where its instructions
/// keep values and bodies, which are headers, and how field ids map onto
/// their slots. Every traversal built on this lives in blockstitch.
impl BlockKind for InstructionKind {
    const HEADER_LABEL: &'static str = "When Ran/Block Definition";
    // CallBlock branch callbacks need one body slot per branch (up to 255,
    // see validate_block_pieces), so walks cover every slot and `body_mut`
    // decides per instruction. Other instructions answer `None` past their
    // own slots, so the wider loop costs them nothing.
    const BODY_SLOTS: u8 = u8::MAX;

    fn visit_values_mut(&mut self, f: &mut dyn FnMut(&mut Value, InputValueType)) {
        match self {
            InstructionKind::Token(token) => token.visit_values_mut(f),
            InstructionKind::Wait(value)
            | InstructionKind::Return(value)
            | InstructionKind::WhenBatteryDischargedTo(value)
            | InstructionKind::WhenBatteryChargedTo(value)
            | InstructionKind::WhenKey(value, _)
            | InstructionKind::SetClipboard(value)
            | InstructionKind::SetVariable(_, value)
            | InstructionKind::ChangeVariable(_, value)
            | InstructionKind::Repeat { count: value, .. }
            | InstructionKind::Log(value)
            | InstructionKind::ThrowError(value)
            | InstructionKind::WhenFileChanged(value)
            | InstructionKind::WhenWindow { pattern: value, .. }
            | InstructionKind::WhenTyped { text: value, .. }
            | InstructionKind::PasteText(value)
            | InstructionKind::WhenApp { pattern: value, .. }
            | InstructionKind::WhenIdle(value)
            | InstructionKind::WhenCron(value)
            | InstructionKind::WithLock { name: value, .. }
            | InstructionKind::WhenUsb { pattern: value, .. }
            | InstructionKind::WhenMouseButton { modifiers: value, .. }
            | InstructionKind::AskYesNo { prompt: value, .. }
            | InstructionKind::AskChoice { prompt: value, .. }
            | InstructionKind::ShowMessage(value)
            | InstructionKind::WhenAt(value)
            | InstructionKind::WhenWebhook { path: value, .. } => f(value, InputValueType::Any),
            InstructionKind::WhenSignal(name) | InstructionKind::SendSignal { name, .. } => f(&mut name.value, InputValueType::Any),
            InstructionKind::AskText { prompt, default, .. } => {
                f(prompt, InputValueType::Any);
                f(default, InputValueType::Any);
            }
            InstructionKind::SplitIntoList { text, separator, .. } => {
                f(text, InputValueType::Any);
                f(separator, InputValueType::Any);
            }
            InstructionKind::WriteFile { path, text, .. } => {
                f(path, InputValueType::Any);
                f(text, InputValueType::Any);
            }
            InstructionKind::HttpRequest { url, body, headers, .. } => {
                f(url, InputValueType::Any);
                f(body, InputValueType::Any);
                f(headers, InputValueType::Any);
            }
            InstructionKind::Notify { title, message } => {
                f(title, InputValueType::Any);
                f(message, InputValueType::Any);
            }
            InstructionKind::Retry { count, delay, .. } => {
                f(count, InputValueType::Any);
                f(delay, InputValueType::Any);
            }
            InstructionKind::WindowAction { pattern, .. } => f(pattern, InputValueType::Any),
            InstructionKind::MoveWindow { pattern, x, y, .. } => {
                f(pattern, InputValueType::Any);
                f(x, InputValueType::Any);
                f(y, InputValueType::Any);
            }
            InstructionKind::RunMacro { target, .. } | InstructionKind::StopMacroNamed(target) => {
                f(target, InputValueType::Any)
            }
            InstructionKind::SetGlobal { name, value: other } | InstructionKind::ChangeGlobal { name, by: other } => {
                f(name, InputValueType::Any);
                f(other, InputValueType::Any);
            }
            InstructionKind::WaitUntil { condition, timeout } => {
                f(condition, InputValueType::Bool);
                f(timeout, InputValueType::Any);
            }
            InstructionKind::FindImage { tolerance, .. } | InstructionKind::ClickImage { tolerance, .. } => {
                f(tolerance, InputValueType::Any)
            }
            InstructionKind::WaitForImage { tolerance, timeout, .. } => {
                f(tolerance, InputValueType::Any);
                f(timeout, InputValueType::Any);
            }
            InstructionKind::DetectObjectsInWindow { pattern, detector, label, confidence, .. } => {
                for v in [pattern, detector, label, confidence] {
                    f(v, InputValueType::Any);
                }
            }
            InstructionKind::WaitForDetection { pattern, detector, label, confidence, timeout, .. } => {
                for v in [pattern, detector, label, confidence, timeout] {
                    f(v, InputValueType::Any);
                }
            }
            InstructionKind::AimStep { pattern, target_x, target_y, gain, max_step, deadzone, .. } => {
                for v in [pattern, target_x, target_y, gain, max_step, deadzone] {
                    f(v, InputValueType::Any);
                }
            }
            InstructionKind::AimAtDetection { index, gain, max_step, deadzone, timeout, settle_ms, .. } => {
                for v in [index, gain, max_step, deadzone, timeout, settle_ms] {
                    f(v, InputValueType::Any);
                }
            }
            // A call site's declared Boolean inputs live on the `BlockDef`,
            // not here - `BlockGraph::migrate_bool_slots` handles those.
            InstructionKind::CallBlock { args, .. } => {
                for arg in args {
                    f(arg, InputValueType::Any);
                }
            }
            InstructionKind::If { condition, .. }
            | InstructionKind::IfElse { condition, .. }
            | InstructionKind::While { condition, .. } => f(condition, InputValueType::Bool),
            InstructionKind::AddToList { value, .. } | InstructionKind::InsertIntoList { value, .. } => {
                f(value, InputValueType::Any)
            }
            InstructionKind::DeleteOfList { index, .. } => f(index, InputValueType::Any),
            InstructionKind::ShiftList { amount, .. } => f(amount, InputValueType::Any),
            InstructionKind::ReplaceItemOfList { index, value, .. } => {
                f(index, InputValueType::Any);
                f(value, InputValueType::Any);
            }
            InstructionKind::Command(_)
            | InstructionKind::Comment(_)
            | InstructionKind::WhenRan
            | InstructionKind::BlockHeader(_)
            | InstructionKind::EscapeLoop
            | InstructionKind::ContinueLoop
            | InstructionKind::StopMacro
            | InstructionKind::TryCatch { .. }
            | InstructionKind::WhenTime(..)
            | InstructionKind::WhenPowerPluggedIn
            | InstructionKind::WhenPowerUnplugged
            | InstructionKind::WhenClipboardChanged
            | InstructionKind::WhenScreenLock { .. }
            | InstructionKind::WhenNetwork { .. }
            | InstructionKind::Forever { .. }
            | InstructionKind::OpenApp { .. }
            | InstructionKind::CloseApp { .. }
            | InstructionKind::DeleteAllOfList { .. }
            | InstructionKind::ReverseList { .. }
            | InstructionKind::RunBranch(_) => {}
        }
    }

    /// True for "header" blocks (`WhenRan`, `BlockHeader`, and the event
    /// entry points) - must be first in their strand, nothing may stack
    /// above them, and they render with a flat top edge.
    fn is_header(&self) -> bool {
        matches!(
            self,
            InstructionKind::WhenRan
                | InstructionKind::BlockHeader(_)
                | InstructionKind::WhenBatteryDischargedTo(_)
                | InstructionKind::WhenBatteryChargedTo(_)
                | InstructionKind::WhenTime(..)
                | InstructionKind::WhenPowerPluggedIn
                | InstructionKind::WhenPowerUnplugged
                | InstructionKind::WhenClipboardChanged
                | InstructionKind::WhenKey(..)
                | InstructionKind::WhenWindow { .. }
                | InstructionKind::WhenFileChanged(_)
                | InstructionKind::WhenTyped { .. }
                | InstructionKind::WhenApp { .. }
                | InstructionKind::WhenIdle(_)
                | InstructionKind::WhenScreenLock { .. }
                | InstructionKind::WhenCron(_)
                | InstructionKind::WhenNetwork { .. }
                | InstructionKind::WhenUsb { .. }
                | InstructionKind::WhenMouseButton { .. }
                | InstructionKind::WhenAt(_)
                | InstructionKind::WhenSignal(_)
                | InstructionKind::WhenWebhook { .. }
        )
    }

    fn body(&self, slot: u8) -> Option<&Vec<Instruction>> {
        match (self, slot) {
            (InstructionKind::If { body, .. }, 0) => Some(body),
            (InstructionKind::IfElse { then_body, .. }, 0) => Some(then_body),
            (InstructionKind::IfElse { else_body, .. }, 1) => Some(else_body),
            (InstructionKind::Repeat { body, .. }, 0) => Some(body),
            (InstructionKind::Forever { body }, 0) => Some(body),
            (InstructionKind::While { body, .. }, 0) => Some(body),
            (InstructionKind::TryCatch { body, .. }, 0) => Some(body),
            (InstructionKind::TryCatch { catch_body, .. }, 1) => Some(catch_body),
            (InstructionKind::Retry { body, .. }, 0) => Some(body),
            (InstructionKind::WithLock { body, .. }, 0) => Some(body),
            (InstructionKind::CallBlock { branches, .. }, slot) => branches.get(slot as usize),
            _ => None,
        }
    }

    fn body_mut(&mut self, slot: u8) -> Option<&mut Vec<Instruction>> {
        match (self, slot) {
            (InstructionKind::If { body, .. }, 0) => Some(body),
            (InstructionKind::IfElse { then_body, .. }, 0) => Some(then_body),
            (InstructionKind::IfElse { else_body, .. }, 1) => Some(else_body),
            (InstructionKind::Repeat { body, .. }, 0) => Some(body),
            (InstructionKind::Forever { body }, 0) => Some(body),
            (InstructionKind::While { body, .. }, 0) => Some(body),
            (InstructionKind::TryCatch { body, .. }, 0) => Some(body),
            (InstructionKind::TryCatch { catch_body, .. }, 1) => Some(catch_body),
            (InstructionKind::Retry { body, .. }, 0) => Some(body),
            (InstructionKind::WithLock { body, .. }, 0) => Some(body),
            (InstructionKind::CallBlock { branches, .. }, slot) => branches.get_mut(slot as usize),
            _ => None,
        }
    }

    fn variable_target_mut(&mut self) -> Option<&mut String> {
        match self {
            InstructionKind::SetVariable(name, _)
            | InstructionKind::ChangeVariable(name, _)
            | InstructionKind::HttpRequest { variable: name, .. }
            | InstructionKind::AskText { name, .. }
            | InstructionKind::AskYesNo { name, .. }
            | InstructionKind::AskChoice { name, .. }
            | InstructionKind::WhenWebhook { variable: name, .. }
            | InstructionKind::DetectObjectsInWindow { count_variable: name, .. }
            | InstructionKind::AimAtDetection { status_variable: name, .. } => Some(name),
            _ => None,
        }
    }

    fn list_target_mut(&mut self) -> Option<&mut String> {
        match self {
            InstructionKind::AddToList { name, .. }
            | InstructionKind::DeleteOfList { name, .. }
            | InstructionKind::DeleteAllOfList { name }
            | InstructionKind::ShiftList { name, .. }
            | InstructionKind::InsertIntoList { name, .. }
            | InstructionKind::ReplaceItemOfList { name, .. }
            | InstructionKind::ReverseList { name }
            | InstructionKind::SplitIntoList { name, .. }
            | InstructionKind::AskChoice { list: name, .. }
            | InstructionKind::DetectObjectsInWindow { results_list: name, .. }
            | InstructionKind::AimAtDetection { results_list: name, .. } => Some(name),
            _ => None,
        }
    }

    fn calls_block(&self, block_id: &str) -> bool {
        matches!(self, InstructionKind::CallBlock { block_id: id, .. } if id == block_id)
    }

    fn call_args_mut(&mut self, block_id: &str) -> Option<&mut Vec<Value>> {
        match self {
            InstructionKind::CallBlock { block_id: id, args, .. } if id == block_id => Some(args),
            _ => None,
        }
    }

    fn block_header_id(&self) -> Option<&str> {
        match self {
            InstructionKind::BlockHeader(id) => Some(id),
            _ => None,
        }
    }

    fn value_slot_mut(&mut self, field: &str) -> Option<&mut Value> {
        fields::value_slot_mut(self, field.parse().ok()?)
    }

    fn blank_field_value(&self, field: &str, blocks: &[BlockDef]) -> Option<Value> {
        fields::blank_field_value(self, field.parse().ok()?, blocks)
    }

    fn field_requires_integer(&self, field: &str) -> bool {
        field
            .parse()
            .is_ok_and(|field: FieldId| field.requires_integer())
    }
}

/// A named choice or an unrestricted expression, both resolved as text at runtime.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Hash)]
#[serde(from = "SignalNameDe")]
pub struct SignalName {
    pub value: Value,
    pub expression: bool,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum SignalNameDe {
    Current { value: Value, expression: bool },
    Legacy(Value),
}

impl From<SignalNameDe> for SignalName {
    fn from(value: SignalNameDe) -> Self {
        match value {
            SignalNameDe::Current { value, expression } => Self::new(value, expression),
            SignalNameDe::Legacy(value) => Self::new(value, false),
        }
    }
}

impl SignalName {
    pub fn new(value: Value, expression: bool) -> Self {
        let expression = expression || !matches!(value, Value::Text { .. });
        Self { value, expression }
    }
}

impl From<Value> for SignalName {
    fn from(value: Value) -> Self {
        Self::new(value, false)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(from = "InstructionKindDe")]
pub enum InstructionKind {
    Token(InputToken),
    Wait(Value),
    Command(String),
    Comment(String),
    /// Marks a strand as an entry point (always at index 0): it runs as its
    /// own concurrent thread when the macro runs. A macro can have several.
    WhenRan,
    /// Header-only marker (like `WhenRan`/`BlockHeader`) for a strand whose
    /// body should run whenever the system's battery charge drops to (or
    /// below) the given percentage. Unlike `WhenRan`, this is *not* an
    /// entry point Run/Loop invokes - `runner::run_with_offset` skips these
    /// strands entirely. Instead they're driven independently by a
    /// long-running background watcher outside a macro run altogether (in
    /// the desktop app, `blockwork-app`'s `battery_watch` module), which polls
    /// the battery, fires the strand's body (everything after this marker)
    /// the moment the condition holds, and won't fire it again until the
    /// battery recovers past the threshold and crosses it again.
    WhenBatteryDischargedTo(Value),
    /// Same as `WhenBatteryDischargedTo`, but fires when the battery charge
    /// rises to (or above) the given percentage instead.
    WhenBatteryChargedTo(Value),
    /// Header-only marker, same shape/semantics as `WhenBatteryDischargedTo`
    /// (excluded from Run/Loop, driven by a background watcher - `time_watch`
    /// in the desktop app) but for a recurring point in local time instead
    /// of a battery level. See `TimeSchedule` for the recurrence shapes; the
    /// value is the time of day, like `09:30` (see `parse_time_of_day`).
    WhenTime(TimeSchedule, Value),
    /// Header-only marker, no payload (like `WhenRan`) - excluded from
    /// Run/Loop and driven by the same background watcher as
    /// `WhenBattery*To` (`battery_watch` in the desktop app), which fires
    /// this strand's body the moment the system starts receiving external
    /// power. See `crate::battery::is_plugged_in`.
    WhenPowerPluggedIn,
    /// Same as `WhenPowerPluggedIn`, but fires when external power is lost
    /// instead - never fires at all on a system with no battery/UPS, since
    /// `is_plugged_in` is always `true` there.
    WhenPowerUnplugged,
    /// Header-only marker, no payload (like `WhenPowerPluggedIn`) - excluded
    /// from Run/Loop and driven by a background watcher (`clipboard_watch`
    /// in the desktop app), which fires this strand's body whenever the
    /// clipboard's contents change. Edge-triggered like the other watcher
    /// headers, but with no threshold to recover past - any change fires it.
    WhenClipboardChanged,
    /// Header-only marker driven by the key watcher (`key_watch` in the
    /// desktop app): fires the body when the key combo in the value is
    /// pressed or released. `Click` is treated as `Press`.
    WhenKey(Value, Direction),
    /// Launches an installed application, chosen via the desktop app's "Open
    /// App" picker (`blockwork-app`'s `installed_apps` module lists candidates).
    /// `command` is the already-resolved, platform-specific launch string
    /// (a cleaned freedesktop `Exec=` line on Linux, a `.lnk` path on
    /// Windows, an `.app` bundle path on macOS) captured at pick time -
    /// running it later never re-queries the installed-app list. `name` and
    /// `icon` (a `data:` URI, when one was found) are cached at the same
    /// time purely for display, so the block keeps showing the right label
    /// and picture even if the app is later renamed or uninstalled.
    OpenApp {
        command: String,
        name: String,
        icon: Option<String>,
    },
    /// Same picker/payload shape as `OpenApp`, but terminates the app
    /// instead of launching it - `runner::close_app` derives a process
    /// matcher from `command` (and, on macOS, `name`) rather than executing
    /// it directly. `command`/`name`/`icon` are cached at pick time for the
    /// exact same reason `OpenApp`'s are.
    CloseApp {
        command: String,
        name: String,
        icon: Option<String>,
    },
    /// `set <name> to <value>` - overwrites the named variable.
    SetVariable(String, Value),
    /// `change <name> by <value>` - adds `value` to the named variable.
    /// No-op if `value` isn't numeric; the variable is coerced to `0` first
    /// if it wasn't already numeric.
    ChangeVariable(String, Value),
    /// `set clipboard to <value>` — overwrites the system clipboard with the
    /// value's text. See `crate::clipboard::set_text`.
    SetClipboard(Value),
    /// Appends a number/text value to a named list. Boolean values are ignored.
    AddToList {
        value: Value,
        name: String,
    },
    /// Removes the 1-based item at `index` from a named list.
    DeleteOfList {
        index: Value,
        name: String,
    },
    /// Removes every item from a named list.
    DeleteAllOfList {
        name: String,
    },
    /// Rotates a named list by `amount` positions (positive is toward the end).
    ShiftList {
        name: String,
        amount: Value,
    },
    /// Inserts a number/text value at the 1-based `index` in a named list.
    InsertIntoList {
        value: Value,
        index: Value,
        name: String,
    },
    /// Replaces the 1-based item at `index` in a named list with a literal.
    ReplaceItemOfList {
        index: Value,
        name: String,
        value: Value,
    },
    /// Reverses a named list in place.
    ReverseList {
        name: String,
    },
    /// Marks a strand as a custom block's body; the `String` is the
    /// `BlockDef::id`. Header-only, like `WhenRan`, but never auto-runs -
    /// only invoked via `CallBlock`/`Value::Call`.
    BlockHeader(String),
    /// Command-position invocation of a `Normal`/`Ending`-shaped custom
    /// block: runs its body inline with `args` bound to its inputs.
    CallBlock {
        block_id: String,
        args: Vec<Value>,
        #[serde(default)]
        branches: Vec<Vec<Instruction>>,
    },
    /// Runs one callback branch passed to the enclosing custom-block call.
    RunBranch(String),
    /// Only meaningful inside a `ReturnsValue`/`ReturnsBool`-shaped block's
    /// body: evaluates `Value` and halts execution, returning the result to
    /// the caller.
    Return(Value),
    /// `if <condition> then { body }` - runs `body` inline (same strand,
    /// same depth) when `condition` evaluates truthy.
    If {
        condition: Value,
        body: Vec<Instruction>,
    },
    /// `if <condition> then { then_body } else { else_body }`.
    IfElse {
        condition: Value,
        then_body: Vec<Instruction>,
        else_body: Vec<Instruction>,
    },
    /// `repeat <count> { body }` - runs `body` `count` times (rounded,
    /// clamped to non-negative).
    Repeat {
        count: Value,
        body: Vec<Instruction>,
    },
    /// `forever { body }` - runs `body` in an unconditional loop; only ends
    /// via `EscapeLoop`, a `Return` inside it, or the run being stopped.
    Forever {
        body: Vec<Instruction>,
    },
    /// `while <condition> { body }` - re-evaluates `condition` before every
    /// iteration, running `body` for as long as it's truthy.
    While {
        condition: Value,
        body: Vec<Instruction>,
    },
    /// Stops the nearest enclosing `Repeat`/`Forever`/`While` immediately.
    /// A no-op if not inside a loop.
    EscapeLoop,
    /// Skips straight to the next iteration of the nearest enclosing
    /// `Repeat`/`Forever`/`While`. A no-op if not inside a loop.
    ContinueLoop,
    /// `write`/`append <text> to file <path>`.
    WriteFile {
        path: Value,
        text: Value,
        append: bool,
    },
    /// Sends an HTTP request and stores the response body in `variable`
    /// (skipped when empty). A 4xx/5xx status is an error.
    HttpRequest {
        method: String,
        url: Value,
        body: Value,
        headers: Value,
        variable: String,
    },
    /// Shows a desktop notification.
    Notify {
        title: Value,
        message: Value,
    },
    /// Writes a line to the run log.
    Log(Value),
    /// `try { body } on error { catch_body }`: an error inside `body` stops
    /// it and runs `catch_body`, where the `LastError` reporter has the message.
    TryCatch {
        body: Vec<Instruction>,
        catch_body: Vec<Instruction>,
    },
    /// Fails with the given message: ends the strand unless a `TryCatch`
    /// catches it.
    ThrowError(Value),
    /// Stops every strand of the current run.
    StopMacro,
    /// Runs `body`; if a block in it fails, waits `delay` ms and runs it
    /// again, up to `count` attempts in all.
    Retry {
        count: Value,
        delay: Value,
        body: Vec<Instruction>,
    },
    /// One of `window::ACTIONS` on the first window whose title or app
    /// (`by`, a `window::MatchBy` name) contains `pattern` (the active
    /// window when empty).
    WindowAction {
        action: String,
        pattern: Value,
        by: String,
    },
    /// Moves a matching window to (`x`, `y`), or with `resize` sizes it
    /// to `x` by `y`.
    MoveWindow {
        pattern: Value,
        by: String,
        resize: bool,
        x: Value,
        y: Value,
    },
    /// Starts another macro, found by name; with `wait` the strand pauses
    /// until that run ends.
    RunMacro {
        target: Value,
        wait: bool,
    },
    /// Stops every run of the named macro.
    StopMacroNamed(Value),
    /// Sets a global variable: shared by every macro and kept across restarts.
    SetGlobal {
        name: Value,
        value: Value,
    },
    /// Adds `by` to a global variable.
    ChangeGlobal {
        name: Value,
        by: Value,
    },
    /// Pauses until `condition` is true, failing after `timeout` ms (0 = never).
    WaitUntil {
        condition: Value,
        timeout: Value,
    },
    /// Looks for `image` (a PNG `data:` URI or file path) on screen and puts
    /// the screen position of its middle in two variables, -1 if it's not there.
    FindImage {
        image: String,
        tolerance: Value,
        x_variable: String,
        y_variable: String,
    },
    /// Clicks the middle of `image` on screen; an error if it's not there.
    ClickImage {
        image: String,
        tolerance: Value,
        button: MacroButton,
    },
    /// Pauses until `image` appears (or with `appear` false, disappears),
    /// failing after `timeout` ms (0 = never).
    WaitForImage {
        image: String,
        tolerance: Value,
        appear: bool,
        timeout: Value,
    },
    /// Header for a strand fired when a window whose title or app contains
    /// `pattern` gains (`focused`) or loses focus. Driven by `window_watch`
    /// in the desktop app, like the other `When*` watcher headers.
    WhenWindow {
        pattern: Value,
        focused: bool,
    },
    /// Header for a strand fired when the file at the path is created,
    /// changed or deleted. Driven by `file_watch` in the desktop app.
    WhenFileChanged(Value),
    /// Header for a strand fired when the user types `text` (text
    /// expansion); with `erase` the typed text is backspaced away first.
    /// Driven by `type_watch` in the desktop app.
    WhenTyped {
        text: Value,
        erase: bool,
    },
    /// Pastes text through the clipboard, then puts the old clipboard back.
    PasteText(Value),
    /// Header for a strand fired when a program whose name contains
    /// `pattern` starts (or with `started` false, exits).
    WhenApp {
        pattern: Value,
        started: bool,
    },
    /// Header for a strand fired once there has been no keyboard or mouse
    /// input for the given number of minutes.
    WhenIdle(Value),
    /// Header for a strand fired when the screen locks (or unlocks).
    WhenScreenLock {
        locked: bool,
    },
    /// Runs `body` while holding the named lock, so only one strand in any
    /// macro runs a block guarded by the same name at a time.
    WithLock {
        name: Value,
        body: Vec<Instruction>,
    },
    /// Header for a strand fired on a cron schedule, like `*/15 9-17 * * mon-fri`.
    WhenCron(Value),
    /// Replaces the list `name` with `text` cut at every `separator`.
    SplitIntoList {
        text: Value,
        separator: Value,
        name: String,
    },
    /// Header for a strand fired when the computer goes online (or offline).
    WhenNetwork {
        connected: bool,
    },
    /// Header for a strand fired when a USB device whose name or ids contain
    /// `pattern` (any device when empty) is plugged in (or unplugged).
    WhenUsb {
        pattern: Value,
        connected: bool,
    },
    /// Header for a strand fired when a mouse button is pressed (or
    /// released), optionally only while the modifier keys in `modifiers` are held.
    WhenMouseButton {
        button: MacroButton,
        pressed: bool,
        modifiers: Value,
    },
    /// Asks for text in a dialog and puts the answer in variable `name`.
    /// Cancelling is an error.
    AskText {
        prompt: Value,
        default: Value,
        name: String,
    },
    /// Asks a yes/no question and puts true or false in variable `name`.
    AskYesNo {
        prompt: Value,
        name: String,
    },
    /// Asks to pick an item of list `list` and puts it in variable `name`.
    AskChoice {
        prompt: Value,
        list: String,
        name: String,
    },
    /// Shows a message in a dialog and waits until it's closed.
    ShowMessage(Value),
    /// Header for a strand fired once at a date and time, like `2026-12-24 18:00`.
    WhenAt(Value),
    /// Header for a strand started by a `SendSignal` of the same name in this macro.
    WhenSignal(SignalName),
    /// Starts this macro's `WhenSignal` strands named `name`, sharing this
    /// run's variables; with `wait`, until they've all ended.
    SendSignal {
        name: SignalName,
        wait: bool,
    },
    /// Header for a strand fired by a web request to `path` on the local
    /// webhook server. The request body goes in variable `variable`.
    WhenWebhook {
        path: Value,
        variable: String,
    },
    /// One detection pass over a matching window (or `region` of it, as
    /// `x,y,width,height`; empty for all): boxes from `detector` (a local
    /// URL) scoring at least `confidence` percent replace list
    /// `results_list`, one JSON item each, ordered by `sort`.
    DetectObjectsInWindow {
        pattern: Value,
        by: String,
        detector: Value,
        label: Value,
        confidence: Value,
        sort: String,
        results_list: String,
        count_variable: String,
        region: String,
    },
    /// Pauses until `label` appears in the window (or with `appear` false,
    /// is gone), failing after `timeout` ms (0 = never).
    WaitForDetection {
        pattern: Value,
        by: String,
        detector: Value,
        label: Value,
        confidence: Value,
        appear: bool,
        timeout: Value,
        region: String,
    },
    /// One relative mouse move from the window's center toward the screen
    /// point (`target_x`, `target_y`); see `detect::AimController`.
    AimStep {
        pattern: Value,
        by: String,
        target_x: Value,
        target_y: Value,
        gain: Value,
        max_step: Value,
        deadzone: Value,
    },
    /// Follows item `index` of a list a `DetectObjectsInWindow` filled,
    /// re-detecting and stepping until it's held centered for `settle_ms`.
    /// Writes `hit`, `timeout` or `lost` to `status_variable`.
    AimAtDetection {
        results_list: String,
        index: Value,
        gain: Value,
        max_step: Value,
        deadzone: Value,
        timeout: Value,
        settle_ms: Value,
        status_variable: String,
    },
}

impl std::hash::Hash for InstructionKind {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            Self::Token(t) => {
                0u8.hash(state);
                t.hash(state);
            }
            Self::Wait(d) => {
                1u8.hash(state);
                d.hash(state);
            }
            Self::Command(s) => {
                2u8.hash(state);
                s.hash(state);
            }
            Self::Comment(s) => {
                3u8.hash(state);
                s.hash(state);
            }
            Self::WhenRan => {
                4u8.hash(state);
            }
            Self::SetVariable(n, v) => {
                5u8.hash(state);
                n.hash(state);
                v.hash(state);
            }
            Self::ChangeVariable(n, v) => {
                6u8.hash(state);
                n.hash(state);
                v.hash(state);
            }
            Self::BlockHeader(id) => {
                7u8.hash(state);
                id.hash(state);
            }
            Self::CallBlock {
                block_id,
                args,
                branches,
            } => {
                8u8.hash(state);
                block_id.hash(state);
                args.hash(state);
                branches.hash(state);
            }
            Self::RunBranch(name) => {
                26u8.hash(state);
                name.hash(state);
            }
            Self::Return(v) => {
                9u8.hash(state);
                v.hash(state);
            }
            Self::If { condition, body } => {
                10u8.hash(state);
                condition.hash(state);
                body.hash(state);
            }
            Self::IfElse {
                condition,
                then_body,
                else_body,
            } => {
                11u8.hash(state);
                condition.hash(state);
                then_body.hash(state);
                else_body.hash(state);
            }
            Self::Repeat { count, body } => {
                12u8.hash(state);
                count.hash(state);
                body.hash(state);
            }
            Self::Forever { body } => {
                13u8.hash(state);
                body.hash(state);
            }
            Self::While { condition, body } => {
                14u8.hash(state);
                condition.hash(state);
                body.hash(state);
            }
            Self::EscapeLoop => {
                15u8.hash(state);
            }
            Self::ContinueLoop => {
                16u8.hash(state);
            }
            Self::WhenBatteryDischargedTo(v) => {
                17u8.hash(state);
                v.hash(state);
            }
            Self::WhenBatteryChargedTo(v) => {
                18u8.hash(state);
                v.hash(state);
            }
            Self::WhenTime(s, t) => {
                19u8.hash(state);
                s.hash(state);
                t.hash(state);
            }
            Self::WhenPowerPluggedIn => {
                20u8.hash(state);
            }
            Self::WhenPowerUnplugged => {
                21u8.hash(state);
            }
            Self::OpenApp {
                command,
                name,
                icon,
            } => {
                22u8.hash(state);
                command.hash(state);
                name.hash(state);
                icon.hash(state);
            }
            Self::CloseApp {
                command,
                name,
                icon,
            } => {
                23u8.hash(state);
                command.hash(state);
                name.hash(state);
                icon.hash(state);
            }
            Self::AddToList { value, name } => {
                24u8.hash(state);
                value.hash(state);
                name.hash(state);
            }
            Self::DeleteOfList { index, name } => {
                25u8.hash(state);
                index.hash(state);
                name.hash(state);
            }
            Self::DeleteAllOfList { name } => {
                26u8.hash(state);
                name.hash(state);
            }
            Self::ShiftList { name, amount } => {
                27u8.hash(state);
                name.hash(state);
                amount.hash(state);
            }
            Self::InsertIntoList { value, index, name } => {
                28u8.hash(state);
                value.hash(state);
                index.hash(state);
                name.hash(state);
            }
            Self::ReplaceItemOfList { index, name, value } => {
                29u8.hash(state);
                index.hash(state);
                name.hash(state);
                value.hash(state);
            }
            Self::ReverseList { name } => {
                30u8.hash(state);
                name.hash(state);
            }
            Self::SetClipboard(v) => {
                31u8.hash(state);
                v.hash(state);
            }
            Self::WhenClipboardChanged => {
                32u8.hash(state);
            }
            Self::WhenKey(key, direction) => {
                33u8.hash(state);
                key.hash(state);
                direction.hash(state);
            }
            Self::WriteFile { path, text, append } => {
                34u8.hash(state);
                path.hash(state);
                text.hash(state);
                append.hash(state);
            }
            Self::HttpRequest {
                method,
                url,
                body,
                headers,
                variable,
            } => {
                35u8.hash(state);
                method.hash(state);
                url.hash(state);
                body.hash(state);
                headers.hash(state);
                variable.hash(state);
            }
            Self::Notify { title, message } => {
                36u8.hash(state);
                title.hash(state);
                message.hash(state);
            }
            Self::Log(v) => {
                37u8.hash(state);
                v.hash(state);
            }
            Self::TryCatch { body, catch_body } => {
                38u8.hash(state);
                body.hash(state);
                catch_body.hash(state);
            }
            Self::ThrowError(v) => {
                39u8.hash(state);
                v.hash(state);
            }
            Self::StopMacro => {
                40u8.hash(state);
            }
            Self::WhenWindow { pattern, focused } => {
                41u8.hash(state);
                pattern.hash(state);
                focused.hash(state);
            }
            Self::WhenFileChanged(path) => {
                42u8.hash(state);
                path.hash(state);
            }
            Self::Retry { count, delay, body } => {
                43u8.hash(state);
                count.hash(state);
                delay.hash(state);
                body.hash(state);
            }
            Self::WindowAction { action, pattern, by } => {
                44u8.hash(state);
                action.hash(state);
                pattern.hash(state);
                by.hash(state);
            }
            Self::MoveWindow { pattern, by, resize, x, y } => {
                45u8.hash(state);
                pattern.hash(state);
                by.hash(state);
                resize.hash(state);
                x.hash(state);
                y.hash(state);
            }
            Self::RunMacro { target, wait } => {
                46u8.hash(state);
                target.hash(state);
                wait.hash(state);
            }
            Self::StopMacroNamed(target) => {
                47u8.hash(state);
                target.hash(state);
            }
            Self::SetGlobal { name, value } => {
                48u8.hash(state);
                name.hash(state);
                value.hash(state);
            }
            Self::ChangeGlobal { name, by } => {
                49u8.hash(state);
                name.hash(state);
                by.hash(state);
            }
            Self::WaitUntil { condition, timeout } => {
                50u8.hash(state);
                condition.hash(state);
                timeout.hash(state);
            }
            Self::FindImage { image, tolerance, x_variable, y_variable } => {
                51u8.hash(state);
                image.hash(state);
                tolerance.hash(state);
                x_variable.hash(state);
                y_variable.hash(state);
            }
            Self::ClickImage { image, tolerance, button } => {
                52u8.hash(state);
                image.hash(state);
                tolerance.hash(state);
                button.hash(state);
            }
            Self::WaitForImage { image, tolerance, appear, timeout } => {
                53u8.hash(state);
                image.hash(state);
                tolerance.hash(state);
                appear.hash(state);
                timeout.hash(state);
            }
            Self::WhenTyped { text, erase } => {
                54u8.hash(state);
                text.hash(state);
                erase.hash(state);
            }
            Self::PasteText(v) => {
                55u8.hash(state);
                v.hash(state);
            }
            Self::WhenApp { pattern, started } => {
                56u8.hash(state);
                pattern.hash(state);
                started.hash(state);
            }
            Self::WhenIdle(v) => {
                57u8.hash(state);
                v.hash(state);
            }
            Self::WhenScreenLock { locked } => {
                58u8.hash(state);
                locked.hash(state);
            }
            Self::WithLock { name, body } => {
                59u8.hash(state);
                name.hash(state);
                body.hash(state);
            }
            Self::WhenCron(v) => {
                60u8.hash(state);
                v.hash(state);
            }
            Self::SplitIntoList { text, separator, name } => {
                61u8.hash(state);
                text.hash(state);
                separator.hash(state);
                name.hash(state);
            }
            Self::WhenNetwork { connected } => {
                62u8.hash(state);
                connected.hash(state);
            }
            Self::WhenUsb { pattern, connected } => {
                63u8.hash(state);
                pattern.hash(state);
                connected.hash(state);
            }
            Self::WhenMouseButton { button, pressed, modifiers } => {
                64u8.hash(state);
                button.hash(state);
                pressed.hash(state);
                modifiers.hash(state);
            }
            Self::AskText { prompt, default, name } => {
                65u8.hash(state);
                prompt.hash(state);
                default.hash(state);
                name.hash(state);
            }
            Self::AskYesNo { prompt, name } => {
                66u8.hash(state);
                prompt.hash(state);
                name.hash(state);
            }
            Self::AskChoice { prompt, list, name } => {
                67u8.hash(state);
                prompt.hash(state);
                list.hash(state);
                name.hash(state);
            }
            Self::ShowMessage(v) => {
                68u8.hash(state);
                v.hash(state);
            }
            Self::WhenAt(v) => {
                69u8.hash(state);
                v.hash(state);
            }
            Self::WhenSignal(v) => {
                70u8.hash(state);
                v.hash(state);
            }
            Self::SendSignal { name, wait } => {
                71u8.hash(state);
                name.hash(state);
                wait.hash(state);
            }
            Self::WhenWebhook { path, variable } => {
                72u8.hash(state);
                path.hash(state);
                variable.hash(state);
            }
            Self::DetectObjectsInWindow { pattern, by, detector, label, confidence, sort, results_list, count_variable, region } => {
                73u8.hash(state);
                (pattern, by, detector, label, confidence).hash(state);
                (sort, results_list, count_variable, region).hash(state);
            }
            Self::WaitForDetection { pattern, by, detector, label, confidence, appear, timeout, region } => {
                74u8.hash(state);
                (pattern, by, detector, label, confidence, appear, timeout, region).hash(state);
            }
            Self::AimStep { pattern, by, target_x, target_y, gain, max_step, deadzone } => {
                75u8.hash(state);
                (pattern, by, target_x, target_y, gain, max_step, deadzone).hash(state);
            }
            Self::AimAtDetection { results_list, index, gain, max_step, deadzone, timeout, settle_ms, status_variable } => {
                76u8.hash(state);
                (results_list, index, gain, max_step, deadzone).hash(state);
                (timeout, settle_ms, status_variable).hash(state);
            }
        }
    }
}

/// `WhenTime` as saved now (recurrence + time value), or before the time
/// became a value (one object with a fixed `hour`/`minute`).
#[derive(Deserialize)]
#[serde(untagged)]
enum WhenTimeDe {
    Current(TimeSchedule, Value),
    Legacy(LegacyTimeSchedule),
}

#[derive(Deserialize)]
enum InstructionKindDe {
    Token(InputToken),
    Wait(WaitDe),
    Command(String),
    Comment(String),
    WhenRan,
    WhenBatteryDischargedTo(Value),
    WhenBatteryChargedTo(Value),
    WhenTime(WhenTimeDe),
    WhenPowerPluggedIn,
    WhenPowerUnplugged,
    WhenClipboardChanged,
    WhenKey(Value, Direction),
    OpenApp {
        command: String,
        name: String,
        icon: Option<String>,
    },
    CloseApp {
        command: String,
        name: String,
        icon: Option<String>,
    },
    SetVariable(String, Value),
    ChangeVariable(String, Value),
    SetClipboard(Value),
    AddToList {
        value: Value,
        name: String,
    },
    DeleteOfList {
        index: Value,
        name: String,
    },
    DeleteAllOfList {
        name: String,
    },
    ShiftList {
        name: String,
        amount: Value,
    },
    InsertIntoList {
        value: Value,
        index: Value,
        name: String,
    },
    ReplaceItemOfList {
        index: Value,
        name: String,
        value: Value,
    },
    ReverseList {
        name: String,
    },
    BlockHeader(String),
    CallBlock {
        block_id: String,
        args: Vec<Value>,
        #[serde(default)]
        branches: Vec<Vec<Instruction>>,
    },
    RunBranch(String),
    Return(Value),
    If {
        condition: Value,
        body: Vec<Instruction>,
    },
    IfElse {
        condition: Value,
        then_body: Vec<Instruction>,
        else_body: Vec<Instruction>,
    },
    Repeat {
        count: Value,
        body: Vec<Instruction>,
    },
    Forever {
        body: Vec<Instruction>,
    },
    While {
        condition: Value,
        body: Vec<Instruction>,
    },
    EscapeLoop,
    ContinueLoop,
    WriteFile {
        path: Value,
        text: Value,
        append: bool,
    },
    HttpRequest {
        method: String,
        url: Value,
        body: Value,
        headers: Value,
        variable: String,
    },
    Notify {
        title: Value,
        message: Value,
    },
    Log(Value),
    TryCatch {
        body: Vec<Instruction>,
        catch_body: Vec<Instruction>,
    },
    ThrowError(Value),
    StopMacro,
    WhenWindow {
        pattern: Value,
        focused: bool,
    },
    WhenFileChanged(Value),
    Retry {
        count: Value,
        delay: Value,
        body: Vec<Instruction>,
    },
    WindowAction {
        action: String,
        pattern: Value,
        #[serde(default = "match_by_title")]
        by: String,
    },
    MoveWindow {
        pattern: Value,
        #[serde(default = "match_by_title")]
        by: String,
        resize: bool,
        x: Value,
        y: Value,
    },
    RunMacro {
        target: Value,
        wait: bool,
    },
    StopMacroNamed(Value),
    SetGlobal {
        name: Value,
        value: Value,
    },
    ChangeGlobal {
        name: Value,
        by: Value,
    },
    WaitUntil {
        condition: Value,
        timeout: Value,
    },
    FindImage {
        image: String,
        tolerance: Value,
        x_variable: String,
        y_variable: String,
    },
    ClickImage {
        image: String,
        tolerance: Value,
        button: MacroButton,
    },
    WaitForImage {
        image: String,
        tolerance: Value,
        appear: bool,
        timeout: Value,
    },
    WhenTyped {
        text: Value,
        erase: bool,
    },
    PasteText(Value),
    WhenApp {
        pattern: Value,
        started: bool,
    },
    WhenIdle(Value),
    WhenScreenLock {
        locked: bool,
    },
    WithLock {
        name: Value,
        body: Vec<Instruction>,
    },
    WhenCron(Value),
    SplitIntoList {
        text: Value,
        separator: Value,
        name: String,
    },
    WhenNetwork {
        connected: bool,
    },
    WhenUsb {
        pattern: Value,
        connected: bool,
    },
    WhenMouseButton {
        button: MacroButton,
        pressed: bool,
        #[serde(default = "empty_text")]
        modifiers: Value,
    },
    AskText {
        prompt: Value,
        default: Value,
        name: String,
    },
    AskYesNo {
        prompt: Value,
        name: String,
    },
    AskChoice {
        prompt: Value,
        list: String,
        name: String,
    },
    ShowMessage(Value),
    WhenAt(Value),
    WhenSignal(SignalName),
    SendSignal {
        name: SignalName,
        wait: bool,
    },
    WhenWebhook {
        path: Value,
        variable: String,
    },
    DetectObjectsInWindow {
        pattern: Value,
        by: String,
        detector: Value,
        label: Value,
        confidence: Value,
        sort: String,
        results_list: String,
        count_variable: String,
        #[serde(default)]
        region: String,
    },
    WaitForDetection {
        pattern: Value,
        by: String,
        detector: Value,
        label: Value,
        confidence: Value,
        appear: bool,
        timeout: Value,
        #[serde(default)]
        region: String,
    },
    AimStep {
        pattern: Value,
        by: String,
        target_x: Value,
        target_y: Value,
        gain: Value,
        max_step: Value,
        deadzone: Value,
    },
    AimAtDetection {
        results_list: String,
        index: Value,
        gain: Value,
        max_step: Value,
        deadzone: Value,
        timeout: Value,
        settle_ms: Value,
        status_variable: String,
    },
}

fn empty_text() -> Value {
    Value::text("")
}

fn match_by_title() -> String {
    "Title".into()
}

#[derive(Deserialize)]
#[serde(untagged)]
enum WaitDe {
    /// Oldest save shape: a bare duration, no randomness field existed yet.
    LegacyNumber(u64),
    /// Pre-`Op::Random` save shape: `[duration, randomness]`, migrated into
    /// `Op::Random` below so old macros keep the same spread of wait times.
    LegacyWithRandomness(Value, Value),
    /// Current shape: a single `Value` (a plain duration, or a duration
    /// wrapped in any operator including `Op::Random`).
    Current(Value),
}

/// Folds a legacy `[duration, randomness]` `Wait` into today's single-`Value`
/// shape: `duration` alone if randomness was zero, otherwise `duration`
/// wrapped in `Op::Random` spanning `[duration - randomness, duration + randomness]`.
fn migrate_wait_duration(duration: Value, randomness: Value) -> Value {
    if randomness == Value::number(0.0) {
        return duration;
    }
    let zero = || Box::new(Value::number(0.0));
    Value::Op {
        op: Op::Random,
        args: vec![
            Value::Op {
                op: Op::Sub,
                args: vec![duration.clone(), randomness.clone()],
                saved: zero(),
            },
            Value::Op {
                op: Op::Add,
                args: vec![duration, randomness],
                saved: zero(),
            },
        ],
        saved: zero(),
    }
}

impl From<InstructionKindDe> for InstructionKind {
    fn from(de: InstructionKindDe) -> Self {
        match de {
            InstructionKindDe::Token(t) => InstructionKind::Token(t),
            InstructionKindDe::Wait(WaitDe::LegacyNumber(d)) => {
                InstructionKind::Wait(Value::number(d as f64))
            }
            InstructionKindDe::Wait(WaitDe::LegacyWithRandomness(d, r)) => {
                InstructionKind::Wait(migrate_wait_duration(d, r))
            }
            InstructionKindDe::Wait(WaitDe::Current(d)) => InstructionKind::Wait(d),
            InstructionKindDe::Command(s) => InstructionKind::Command(s),
            InstructionKindDe::Comment(s) => InstructionKind::Comment(s),
            InstructionKindDe::WhenRan => InstructionKind::WhenRan,
            InstructionKindDe::WhenBatteryDischargedTo(v) => {
                InstructionKind::WhenBatteryDischargedTo(v)
            }
            InstructionKindDe::WhenBatteryChargedTo(v) => InstructionKind::WhenBatteryChargedTo(v),
            InstructionKindDe::WhenTime(WhenTimeDe::Current(s, t)) => InstructionKind::WhenTime(s, t),
            InstructionKindDe::WhenTime(WhenTimeDe::Legacy(old)) => {
                InstructionKind::WhenTime(old.schedule, Value::text(format!("{:02}:{:02}", old.hour, old.minute)))
            }
            InstructionKindDe::WhenPowerPluggedIn => InstructionKind::WhenPowerPluggedIn,
            InstructionKindDe::WhenPowerUnplugged => InstructionKind::WhenPowerUnplugged,
            InstructionKindDe::WhenClipboardChanged => InstructionKind::WhenClipboardChanged,
            InstructionKindDe::WhenKey(key, direction) => InstructionKind::WhenKey(key, direction),
            InstructionKindDe::OpenApp {
                command,
                name,
                icon,
            } => InstructionKind::OpenApp {
                command,
                name,
                icon,
            },
            InstructionKindDe::CloseApp {
                command,
                name,
                icon,
            } => InstructionKind::CloseApp {
                command,
                name,
                icon,
            },
            InstructionKindDe::SetVariable(n, v) => InstructionKind::SetVariable(n, v),
            InstructionKindDe::ChangeVariable(n, v) => InstructionKind::ChangeVariable(n, v),
            InstructionKindDe::SetClipboard(v) => InstructionKind::SetClipboard(v),
            InstructionKindDe::AddToList { value, name } => {
                InstructionKind::AddToList { value, name }
            }
            InstructionKindDe::DeleteOfList { index, name } => {
                InstructionKind::DeleteOfList { index, name }
            }
            InstructionKindDe::DeleteAllOfList { name } => {
                InstructionKind::DeleteAllOfList { name }
            }
            InstructionKindDe::ShiftList { name, amount } => {
                InstructionKind::ShiftList { name, amount }
            }
            InstructionKindDe::InsertIntoList { value, index, name } => {
                InstructionKind::InsertIntoList { value, index, name }
            }
            InstructionKindDe::ReplaceItemOfList { index, name, value } => {
                InstructionKind::ReplaceItemOfList { index, name, value }
            }
            InstructionKindDe::ReverseList { name } => InstructionKind::ReverseList { name },
            InstructionKindDe::BlockHeader(id) => InstructionKind::BlockHeader(id),
            InstructionKindDe::CallBlock {
                block_id,
                args,
                branches,
            } => InstructionKind::CallBlock {
                block_id,
                args,
                branches,
            },
            InstructionKindDe::RunBranch(name) => InstructionKind::RunBranch(name),
            InstructionKindDe::Return(v) => InstructionKind::Return(v),
            InstructionKindDe::If { condition, body } => InstructionKind::If { condition, body },
            InstructionKindDe::IfElse {
                condition,
                then_body,
                else_body,
            } => InstructionKind::IfElse {
                condition,
                then_body,
                else_body,
            },
            InstructionKindDe::Repeat { count, body } => InstructionKind::Repeat { count, body },
            InstructionKindDe::Forever { body } => InstructionKind::Forever { body },
            InstructionKindDe::While { condition, body } => {
                InstructionKind::While { condition, body }
            }
            InstructionKindDe::EscapeLoop => InstructionKind::EscapeLoop,
            InstructionKindDe::ContinueLoop => InstructionKind::ContinueLoop,
            InstructionKindDe::WriteFile { path, text, append } => {
                InstructionKind::WriteFile { path, text, append }
            }
            InstructionKindDe::HttpRequest {
                method,
                url,
                body,
                headers,
                variable,
            } => InstructionKind::HttpRequest {
                method,
                url,
                body,
                headers,
                variable,
            },
            InstructionKindDe::Notify { title, message } => InstructionKind::Notify { title, message },
            InstructionKindDe::Log(v) => InstructionKind::Log(v),
            InstructionKindDe::TryCatch { body, catch_body } => {
                InstructionKind::TryCatch { body, catch_body }
            }
            InstructionKindDe::ThrowError(v) => InstructionKind::ThrowError(v),
            InstructionKindDe::StopMacro => InstructionKind::StopMacro,
            InstructionKindDe::WhenWindow { pattern, focused } => {
                InstructionKind::WhenWindow { pattern, focused }
            }
            InstructionKindDe::WhenFileChanged(v) => InstructionKind::WhenFileChanged(v),
            InstructionKindDe::Retry { count, delay, body } => InstructionKind::Retry { count, delay, body },
            InstructionKindDe::WindowAction { action, pattern, by } => {
                InstructionKind::WindowAction { action, pattern, by }
            }
            InstructionKindDe::MoveWindow { pattern, by, resize, x, y } => {
                InstructionKind::MoveWindow { pattern, by, resize, x, y }
            }
            InstructionKindDe::RunMacro { target, wait } => InstructionKind::RunMacro { target, wait },
            InstructionKindDe::StopMacroNamed(v) => InstructionKind::StopMacroNamed(v),
            InstructionKindDe::SetGlobal { name, value } => InstructionKind::SetGlobal { name, value },
            InstructionKindDe::ChangeGlobal { name, by } => InstructionKind::ChangeGlobal { name, by },
            InstructionKindDe::WaitUntil { condition, timeout } => InstructionKind::WaitUntil { condition, timeout },
            InstructionKindDe::FindImage { image, tolerance, x_variable, y_variable } => {
                InstructionKind::FindImage { image, tolerance, x_variable, y_variable }
            }
            InstructionKindDe::ClickImage { image, tolerance, button } => InstructionKind::ClickImage { image, tolerance, button },
            InstructionKindDe::WaitForImage { image, tolerance, appear, timeout } => {
                InstructionKind::WaitForImage { image, tolerance, appear, timeout }
            }
            InstructionKindDe::WhenTyped { text, erase } => InstructionKind::WhenTyped { text, erase },
            InstructionKindDe::PasteText(v) => InstructionKind::PasteText(v),
            InstructionKindDe::WhenApp { pattern, started } => InstructionKind::WhenApp { pattern, started },
            InstructionKindDe::WhenIdle(v) => InstructionKind::WhenIdle(v),
            InstructionKindDe::WhenScreenLock { locked } => InstructionKind::WhenScreenLock { locked },
            InstructionKindDe::WithLock { name, body } => InstructionKind::WithLock { name, body },
            InstructionKindDe::WhenCron(v) => InstructionKind::WhenCron(v),
            InstructionKindDe::SplitIntoList { text, separator, name } => InstructionKind::SplitIntoList { text, separator, name },
            InstructionKindDe::WhenNetwork { connected } => InstructionKind::WhenNetwork { connected },
            InstructionKindDe::WhenUsb { pattern, connected } => InstructionKind::WhenUsb { pattern, connected },
            InstructionKindDe::WhenMouseButton { button, pressed, modifiers } => {
                InstructionKind::WhenMouseButton { button, pressed, modifiers }
            }
            InstructionKindDe::AskText { prompt, default, name } => InstructionKind::AskText { prompt, default, name },
            InstructionKindDe::AskYesNo { prompt, name } => InstructionKind::AskYesNo { prompt, name },
            InstructionKindDe::AskChoice { prompt, list, name } => InstructionKind::AskChoice { prompt, list, name },
            InstructionKindDe::ShowMessage(v) => InstructionKind::ShowMessage(v),
            InstructionKindDe::WhenAt(v) => InstructionKind::WhenAt(v),
            InstructionKindDe::WhenSignal(v) => InstructionKind::WhenSignal(v),
            InstructionKindDe::SendSignal { name, wait } => InstructionKind::SendSignal { name, wait },
            InstructionKindDe::WhenWebhook { path, variable } => InstructionKind::WhenWebhook { path, variable },
            InstructionKindDe::DetectObjectsInWindow { pattern, by, detector, label, confidence, sort, results_list, count_variable, region } => {
                InstructionKind::DetectObjectsInWindow { pattern, by, detector, label, confidence, sort, results_list, count_variable, region }
            }
            InstructionKindDe::WaitForDetection { pattern, by, detector, label, confidence, appear, timeout, region } => {
                InstructionKind::WaitForDetection { pattern, by, detector, label, confidence, appear, timeout, region }
            }
            InstructionKindDe::AimStep { pattern, by, target_x, target_y, gain, max_step, deadzone } => {
                InstructionKind::AimStep { pattern, by, target_x, target_y, gain, max_step, deadzone }
            }
            InstructionKindDe::AimAtDetection { results_list, index, gain, max_step, deadzone, timeout, settle_ms, status_variable } => {
                InstructionKind::AimAtDetection { results_list, index, gain, max_step, deadzone, timeout, settle_ms, status_variable }
            }
        }
    }
}

/// One saved macro: a [`MacroGraph`] canvas plus what only Blockwork cares
/// about - its name, playback speed and recording target. `graph` is
/// flattened on the wire and [`Deref`]ed, so both shapes are unchanged.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(from = "MacroDe")]
pub struct Macro {
    pub id: String,
    pub name: String,
    pub description: String,
    #[serde(flatten)]
    pub graph: MacroGraph,
    #[serde(default)]
    pub signals: Vec<String>,
    /// Strand explicitly chosen to receive freshly-recorded input; `None`
    /// falls back to the "first When Ran strand, else first strand" rule.
    /// Kept out of undo/redo (it's a preference, not an instruction edit).
    #[serde(default)]
    pub recording_target: Option<String>,
    /// Playback speed: every `Wait` duration is divided by this at runtime.
    /// 1.0 is normal, 2.0 is twice as fast. Clamped to [`SPEED_MULTIPLIER_RANGE`].
    #[serde(default = "default_speed_multiplier")]
    pub speed_multiplier: f64,
    /// Settings edited from the "Macro Settings" popup - see [`MacroSettings`].
    #[serde(default)]
    pub settings: MacroSettings,
}

impl Deref for Macro {
    type Target = MacroGraph;

    fn deref(&self) -> &MacroGraph {
        &self.graph
    }
}

impl DerefMut for Macro {
    fn deref_mut(&mut self) -> &mut MacroGraph {
        &mut self.graph
    }
}

/// Valid range for both the per-macro and global speed multipliers, enforced
/// wherever either is set from user input.
pub const SPEED_MULTIPLIER_RANGE: std::ops::RangeInclusive<f64> = 0.1..=10.0;

/// Per-macro settings edited from the "Macro Settings" popup next to the
/// macro dropdown - not part of the macro's own behavior, but affecting how
/// the app treats it. Persisted and exported/imported with the macro like
/// everything else in `Macro`, so a new field here needs no separate wiring
/// to survive a save/export round-trip.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct MacroSettings {
    /// When `true`, this macro's `WhenBattery*`/`WhenTime`/`WhenPower*`
    /// strands are watched by the background watchers (`battery_watch`/
    /// `time_watch` in the desktop app) even while a different macro is
    /// selected. By default only the currently selected macro's event
    /// strands are live.
    #[serde(default)]
    pub always_listen: bool,
    /// Stops a run (or a trigger-fired strand) after this many seconds;
    /// 0 means no limit.
    #[serde(default)]
    pub max_run_secs: u32,
    /// Stops any one loop after this many iterations; 0 means no limit.
    #[serde(default)]
    pub max_loop_iterations: u32,
    /// Triggers and hotkeys only start the macro while a window whose
    /// title or app contains this is focused; empty means any window.
    #[serde(default)]
    pub only_in_window: String,
    #[serde(default)]
    pub power: conditions::PowerCondition,
    /// Only from/until these minutes after midnight (may wrap past midnight).
    #[serde(default)]
    pub active_hours: Option<(u16, u16)>,
    /// Only on these weekdays, bit 0 = Sunday; 0 means every day.
    #[serde(default)]
    pub active_days: u8,
    /// Library folder; empty means the top level.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub folder: String,
    /// Runs schedules and "once at" triggers it missed while Blockwork was
    /// closed or the computer slept, once, when it gets the chance.
    #[serde(default)]
    pub catch_up_missed: bool,    /// Block packs this macro's blocks came from.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub block_packs: Vec<crate::block_packs::PackRecord>,
}

/// Deserialization shape supporting both the current multi-strand format and
/// the legacy flat `code: Vec<Instruction>` format from older saves; legacy
/// macros become a single root strand. The canvas collections are listed out
/// rather than flattened, so the untagged split stays a field-by-field match.
#[derive(Deserialize)]
#[serde(untagged)]
enum MacroDe {
    Current {
        #[serde(default = "default_macro_id")]
        id: String,
        name: String,
        description: String,
        strands: Vec<Strand>,
        #[serde(default)]
        signals: Vec<String>,
        #[serde(default)]
        recording_target: Option<String>,
        #[serde(default = "default_speed_multiplier")]
        speed_multiplier: f64,
        #[serde(default)]
        floating_values: Vec<FloatingValue>,
        #[serde(default)]
        comments: Vec<Comment>,
        #[serde(default)]
        variables: Vec<VariableDef>,
        #[serde(default)]
        block_defs: Vec<BlockDef>,
        #[serde(default)]
        settings: MacroSettings,
        #[serde(default)]
        lists: Vec<ListDef>,
        // Carried for round-trip only, Blockwork has no dict blocks or UI.
        #[serde(default)]
        dicts: Vec<DictDef>,
    },
    Legacy {
        #[serde(default = "default_macro_id")]
        id: String,
        name: String,
        description: String,
        code: Vec<Instruction>,
    },
}

impl From<MacroDe> for Macro {
    fn from(de: MacroDe) -> Self {
        let mut mac = match de {
            MacroDe::Current {
                id,
                name,
                description,
                mut strands,
                signals,
                recording_target,
                speed_multiplier,
                floating_values,
                comments,
                variables,
                block_defs,
                settings,
                lists,
                dicts,
            } => {
                // Pre-"When Ran" saves have a strand id=="root" that was the
                // implicit entry point; give it a real WhenRan on upgrade.
                if let Some(legacy) = strands.iter_mut().find(|s| s.id == LEGACY_ROOT_STRAND_ID)
                    && !legacy.starts_with_header()
                {
                    legacy
                        .instructions
                        .insert(0, Instruction::new(InstructionKind::WhenRan));
                }
                Self {
                    id,
                    name,
                    description,
                    graph: MacroGraph {
                        strands,
                        floating_values,
                        comments,
                        variables,
                        lists,
                        dicts,
                        block_defs,
                    },
                    signals,
                    recording_target,
                    speed_multiplier,
                    settings,
                }
            }
            MacroDe::Legacy {
                id,
                name,
                description,
                mut code,
            } => {
                code.insert(0, Instruction::new(InstructionKind::WhenRan));
                Self {
                    id,
                    name,
                    description,
                    graph: MacroGraph {
                        strands: vec![Strand::with_instructions(0, 0, code)],
                        ..MacroGraph::new()
                    },
                    signals: Vec::new(),
                    recording_target: None,
                    speed_multiplier: default_speed_multiplier(),
                    settings: MacroSettings::default(),
                }
            }
        };
        // Repairs boolean slots poisoned by the historical `Value::Bool`-less
        // bug (see `Value::migrate_bool_slots`) - a save from before that fix
        // may have a raw number leaf sitting where a blank hexagon belongs.
        mac.graph.migrate_bool_slots();
        // A macro file may have been created before block colors existed (in
        // which case serde supplied blue), or hand-edited/imported with an
        // invalid color. Keep all persisted values safe to place in a CSS
        // custom property before the frontend ever sees them.
        mac.graph.normalize_block_colors();
        mac.migrate_legacy_comments();
        mac.collect_named_signals();
        mac
    }
}

impl Macro {
    pub fn new(name: String, description: String, mut code: Vec<Instruction>) -> Self {
        code.insert(0, Instruction::new(InstructionKind::WhenRan));
        Self {
            id: default_macro_id(),
            name,
            description,
            graph: MacroGraph {
                strands: vec![Strand::with_instructions(0, 0, code)],
                ..MacroGraph::new()
            },
            signals: Vec::new(),
            recording_target: None,
            speed_multiplier: default_speed_multiplier(),
            settings: MacroSettings::default(),
        }
    }

    /// Pack implementation strands stay available to playback, outside the editor.
    pub fn editor_strands(&self) -> impl Iterator<Item = &Strand> {
        self.strands.iter().filter(|strand| !crate::block_packs::is_pack_strand(self, strand))
    }

    pub fn clear_editor(&mut self) {
        let ids: Vec<_> = self.editor_strands().map(|strand| strand.id.clone()).collect();
        self.graph.strands.retain(|strand| !ids.contains(&strand.id));
        self.graph.prune_orphaned_comments();
    }

    pub fn next_editor_position(&self) -> (i32, i32) {
        (self.editor_strands().map(|strand| strand.x).max().unwrap_or(0) + 260, 0)
    }

    /// Keep named references registered when importing or adding blocks.
    pub fn collect_named_signals(&mut self) {
        let mut names = Vec::new();
        self.graph.walk_instructions(&mut |ins| {
            if let InstructionKind::WhenSignal(name) | InstructionKind::SendSignal { name, .. } =
                &ins.kind
                && !name.expression
                && let Value::Text { value } = &name.value
                && !value.trim().is_empty()
            {
                names.push(value.trim().to_string());
            }
        });
        let mut unique: Vec<String> = Vec::new();
        for name in self.signals.iter().chain(names.iter()) {
            let name = name.trim();
            if !name.is_empty() && !unique.iter().any(|s| s.eq_ignore_ascii_case(name)) {
                unique.push(name.to_string());
            }
        }
        self.graph.walk_instructions_mut(&mut |ins| {
            if let InstructionKind::WhenSignal(name) | InstructionKind::SendSignal { name, .. } =
                &mut ins.kind
                && !name.expression
                && let Value::Text { value } = &mut name.value
                && let Some(canonical) =
                    unique.iter().find(|s| s.eq_ignore_ascii_case(value.trim()))
            {
                *value = canonical.clone();
            }
        });
        self.signals = unique;
    }

    pub fn validate_signal_name(
        &self,
        name: &str,
        replacing: Option<&str>,
    ) -> Result<String, String> {
        let name = name.trim();
        if name.is_empty() {
            return Err("Signal name can't be empty".into());
        }
        if self
            .signals
            .iter()
            .any(|s| Some(s.as_str()) != replacing && s.eq_ignore_ascii_case(name))
        {
            return Err(format!("A signal named \"{name}\" already exists"));
        }
        Ok(name.to_string())
    }

    pub fn rename_signal(&mut self, old: &str, new: &str) -> Result<(), String> {
        let new = self.validate_signal_name(new, Some(old))?;
        let index = self
            .signals
            .iter()
            .position(|s| s == old)
            .ok_or("Signal not found")?;
        self.signals[index] = new.clone();
        self.graph.walk_instructions_mut(&mut |ins| {
            if let InstructionKind::WhenSignal(name) | InstructionKind::SendSignal { name, .. } =
                &mut ins.kind
                && !name.expression
                && let Value::Text { value } = &mut name.value
                && value.trim().eq_ignore_ascii_case(old)
            {
                *value = new.clone();
            }
        });
        Ok(())
    }

    /// Whether any instruction, including nested bodies, moves to an
    /// absolute screen position.
    pub fn has_absolute_mouse_move(&self) -> bool {
        let mut found = false;
        self.graph.walk_instructions(&mut |ins| {
            found |= matches!(
                &ins.kind,
                InstructionKind::Token(InputToken::MoveMouse(
                    _,
                    _,
                    crate::input::types::Coordinate::Abs
                ))
            );
        });
        found
    }

    pub fn ensure_id(&mut self) {
        if self.id.trim().is_empty() {
            self.id = default_macro_id();
        }
    }

    /// Renames a branch callback (`RunBranch(old)` blocks) inside the named
    /// custom block's own body, keeping it working after the prototype's
    /// branch piece is renamed. No-op if the block has no such body strand.
    pub fn rename_block_branch_body(&mut self, block_id: &str, old: &str, new: &str) {
        let Some(strand) = self.graph.strands.iter_mut().find(|s| {
            matches!(
                s.instructions.first().map(|i| &i.kind),
                Some(InstructionKind::BlockHeader(id)) if id == block_id
            )
        }) else {
            return;
        };
        for ins in &mut strand.instructions {
            ins.walk_mut(&mut |child| {
                if let InstructionKind::RunBranch(name) = &mut child.kind
                    && name == old
                {
                    *name = new.to_string();
                }
            });
        }
    }

    /// Defines a new custom block, with Blockwork's own header instruction
    /// at the top of its body strand.
    pub fn create_block(
        &mut self,
        pieces: Vec<BlockPiece>,
        shape: BlockShape,
        color: String,
        x: i32,
        y: i32,
    ) -> String {
        self.graph.create_block(pieces, shape, color, x, y, |id| {
            InstructionKind::BlockHeader(id.to_string())
        })
    }

    /// One-time upgrade for saves from before floating/attached comments
    /// existed: pulls every legacy inline `Comment` instruction out of the
    /// instruction stream and re-homes it as a freestanding `Comment` parked
    /// near its old strand. Idempotent - a save with no legacy `Comment`
    /// instructions left is a no-op.
    fn migrate_legacy_comments(&mut self) {
        fn extract(list: &mut Vec<Instruction>, out: &mut Vec<String>) {
            list.retain_mut(|ins| {
                for slot in 0..2u8 {
                    if let Some(body) = ins.body_mut(slot) {
                        extract(body, out);
                    }
                }
                if let InstructionKind::Comment(text) = &ins.kind {
                    out.push(text.clone());
                    false
                } else {
                    true
                }
            });
        }
        for strand in &mut self.graph.strands {
            let mut texts = Vec::new();
            extract(&mut strand.instructions, &mut texts);
            for (i, text) in texts.into_iter().enumerate() {
                self.graph.comments.push(Comment::new(
                    strand.x + 40,
                    strand.y + 40 + i as i32 * 30,
                    text,
                    None,
                ));
            }
        }
    }

    /// Strand that freshly-recorded input gets appended to: the explicit
    /// `recording_target` if it still exists, else the first "When Ran"
    /// strand, else the first strand (creating one if the macro is empty).
    pub fn recording_target_mut(&mut self) -> &mut Strand {
        if let Some(id) = self.recording_target_id() {
            let pos = self.graph.strands.iter().position(|strand| strand.id == id).unwrap();
            return &mut self.graph.strands[pos];
        }
        self.graph.strands.push(Strand::new(0, 0));
        self.graph.strands.last_mut().unwrap()
    }

    /// Resolves the recording target without entering a pack implementation.
    pub fn recording_target_id(&self) -> Option<String> {
        if let Some(id) = &self.recording_target
            && self.editor_strands().any(|strand| &strand.id == id)
        {
            return Some(id.clone());
        }
        self.editor_strands().find(|strand| strand.starts_with_header())
            .or_else(|| self.editor_strands().next()).map(|strand| strand.id.clone())
    }
}

impl std::hash::Hash for Macro {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
        self.name.hash(state);
        self.description.hash(state);
        self.graph.hash(state);
        self.signals.hash(state);
        self.recording_target.hash(state);
        self.speed_multiplier.to_bits().hash(state);
        self.settings.hash(state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::value::Evaluated;
    use crate::input::types::Coordinate;

    #[test]
    fn signal_names_migrate_and_unused_names_survive_round_trip() {
        let mut mac: Macro = serde_json::from_value(serde_json::json!({
            "name": "Old macro", "description": "", "signals": ["Unused"],
            "strands": [{"id": "s", "x": 0, "y": 0, "instructions": [
                {"id": "send", "kind": {"SendSignal": {"name": {"kind": "Text", "value": " Go "}, "wait": true}}},
                {"id": "receive", "kind": {"WhenSignal": {"kind": "Text", "value": "go"}}}
            ]}]
        })).unwrap();
        assert_eq!(mac.signals, ["Unused", "Go"]);
        mac.strands.clear();
        let restored: Macro = serde_json::from_str(&serde_json::to_string(&mac).unwrap()).unwrap();
        assert_eq!(restored.signals, ["Unused", "Go"]);
    }

    #[test]
    fn signal_rename_updates_nested_named_references_but_preserves_expressions() {
        let mut mac = Macro::new(
            "Signals".into(),
            "".into(),
            vec![
                Instruction::new(InstructionKind::WhenSignal(Value::text("go").into())),
                Instruction::new(InstructionKind::Repeat {
                    count: Value::number(1.0),
                    body: vec![
                        Instruction::new(InstructionKind::SendSignal {
                            name: Value::text(" GO ").into(),
                            wait: true,
                        }),
                        Instruction::new(InstructionKind::SendSignal {
                            name: SignalName::new(Value::text("go"), true),
                            wait: false,
                        }),
                    ],
                }),
            ],
        );
        mac.collect_named_signals();
        assert!(mac.validate_signal_name(" GO ", None).is_err());
        assert!(mac.validate_signal_name(" ", None).is_err());
        mac.rename_signal("go", "finished").unwrap();
        assert_eq!(mac.signals, ["finished"]);
        let mut names = Vec::new();
        mac.graph.walk_instructions(&mut |ins| {
            if let InstructionKind::WhenSignal(name) | InstructionKind::SendSignal { name, .. } =
                &ins.kind
            {
                names.push((name.value.clone(), name.expression));
            }
        });
        assert_eq!(
            names,
            vec![
                (Value::text("finished"), false),
                (Value::text("finished"), false),
                (Value::text("go"), true)
            ]
        );
        let restored: Macro = serde_json::from_str(&serde_json::to_string(&mac).unwrap()).unwrap();
        assert_eq!(restored.signals, ["finished"]);
        assert_eq!(restored.graph, mac.graph);
    }

    #[test]
    fn legacy_dynamic_signals_remain_expressions() {
        let value = Value::Op {
            op: Op::Join,
            args: vec![Value::text("task:"), Value::text("42")],
            saved: Box::new(Value::text("")),
        };
        let old = serde_json::json!({"WhenSignal": value});
        let instruction: InstructionKind = serde_json::from_value(old).unwrap();
        let InstructionKind::WhenSignal(name) = instruction else {
            panic!("expected signal receiver");
        };
        assert!(name.expression);
        assert_eq!(name.value, value);
    }

    #[test]
    fn block_def_migrates_legacy_returns_value_true_to_returns_value_shape() {
        let def: BlockDef =
            serde_json::from_str(r#"{"id":"b1","pieces":[],"returns_value":true}"#).unwrap();
        assert_eq!(def.shape, BlockShape::ReturnsValue);
    }

    #[test]
    fn block_def_migrates_legacy_returns_value_false_to_normal_shape() {
        let def: BlockDef =
            serde_json::from_str(r#"{"id":"b1","pieces":[],"returns_value":false}"#).unwrap();
        assert_eq!(def.shape, BlockShape::Normal);
    }

    #[test]
    fn block_def_reads_current_shape_field() {
        let def: BlockDef =
            serde_json::from_str(r#"{"id":"b1","pieces":[],"shape":"ReturnsBool"}"#).unwrap();
        assert_eq!(def.shape, BlockShape::ReturnsBool);
    }

    #[test]
    fn block_def_round_trips_shape_through_serialize() {
        let def = BlockDef {
            id: "b1".into(),
            pieces: vec![],
            shape: BlockShape::Ending,
            color: default_block_color(),
            icon: String::new(),
                    identifier: String::new(),
                    description: String::new(),
        };
        let json = serde_json::to_string(&def).unwrap();
        assert!(
            json.contains(r#""shape":"Ending""#),
            "expected serialized shape field, got: {json}"
        );
        let round_tripped: BlockDef = serde_json::from_str(&json).unwrap();
        assert_eq!(round_tripped.shape, BlockShape::Ending);
    }

    #[test]
    fn block_def_defaults_color_for_older_macro_files() {
        let def: BlockDef =
            serde_json::from_str(r#"{"id":"b1","pieces":[],"shape":"Normal"}"#).unwrap();
        assert_eq!(def.color, default_block_color());
    }

    #[test]
    fn block_def_color_is_normalized_when_loading_a_macro() {
        let mac: Macro = serde_json::from_str(r##"{"id":"m1","name":"Test","description":"","strands":[],"block_defs":[{"id":"b1","pieces":[],"shape":"Normal","color":"#beef00"}]}"##).unwrap();
        assert_eq!(mac.block_defs[0].color, "#BEEF00");
    }

    #[test]
    fn block_def_color_falls_back_when_loading_an_invalid_color() {
        let mac: Macro = serde_json::from_str(r#"{"id":"m1","name":"Test","description":"","strands":[],"block_defs":[{"id":"b1","pieces":[],"shape":"Normal","color":"not a color"}]}"#).unwrap();
        assert_eq!(mac.block_defs[0].color, default_block_color());
    }

    #[test]
    fn macro_lists_live_in_the_graph_but_stay_flat_on_the_wire() {
        // Lists moved to blockstitch's `BlockGraph`, reached via `Deref`;
        // the save shape keeps `lists` next to `strands` either way.
        let mac: Macro = serde_json::from_str(
            r#"{"id":"m1","name":"Test","description":"","strands":[],"lists":[{"name":"queue","items":[{"kind":"Number","value":1.0}]}]}"#,
        )
        .unwrap();
        assert_eq!(mac.graph.lists.len(), 1);
        assert_eq!(mac.graph.lists[0].name, "queue");
        assert_eq!(mac.lists[0].name, "queue");
        let json = serde_json::to_string(&mac).unwrap();
        assert!(json.contains(r#""lists":[{"name":"queue""#), "got: {json}");
        let back: Macro = serde_json::from_str(&json).unwrap();
        assert_eq!(back, mac);
    }

    #[test]
    fn new_macro_defaults_to_one_when_ran_strand() {
        let mac = Macro::new("Test".into(), "".into(), vec![]);
        assert_eq!(mac.strands.len(), 1);
        assert!(mac.strands[0].starts_with_header());
    }

    #[test]
    fn detects_absolute_mouse_moves_in_nested_bodies() {
        let mut mac = Macro::new("Test".into(), "".into(), vec![]);
        mac.strands[0]
            .instructions
            .push(Instruction::new(InstructionKind::If {
                condition: Value::Bool,
                body: vec![Instruction::new(InstructionKind::Token(
                    InputToken::MoveMouse(
                        Value::number(10.0),
                        Value::number(20.0),
                        Coordinate::Abs,
                    ),
                ))],
            }));

        assert!(mac.has_absolute_mouse_move());
    }

    #[test]
    fn legacy_flat_code_migrates_to_when_ran_strand() {
        let json = r#"{"name":"Old","description":"","code":[{"Comment":"hi"}]}"#;
        let mac: Macro = serde_json::from_str(json).unwrap();
        assert_eq!(mac.strands.len(), 1);
        // The legacy inline `Comment` instruction is pulled out into a
        // freestanding `Comment` note, not left in the instruction stream.
        assert_eq!(
            mac.strands[0].instructions,
            vec![Instruction::new(InstructionKind::WhenRan)]
        );
        assert_eq!(mac.comments.len(), 1);
        assert_eq!(mac.comments[0].text, "hi");
        assert_eq!(mac.comments[0].attached_to, None);
    }

    #[test]
    fn legacy_root_strand_gains_when_ran_on_load() {
        let json = r#"{"id":"m1","name":"Old","description":"","strands":[
            {"id":"root","x":0,"y":0,"instructions":[{"Comment":"hi"}]},
            {"id":"stray","x":10,"y":10,"instructions":[]}
        ]}"#;
        let mac: Macro = serde_json::from_str(json).unwrap();
        let root = mac.strand("root").unwrap();
        assert!(root.starts_with_header());
        assert_eq!(
            root.instructions,
            vec![Instruction::new(InstructionKind::WhenRan)]
        );
        assert_eq!(mac.comments.len(), 1);
        assert_eq!(mac.comments[0].text, "hi");
        // Untouched, non-entry strand should survive as-is.
        assert!(mac.strand("stray").unwrap().instructions.is_empty());
    }

    #[test]
    fn already_migrated_root_strand_is_not_double_prepended() {
        let json = r#"{"id":"m1","name":"New","description":"","strands":[
            {"id":"root","x":0,"y":0,"instructions":["WhenRan",{"Comment":"hi"}]}
        ]}"#;
        let mac: Macro = serde_json::from_str(json).unwrap();
        assert_eq!(
            mac.strand("root").unwrap().instructions,
            vec![Instruction::new(InstructionKind::WhenRan)]
        );
        assert_eq!(mac.comments.len(), 1);
        assert_eq!(mac.comments[0].text, "hi");
    }

    #[test]
    fn migrate_legacy_comments_reaches_into_nested_if_body() {
        let json = r#"{"id":"m1","name":"New","description":"","strands":[
            {"id":"root","x":0,"y":0,"instructions":["WhenRan",
                {"If":{"condition":{"kind":"Bool"},"body":[{"Comment":"nested"}]}}
            ]}
        ]}"#;
        let mac: Macro = serde_json::from_str(json).unwrap();
        let root = mac.strand("root").unwrap();
        match &root.instructions[1].kind {
            InstructionKind::If { body, .. } => assert!(
                body.is_empty(),
                "nested Comment should be extracted, not left in the If body"
            ),
            other => panic!("expected If, got {other:?}"),
        }
        assert_eq!(mac.comments.len(), 1);
        assert_eq!(mac.comments[0].text, "nested");
        assert_eq!(mac.comments[0].attached_to, None);
    }

    #[test]
    fn prune_orphaned_comments_drops_comment_attached_to_removed_instruction() {
        let wait = Instruction::new(InstructionKind::Wait(Value::number(1000.0)));
        let wait_id = wait.id.clone();
        let mut mac = Macro::new("Test".into(), "".into(), vec![wait]);
        mac.comments.push(Comment {
            id: "c1".into(),
            x: 0,
            y: 0,
            text: "hi".into(),
            collapsed: false,
            attached_to: Some(wait_id),
        });
        mac.comments.push(Comment {
            id: "c2".into(),
            x: 0,
            y: 0,
            text: "freestanding".into(),
            collapsed: false,
            attached_to: None,
        });

        // Remove the Wait instruction (index 1 - index 0 is the WhenRan header).
        mac.strands[0].instructions.remove(1);
        mac.prune_orphaned_comments();

        assert_eq!(mac.comments.len(), 1);
        assert_eq!(mac.comments[0].id, "c2");
    }

    #[test]
    fn prune_orphaned_comments_cascades_into_nested_wrap_body() {
        let inner = Instruction::new(InstructionKind::Wait(Value::number(1.0)));
        let inner_id = inner.id.clone();
        let if_ins = Instruction::new(InstructionKind::If {
            condition: Value::Bool,
            body: vec![inner],
        });
        let mut mac = Macro::new("Test".into(), "".into(), vec![if_ins]);
        mac.comments.push(Comment {
            id: "c1".into(),
            x: 0,
            y: 0,
            text: "nested".into(),
            collapsed: false,
            attached_to: Some(inner_id),
        });

        // Deleting the whole If block (index 1) takes its nested body with it.
        mac.strands[0].instructions.remove(1);
        mac.prune_orphaned_comments();

        assert!(mac.comments.is_empty());
    }

    #[test]
    fn prune_orphaned_comments_keeps_comment_attached_to_surviving_instruction() {
        let wait = Instruction::new(InstructionKind::Wait(Value::number(1000.0)));
        let wait_id = wait.id.clone();
        let mut mac = Macro::new("Test".into(), "".into(), vec![wait]);
        mac.comments.push(Comment {
            id: "c1".into(),
            x: 0,
            y: 0,
            text: "hi".into(),
            collapsed: false,
            attached_to: Some(wait_id),
        });

        mac.prune_orphaned_comments();

        assert_eq!(mac.comments.len(), 1);
    }

    #[test]
    fn migrate_bool_slots_repairs_poisoned_if_condition_on_load() {
        // Pre-`Value::Bool` save: dragging the default boolean block out of
        // an `If`'s condition once left a bare `Number` behind.
        let json = r#"{"id":"m1","name":"Old","description":"","strands":[
            {"id":"root","x":0,"y":0,"instructions":["WhenRan",
                {"If":{"condition":{"kind":"Number","value":0.0},"body":[]}}
            ]}
        ]}"#;
        let mac: Macro = serde_json::from_str(json).unwrap();
        match &mac.strand("root").unwrap().instructions[1].kind {
            InstructionKind::If { condition, .. } => assert_eq!(condition, &Value::Bool),
            other => panic!("expected If, got {other:?}"),
        }
    }

    #[test]
    fn migrate_bool_slots_repairs_poisoned_operand_nested_inside_condition() {
        // The poisoned `Number` can be arbitrarily deep - here inside an
        // `And` that itself is the `If`'s condition. Its sibling (a real
        // comparison) must survive untouched.
        let json = r#"{"id":"m1","name":"Old","description":"","strands":[
            {"id":"root","x":0,"y":0,"instructions":["WhenRan",
                {"If":{"condition":{
                    "kind":"Op","op":"And",
                    "args":[
                        {"kind":"Number","value":0.0},
                        {"kind":"Op","op":"Eq","args":[{"kind":"Number","value":1.0},{"kind":"Number","value":1.0}],"saved":{"kind":"Number","value":0.0}}
                    ],
                    "saved":{"kind":"Number","value":0.0}
                },"body":[]}}
            ]}
        ]}"#;
        let mac: Macro = serde_json::from_str(json).unwrap();
        match &mac.strand("root").unwrap().instructions[1].kind {
            InstructionKind::If {
                condition: Value::Op {
                    op: Op::And, args, ..
                },
                ..
            } => {
                assert_eq!(args[0], Value::Bool);
                assert_eq!(
                    args[1],
                    Value::Op {
                        op: Op::Eq,
                        args: vec![Value::number(1.0), Value::number(1.0)],
                        saved: Box::new(Value::Bool)
                    }
                );
            }
            other => panic!("expected If(And(..)), got {other:?}"),
        }
    }

    #[test]
    fn migrate_bool_slots_repairs_custom_block_boolean_arguments() {
        // The Boolean type of a custom-block argument lives in its
        // definition, not at the call site. A prior drag-out bug saved a zero
        // here, so loading must recover the blank hexagon from that type.
        let json = r#"{"id":"m1","name":"Old","description":"","strands":[
            {"id":"root","x":0,"y":0,"instructions":[{"CallBlock":{
                "block_id":"b1","args":[{"kind":"Number","value":0.0}]
            }}]}
        ],"block_defs":[{"id":"b1","pieces":[
            {"kind":"Input","id":"i1","name":"flag","value_type":"Bool"}
        ],"shape":"Normal"}]}"#;
        let mac: Macro = serde_json::from_str(json).unwrap();
        let args = mac.strands[0]
            .instructions
            .iter()
            .find_map(|instruction| match &instruction.kind {
                InstructionKind::CallBlock { args, .. } => Some(args),
                _ => None,
            })
            .expect("expected CallBlock");
        assert_eq!(args, &vec![Value::Bool]);
    }

    #[test]
    fn migrate_bool_slots_leaves_legitimately_numeric_fields_alone() {
        // A `Wait` duration is never boolean-typed - a `Number` there is
        // always legitimate and must not be touched.
        let json = r#"{"id":"m1","name":"Old","description":"","strands":[
            {"id":"root","x":0,"y":0,"instructions":["WhenRan",{"Wait":{"kind":"Number","value":0.0}}]}
        ]}"#;
        let mac: Macro = serde_json::from_str(json).unwrap();
        assert_eq!(
            mac.strand("root").unwrap().instructions[1],
            Instruction::new(InstructionKind::Wait(Value::number(0.0)))
        );
    }

    #[test]
    fn legacy_wait_with_randomness_migrates_to_random_op() {
        let json = r#"{"Wait":[1000.0,50.0]}"#;
        let ins: Instruction = serde_json::from_str(json).unwrap();
        assert_eq!(
            ins,
            Instruction::new(InstructionKind::Wait(Value::Op {
                op: Op::Random,
                args: vec![
                    Value::Op {
                        op: Op::Sub,
                        args: vec![Value::number(1000.0), Value::number(50.0)],
                        saved: Box::new(Value::number(0.0)),
                    },
                    Value::Op {
                        op: Op::Add,
                        args: vec![Value::number(1000.0), Value::number(50.0)],
                        saved: Box::new(Value::number(0.0)),
                    },
                ],
                saved: Box::new(Value::number(0.0)),
            }))
        );
    }

    #[test]
    fn legacy_wait_with_zero_randomness_migrates_to_plain_duration() {
        let json = r#"{"Wait":[1000.0,0.0]}"#;
        let ins: Instruction = serde_json::from_str(json).unwrap();
        assert_eq!(
            ins,
            Instruction::new(InstructionKind::Wait(Value::number(1000.0)))
        );
    }

    #[test]
    fn legacy_when_time_moves_its_clock_into_a_time_value() {
        use crate::input::schedule::Weekday;
        let ins: Instruction = serde_json::from_str(r#"{"WhenTime":{"kind":"Weekly","weekday":"Monday","hour":7,"minute":5}}"#).unwrap();
        let expected = InstructionKind::WhenTime(TimeSchedule::Weekly { weekday: Weekday::Monday }, Value::text("07:05"));
        assert_eq!(ins, Instruction::new(expected.clone()));
        // And the new shape reads back as itself.
        let round: Instruction = serde_json::from_str(&serde_json::to_string(&ins).unwrap()).unwrap();
        assert_eq!(round, Instruction::new(expected));
    }

    #[test]
    fn legacy_single_arg_wait_migrates_to_value() {
        let json = r#"{"Wait":1000}"#;
        let ins: Instruction = serde_json::from_str(json).unwrap();
        assert_eq!(
            ins,
            Instruction::new(InstructionKind::Wait(Value::number(1000.0)))
        );
    }

    #[test]
    fn legacy_bare_number_move_mouse_fields_migrate_to_value() {
        let json = r#"{"Token":{"MoveMouse":[5,10,"Rel"]}}"#;
        let ins: Instruction = serde_json::from_str(json).unwrap();
        assert_eq!(
            ins,
            Instruction::new(InstructionKind::Token(InputToken::MoveMouse(
                Value::number(5.0),
                Value::number(10.0),
                Coordinate::Rel
            ))),
        );
    }

    #[test]
    fn rename_variable_renames_declaration_and_every_reference() {
        let mut mac = Macro::new(
            "Test".into(),
            "".into(),
            vec![
                Instruction::new(InstructionKind::SetVariable(
                    "x".to_string(),
                    Value::number(1.0),
                )),
                Instruction::new(InstructionKind::ChangeVariable(
                    "x".to_string(),
                    Value::Var {
                        name: "x".to_string(),
                    },
                )),
                Instruction::new(InstructionKind::Token(InputToken::Text(Value::Var {
                    name: "x".to_string(),
                }))),
            ],
        );
        mac.variables.push(VariableDef {
            name: "x".to_string(),
            value: Evaluated::Number(0.0),
        });
        mac.floating_values.push(FloatingValue {
            id: "f1".into(),
            x: 0,
            y: 0,
            value: Value::Var {
                name: "x".to_string(),
            },
            origin_block_id: None,
        });

        mac.rename_variable("x", "y").unwrap();

        assert_eq!(mac.variables[0].name, "y");
        let strand = &mac.strands[0];
        assert_eq!(
            strand.instructions[1],
            Instruction::new(InstructionKind::SetVariable(
                "y".to_string(),
                Value::number(1.0)
            ))
        );
        assert_eq!(
            strand.instructions[2],
            Instruction::new(InstructionKind::ChangeVariable(
                "y".to_string(),
                Value::Var {
                    name: "y".to_string()
                }
            ))
        );
        assert_eq!(
            strand.instructions[3],
            Instruction::new(InstructionKind::Token(InputToken::Text(Value::Var {
                name: "y".to_string()
            })))
        );
        assert_eq!(
            mac.floating_values[0].value,
            Value::Var {
                name: "y".to_string()
            }
        );
    }

    #[test]
    fn rename_variable_rejects_an_undeclared_name_and_changes_nothing() {
        let mut mac = Macro::new(
            "Test".into(),
            "".into(),
            vec![Instruction::new(InstructionKind::Token(InputToken::Text(
                Value::Var {
                    name: "x".to_string(),
                },
            )))],
        );
        assert!(mac.rename_variable("x", "y").is_err());
        assert_eq!(
            mac.strands[0].instructions[1],
            Instruction::new(InstructionKind::Token(InputToken::Text(Value::Var {
                name: "x".to_string()
            })))
        );
    }

    #[test]
    fn rename_var_reaches_into_if_body_and_condition() {
        let mut ins = Instruction::new(InstructionKind::If {
            condition: Value::Var {
                name: "x".to_string(),
            },
            body: vec![Instruction::new(InstructionKind::SetVariable(
                "x".to_string(),
                Value::Var {
                    name: "x".to_string(),
                },
            ))],
        });
        ins.rename_var("x", "y");
        match &ins.kind {
            InstructionKind::If { condition, body } => {
                assert_eq!(
                    *condition,
                    Value::Var {
                        name: "y".to_string()
                    }
                );
                assert_eq!(
                    body[0],
                    Instruction::new(InstructionKind::SetVariable(
                        "y".to_string(),
                        Value::Var {
                            name: "y".to_string()
                        }
                    ))
                );
            }
            _ => panic!("expected If"),
        }
    }

    #[test]
    fn rename_var_reaches_into_if_else_both_branches() {
        let mut ins = Instruction::new(InstructionKind::IfElse {
            condition: Value::Var {
                name: "x".to_string(),
            },
            then_body: vec![Instruction::new(InstructionKind::SetVariable(
                "x".to_string(),
                Value::number(1.0),
            ))],
            else_body: vec![Instruction::new(InstructionKind::SetVariable(
                "x".to_string(),
                Value::number(2.0),
            ))],
        });
        ins.rename_var("x", "y");
        match &ins.kind {
            InstructionKind::IfElse {
                then_body,
                else_body,
                ..
            } => {
                assert_eq!(
                    then_body[0],
                    Instruction::new(InstructionKind::SetVariable(
                        "y".to_string(),
                        Value::number(1.0)
                    ))
                );
                assert_eq!(
                    else_body[0],
                    Instruction::new(InstructionKind::SetVariable(
                        "y".to_string(),
                        Value::number(2.0)
                    ))
                );
            }
            _ => panic!("expected IfElse"),
        }
    }

    #[test]
    fn scrub_block_calls_reaches_into_nested_if_body() {
        let mut ins = Instruction::new(InstructionKind::If {
            condition: Value::number(1.0),
            body: vec![Instruction::new(InstructionKind::SetVariable(
                "x".to_string(),
                Value::Call {
                    block_id: "gone".to_string(),
                    args: vec![],
                    branches: vec![],
                    saved: Box::new(Value::number(0.0)),
                },
            ))],
        });
        ins.scrub_block_calls("gone");
        match &ins.kind {
            InstructionKind::If { body, .. } => {
                assert_eq!(
                    body[0],
                    Instruction::new(InstructionKind::SetVariable(
                        "x".to_string(),
                        Value::number(0.0)
                    ))
                );
            }
            _ => panic!("expected If"),
        }
    }

    #[test]
    fn body_mut_addresses_if_and_if_else_slots() {
        let mut if_ins = InstructionKind::If {
            condition: Value::number(1.0),
            body: vec![Instruction::new(InstructionKind::Comment("a".into()))],
        };
        assert_eq!(
            if_ins.body_mut(0),
            Some(&mut vec![Instruction::new(InstructionKind::Comment(
                "a".into()
            ))])
        );
        assert_eq!(if_ins.body_mut(1), None);

        let mut if_else = InstructionKind::IfElse {
            condition: Value::number(1.0),
            then_body: vec![Instruction::new(InstructionKind::Comment("then".into()))],
            else_body: vec![Instruction::new(InstructionKind::Comment("else".into()))],
        };
        assert_eq!(
            if_else.body_mut(0),
            Some(&mut vec![Instruction::new(InstructionKind::Comment(
                "then".into()
            ))])
        );
        assert_eq!(
            if_else.body_mut(1),
            Some(&mut vec![Instruction::new(InstructionKind::Comment(
                "else".into()
            ))])
        );
        assert_eq!(if_else.body_mut(2), None);
    }

    #[test]
    fn body_mut_addresses_loop_slots() {
        let mut repeat = InstructionKind::Repeat {
            count: Value::number(3.0),
            body: vec![Instruction::new(InstructionKind::Comment("a".into()))],
        };
        assert_eq!(
            repeat.body_mut(0),
            Some(&mut vec![Instruction::new(InstructionKind::Comment(
                "a".into()
            ))])
        );
        assert_eq!(repeat.body_mut(1), None);

        let mut forever = InstructionKind::Forever {
            body: vec![Instruction::new(InstructionKind::Comment("b".into()))],
        };
        assert_eq!(
            forever.body_mut(0),
            Some(&mut vec![Instruction::new(InstructionKind::Comment(
                "b".into()
            ))])
        );

        let mut while_ins = InstructionKind::While {
            condition: Value::Bool,
            body: vec![Instruction::new(InstructionKind::Comment("c".into()))],
        };
        assert_eq!(
            while_ins.body_mut(0),
            Some(&mut vec![Instruction::new(InstructionKind::Comment(
                "c".into()
            ))])
        );

        assert_eq!(InstructionKind::EscapeLoop.body_mut(0), None);
        assert_eq!(InstructionKind::ContinueLoop.body_mut(0), None);
    }
}

