//! The block vocabulary: every kind of instruction a macro can hold, and
//! the small value types their fields use.

use super::Instruction;
use crate::input::gamepad::{GamepadAxis, GamepadButton, GamepadStick};
use crate::input::schedule::TimeSchedule;
use crate::input::touch::TouchGestureKind;
use crate::input::types::{Direction, InputToken, MacroButton};
use crate::input::ui::{UiAction, UiProperty};
use crate::input::value::Value;
use serde::{Deserialize, Serialize};

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

/// Which strands a Stop block ends in the current macro run.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StopScope {
    #[default]
    All,
    ThisStrand,
    OtherStrands,
}

// The trait implementations in saved_shapes accept the old StopMacro shape.
#[derive(Debug, Clone, PartialEq, Hash, Serialize, Deserialize)]
#[serde(remote = "Self")]
pub enum InstructionKind {
    Token(InputToken),
    #[serde(deserialize_with = "super::saved_shapes::wait")]
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
    /// Fired by the enabled OS startup entry once per login session.
    WhenSystemStarts,
    /// Virtual controller output, opt-in on a supported native backend.
    GamepadButton {
        button: GamepadButton,
        direction: Direction,
    },
    GamepadStick {
        stick: GamepadStick,
        x: Value,
        y: Value,
    },
    GamepadTrigger {
        left: bool,
        amount: Value,
    },
    ResetGamepad,
    /// A coalesced change to a freshly resolved accessible element property.
    WhenNativeUiChanged {
        selector: Value,
        property: UiProperty,
    },
    /// Physical controller triggers are opt-in and use stable logical controls.
    WhenGamepadButton {
        controller: Value,
        button: GamepadButton,
        pressed: bool,
    },
    WhenGamepadConnection {
        controller: Value,
        connected: bool,
    },
    WhenGamepadAxis {
        controller: Value,
        axis: GamepadAxis,
        threshold: Value,
        above: bool,
    },
    /// Resolves a fresh accessibility selector and controls its unique match.
    NativeUiAction {
        selector: Value,
        action: UiAction,
        value: Value,
    },
    /// A bounded multi-finger gesture on a target monitor.
    TouchGesture {
        gesture: TouchGestureKind,
        fingers: u8,
        x: Box<Value>,
        y: Box<Value>,
        end_x: Box<Value>,
        end_y: Box<Value>,
        duration: Box<Value>,
        span: Box<Value>,
        amount: Box<Value>,
        #[serde(default)]
        normalized: bool,
        #[serde(default)]
        monitor: u32,
    },
    /// Complete timed contact frames; absent contacts are released.
    TouchSequence {
        sequence: Box<Value>,
        #[serde(default)]
        normalized: bool,
        #[serde(default)]
        monitor: u32,
    },
    /// Header-only marker (like `WhenRan`/`BlockHeader`) for a strand whose
    /// body should run whenever the system's battery charge drops to (or
    /// below) the given percentage. Unlike `WhenRan`, this is *not* an
    /// entry point Run/Loop invokes - `Macro::run_with` skips these
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
    #[serde(deserialize_with = "super::saved_shapes::when_time")]
    WhenTime(TimeSchedule, Value),
    /// Header-only marker, no payload (like `WhenRan`) - excluded from
    /// Run/Loop and driven by the same background watcher as
    /// `WhenBattery*To` (`watch::battery` in the desktop app), which fires
    /// this strand's body the moment the system starts receiving external
    /// power. See `blockwork_core::battery::is_plugged_in`.
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
    /// `set clipboard to <value>`: overwrites the system clipboard with the
    /// value's text. See `blockwork_core::clipboard::set_text`.
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
    /// Stops the selected strands of the current macro run.
    Stop {
        #[serde(default)]
        scope: StopScope,
    },
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
    /// Blockwork extension; see `blockwork_core::browser`.
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
