//! Blockwork's block vocabulary - the instructions its canvas speaks - and
//! the `Macro` document wrapped around blockstitch's [`BlockGraph`].
//! [`InstructionKind`]'s [`BlockKind`] impl is the hinge between the two.

use crate::input::schedule::{LegacyTimeSchedule, TimeSchedule};
use crate::input::types::{Direction, InputToken, MacroButton};
use crate::input::value::{Op, Value};
use serde::{Deserialize, Serialize};
use std::ops::{Deref, DerefMut};
use uuid::Uuid;

#[cfg(feature = "system")]
pub mod backend;
#[cfg(feature = "system")]
pub mod calls;
pub mod conditions;
#[cfg(feature = "system")]
pub mod debug;
pub mod fields;
#[cfg(feature = "system")]
pub mod gdr_import;
#[cfg(feature = "system")]
pub mod host_clock;
#[cfg(feature = "system")]
pub mod locks;
#[cfg(feature = "system")]
pub mod loop_control;
pub mod migrate;
pub mod placement;
#[cfg(feature = "system")]
pub mod priority;
#[cfg(feature = "system")]
pub mod run_registry;
#[cfg(feature = "system")]
pub mod runner;
#[cfg(feature = "system")]
pub mod thread_pool;
#[cfg(feature = "system")]
pub mod trace;
pub mod trigger;

pub use blockstitch_core::graph::{
    BlockDef, BlockGraph, BlockKind, BlockPiece, BlockShape, Comment, DictDef, FloatingValue,
    InputValueType, ListDef, ListItem, RefKind, VariableDef, default_block_color,
    normalize_block_color,
};
pub use fields::FieldId;

/// One instruction on the canvas: Blockwork's [`InstructionKind`] plus the
/// stable id blockstitch tracks it by.
pub type Instruction = blockstitch_core::graph::Instruction<InstructionKind>;
/// One draggable stack of Blockwork instructions.
pub type Strand = blockstitch_core::graph::Strand<InstructionKind>;

/// What a typed-in value is stored as: a number when it reads as one, else text.
pub fn typed_value(text: &str) -> crate::input::value::Evaluated {
    use crate::input::value::Evaluated;
    match text.trim().parse::<f64>() {
        Ok(n) if n.is_finite() && !text.trim().is_empty() => Evaluated::Number(n),
        _ => Evaluated::Text(text.to_string()),
    }
}
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

mod block_kind;
mod document_rules;
pub use document_rules::{
    BlockDraft, EDIT_CONTEXT, UNDO_LIMIT, UndoSnapshot, checked_block_color, clamp_speed,
    clean_folder,
};
mod saved_shapes;

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

/// Extra launch settings for an `OpenApp` block, all empty by default.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(default)]
pub struct LaunchOptions {
    /// Added after the app's own command, read like a command line.
    pub args: String,
    /// Folder the app starts in; empty keeps Blockwork's. Unused on macOS.
    pub working_dir: String,
    /// Environment variables set for the app only.
    pub env: Vec<EnvVar>,
    /// macOS only: open another copy even if the app is already running.
    pub new_instance: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(default)]
pub struct EnvVar {
    pub name: String,
    pub value: String,
}

impl LaunchOptions {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

#[derive(Debug, Clone, PartialEq, Hash, Serialize, Deserialize)]
pub enum InstructionKind {
    Token(InputToken),
    #[serde(deserialize_with = "saved_shapes::wait")]
    Wait(Value),
    /// Starts a shell command and moves on without waiting for it. Files
    /// before format version 2 meant `CommandWait` by it (see `migrate`).
    Command(String),
    /// Runs a shell command and waits for it to finish.
    CommandWait(String),
    /// An old in-strand note. Loading moves it onto the canvas as a
    /// `graph::Comment`; nothing makes new ones.
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
    /// the desktop app, `blockwork-app`'s `watch::battery` module), which polls
    /// the battery, fires the strand's body (everything after this marker)
    /// the moment the condition holds, and won't fire it again until the
    /// battery recovers past the threshold and crosses it again.
    WhenBatteryDischargedTo(Value),
    /// Same as `WhenBatteryDischargedTo`, but fires when the battery charge
    /// rises to (or above) the given percentage instead.
    WhenBatteryChargedTo(Value),
    /// Header-only marker, same shape/semantics as `WhenBatteryDischargedTo`
    /// (excluded from Run/Loop, driven by a background watcher - `watch::time`
    /// in the desktop app) but for a recurring point in local time instead
    /// of a battery level. See `TimeSchedule` for the recurrence shapes; the
    /// value is the time of day, like `09:30` (see `parse_time_of_day`).
    #[serde(deserialize_with = "saved_shapes::when_time")]
    WhenTime(TimeSchedule, Value),
    /// Header-only marker, no payload (like `WhenRan`) - excluded from
    /// Run/Loop and driven by the same background watcher as
    /// `WhenBattery*To` (`watch::battery` in the desktop app), which fires
    /// this strand's body the moment the system starts receiving external
    /// power. See `crate::battery::is_plugged_in`.
    WhenPowerPluggedIn,
    /// Same as `WhenPowerPluggedIn`, but fires when external power is lost
    /// instead - never fires at all on a system with no battery/UPS, since
    /// `is_plugged_in` is always `true` there.
    WhenPowerUnplugged,
    /// Header-only marker, no payload (like `WhenPowerPluggedIn`) - excluded
    /// from Run/Loop and driven by a background watcher (`watch::clipboard`
    /// in the desktop app), which fires this strand's body whenever the
    /// clipboard's contents change. Edge-triggered like the other watcher
    /// headers, but with no threshold to recover past - any change fires it.
    WhenClipboardChanged,
    /// Header-only marker driven by the key watcher (`watch::key` in the
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
    /// `launch` is what the block's launch options panel adds on top.
    OpenApp {
        command: String,
        name: String,
        icon: Option<String>,
        #[serde(default, skip_serializing_if = "LaunchOptions::is_empty")]
        launch: LaunchOptions,
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
        #[serde(default = "match_by_title")]
        by: String,
    },
    /// Moves a matching window to (`x`, `y`), or with `resize` sizes it
    /// to `x` by `y`.
    MoveWindow {
        pattern: Value,
        #[serde(default = "match_by_title")]
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
    /// Pauses until true. `timeout` continues; `error_timeout` ends the strand.
    /// Missing or zero limits wait forever.
    WaitUntil {
        condition: Value,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        timeout: Option<Value>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        error_timeout: Option<Value>,
    },
    /// Looks for `image` (a PNG `data:` URI or file path) on screen, or in
    /// `region` of it, and puts the screen position of the middle of the
    /// match `pick` chooses (the best without one) in two variables, -1 if
    /// there's none. The number of matches goes in `count_variable`.
    /// `pick`, `region` and `count_variable` are the block's optional
    /// components: `None` while the user hasn't added them.
    FindImage {
        image: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tolerance: Option<Value>,
        x_variable: String,
        y_variable: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pick: Option<ImageMatch>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        region: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        count_variable: Option<String>,
    },
    /// Clicks the middle of the match of `image` that `pick` chooses; an
    /// error if there's none.
    ClickImage {
        image: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tolerance: Option<Value>,
        button: MacroButton,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pick: Option<ImageMatch>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        region: Option<String>,
    },
    /// Pauses until `image` appears (or with `appear` false, disappears),
    /// continuing after `timeout` ms or failing after `error_timeout` ms.
    /// Missing or zero limits wait forever; missing tolerance uses 10%.
    WaitForImage {
        image: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tolerance: Option<Value>,
        appear: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        timeout: Option<Value>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        region: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        error_timeout: Option<Value>,
    },
    /// Header for a strand fired when a window whose title or app contains
    /// `pattern` gains (`focused`) or loses focus. Driven by `watch::window`
    /// in the desktop app, like the other `When*` watcher headers.
    WhenWindow {
        pattern: Value,
        focused: bool,
    },
    /// Header for a strand fired when the file at the path is created,
    /// changed or deleted. Driven by `watch::file` in the desktop app.
    WhenFileChanged(Value),
    /// Header for a strand fired when the user types `text` (text
    /// expansion); with `erase` the typed text is backspaced away first.
    /// Driven by `watch::typed` in the desktop app.
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
        #[serde(default, skip_serializing_if = "Option::is_none")]
        modifiers: Option<Value>,
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
        #[serde(default)]
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
        #[serde(default)]
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
    /// Opens `url` in the browser's active tab, or with `new_tab` in a new
    /// one, and waits for it to load. The browser blocks go through the
    /// Blockwork extension; see [`crate::browser`].
    BrowserOpen {
        url: Value,
        new_tab: bool,
    },
    /// Clicks match `nth` (from 1; negative counts from the end) of
    /// `selector` in the active tab, waiting a few seconds for it to appear.
    /// `nth` is an optional component: `None` (the first match) until added.
    BrowserClick {
        selector: Value,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        nth: Option<Value>,
    },
    /// Types `text` into match `nth` of `selector`, replacing what was there.
    BrowserType {
        text: Value,
        selector: Value,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        nth: Option<Value>,
    },
    /// Pauses until `selector` matches something (or with `appear` false,
    /// nothing), failing after `timeout` ms (0 = never).
    BrowserWaitFor {
        selector: Value,
        appear: bool,
        timeout: Value,
    },
    /// Runs JavaScript in the active tab and puts its result in `variable`.
    BrowserScript {
        code: Value,
        variable: String,
    },
}

impl InstructionKind {
    /// Blocks edited as free text, whose keystrokes make one undo step.
    pub fn edits_as_text(&self) -> bool {
        matches!(
            self,
            InstructionKind::Command(_)
                | InstructionKind::CommandWait(_)
                | InstructionKind::Comment(_)
        )
    }
}

fn match_by_title() -> String {
    "Title".into()
}

pub(crate) fn first_match() -> Value {
    Value::number(1.0)
}

pub(crate) fn zero_value() -> Value {
    Value::number(0.0)
}

/// Which match an image block uses when the picture is on screen more
/// than once. A block without one takes the closest likeness, ties going to
/// the first in reading order.
#[derive(Debug, Clone, PartialEq, Hash, Serialize, Deserialize)]
pub struct ImageMatch {
    pub mode: ImagePick,
    #[serde(default = "first_match")]
    pub index: Value,
    #[serde(default = "zero_value")]
    pub near_x: Value,
    #[serde(default = "zero_value")]
    pub near_y: Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ImagePick {
    /// Match number `index` in reading order, counting from 1; negative
    /// counts from the end.
    Nth,
    /// The one whose middle is nearest the mouse pointer.
    NearMouse,
    /// The one whose middle is nearest the point (`near_x`, `near_y`).
    NearPoint,
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
#[derive(Debug, Clone, PartialEq, Hash, Serialize, Deserialize, Default)]
pub struct MacroSettings {
    /// When `true`, this macro's `WhenBattery*`/`WhenTime`/`WhenPower*`
    /// strands are watched by the background watchers (`watch::battery`/
    /// `watch::time` in the desktop app) even while a different macro is
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
    pub catch_up_missed: bool,
    /// Block packs this macro's blocks came from.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub block_packs: Vec<crate::block_packs::PackRecord>,
    /// Metadata for packs authored here, restored on the next export.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pack_exports: Vec<crate::block_packs::PackExport>,
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
        crate::block_packs::migrate_variables(&mut mac);
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
        self.strands
            .iter()
            .filter(|strand| !crate::block_packs::is_pack_strand(self, strand))
    }

    pub fn clear_editor(&mut self) {
        let ids: Vec<_> = self
            .editor_strands()
            .map(|strand| strand.id.clone())
            .collect();
        self.graph
            .strands
            .retain(|strand| !ids.contains(&strand.id));
        self.graph.prune_orphaned_comments();
    }

    pub fn next_editor_position(&self) -> (i32, i32) {
        (
            self.editor_strands()
                .map(|strand| strand.x)
                .max()
                .unwrap_or(0)
                + 260,
            0,
        )
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
            let pos = self
                .graph
                .strands
                .iter()
                .position(|strand| strand.id == id)
                .unwrap();
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
        self.editor_strands()
            .find(|strand| strand.starts_with_header())
            .or_else(|| self.editor_strands().next())
            .map(|strand| strand.id.clone())
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
mod tests;
