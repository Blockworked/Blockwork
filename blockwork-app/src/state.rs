use blockwork_core::hotkey_types::{HotkeyAction, HotkeyBinding, KeyCombo};
use blockwork_core::input::schedule::TimeSchedule;
use blockwork_core::input::types::{Axis, Coordinate, Direction, InputToken};
use blockwork_core::input::{get_mouse_button_names, mouse_button_to_index};
use blockwork_core::macros::MacroGraph;
use blockwork_core::macros::backend::InputBackend;
use blockwork_core::macros::thread_pool::ThreadPool;
// Canvas addressing and undo come from blockstitch. Both these groups are
// re-exported so the rest of the crate still reaches them via `crate::state`.
pub(crate) use blockstitch_core::editor::{
    EditSession, History, InstrPath, PathStep, ValueBuffers, ValueLocation,
};
pub(crate) use blockwork_core::input::value::{Evaluated, Value};
pub(crate) use blockwork_core::macros::{
    BlockDef, BlockPiece, Comment, FloatingValue, Instruction, InstructionKind,
    Macro, MacroSettings, Strand,
};
pub(crate) use blockstitch_core::graph::{ListDef, ListItem};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::AppHandle;
use std::sync::{Arc, Mutex};

pub(crate) type SharedState = Arc<Mutex<AppState>>;

/// How many undo steps the editor keeps.
pub(crate) const UNDO_STACK_LIMIT: usize = 50;

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum RecordingPhase {
    Idle,
    Countdown(u8),
    Active,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum UpdateCheckState {
    Idle,
    Checking,
    UpToDate,
    UpdateAvailable(String),
    Applying,
    Error(String),
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Page {
    Main,
    Settings,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ComboCapture {
    Named(HotkeyAction),
    Binding(usize),
    Pending,
}

/// Where a captured keypress should be written once it arrives. `Strand`
/// writes straight into an instruction; `Standalone` (e.g. the sidebar's Key
/// prefab) has no instruction to write into, so the key is parked in
/// `AppState::pending_standalone_key` instead.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum KeyCaptureTarget {
    Strand(String, InstrPath),
    Standalone,
}

pub(crate) struct AppState {
    pub(crate) macro_selected: Option<usize>,
    pub(crate) current_macro: Option<Macro>,
    pub(crate) macros_list: Vec<Macro>,
    pub(crate) macro_strs: Vec<String>,
    pub(crate) emulator: Option<Arc<Mutex<dyn InputBackend>>>,
    /// Live macro-wide variable store, kept out of the main state lock so a
    /// long-running macro never blocks other commands. Synced from the
    /// selected macro on load, written back to disk once a run finishes.
    pub(crate) variable_values: Arc<Mutex<HashMap<String, Evaluated>>>,
    /// Live macro-wide list values, parallel to `variable_values`.
    pub(crate) list_values: Arc<Mutex<HashMap<String, Vec<ListItem>>>>,
    pub(crate) thread_pool: ThreadPool,
    pub(crate) is_looping: Arc<Mutex<bool>>,
    pub(crate) global_speed_multiplier: f64,
    pub(crate) ipc_server: Option<tokio::task::JoinHandle<()>>,
    pub(crate) ipc_shutdown_tx: Option<tokio::sync::watch::Sender<bool>>,
    pub(crate) ipc_active_port: Option<u16>,
    pub(crate) ipc_auto_start: bool,
    pub(crate) close_to_tray: bool,
    pub(crate) start_at_startup: bool,
    pub(crate) onboarding_done: bool,
    pub(crate) palette_layout: String,
    pub(crate) confirm_clear_instructions: bool,
    pub(crate) clear_confirm_remaining_secs: u8,
    pub(crate) clear_confirm_generation: u64,
    pub(crate) key_capture: Option<KeyCaptureTarget>,
    pub(crate) pending_standalone_key: Option<String>,
    /// Undo/redo over whole-canvas snapshots, plus the key that keeps a run
    /// of keystrokes to one step.
    pub(crate) history: History<MacroGraph>,
    pub(crate) recording_phase: RecordingPhase,
    pub(crate) recording_countdown_generation: u64,
    pub(crate) record_mouse_relative: bool,
    pub(crate) record_mouse_movement: bool,
    pub(crate) absolute_mouse_position_available: bool,
    pub(crate) wayland_session: bool,
    pub(crate) page: Page,
    pub(crate) combo_capture: Option<ComboCapture>,
    pub(crate) hotkey_bindings: Vec<HotkeyBinding>,
    pub(crate) pending_macro_hotkey: Option<(Option<usize>, Option<KeyCombo>)>,
    pub(crate) invalid_field_buffers: ValueBuffers,
    pub(crate) ipc_port_text: String,
    pub(crate) ipc_port_invalid: bool,
    pub(crate) update_check_state: UpdateCheckState,
    /// A macro file the user just picked via `import_macro`, waiting on its
    /// review - `confirm_import_macro`/`cancel_import_macro` resolve it.
    pub(crate) pending_import: Option<(Vec<Macro>, ImportReviewDto)>,
    /// A block pack waiting on its review.
    pub(crate) pending_pack: Option<crate::block_library::PendingPack>,
    /// Set by the emergency stop; watchers fire nothing until it's cleared.
    pub(crate) triggers_paused: bool,
}

// ─── Serializable DTO ──────────────────────────────────────────────────────

#[derive(Serialize, Clone)]
pub(crate) struct StateDto {
    pub(crate) macro_names: Vec<String>,
    pub(crate) macro_selected: Option<usize>,
    pub(crate) current_macro: Option<MacroDto>,
    pub(crate) macros_data: Vec<MacroDto>,
    pub(crate) global_speed_multiplier: f64,
    pub(crate) is_looping: bool,
    pub(crate) ipc_active_port: Option<u16>,
    pub(crate) ipc_auto_start: bool,
    pub(crate) close_to_tray: bool,
    pub(crate) start_at_startup: bool,
    pub(crate) onboarding_done: bool,
    pub(crate) palette_layout: String,
    pub(crate) confirm_clear_instructions: bool,
    pub(crate) confirm_clear_instructions_remaining_secs: u8,
    pub(crate) key_capture: Option<KeyCaptureDto>,
    pub(crate) standalone_key: Option<String>,
    pub(crate) can_undo: bool,
    pub(crate) can_redo: bool,
    pub(crate) recording_phase: RecordingPhaseDto,
    pub(crate) record_mouse_relative: bool,
    pub(crate) record_mouse_movement: bool,
    pub(crate) absolute_mouse_position_available: bool,
    pub(crate) wayland_session: bool,
    pub(crate) page: String,
    pub(crate) combo_capture: Option<ComboCaptureDto>,
    pub(crate) hotkey_bindings: Vec<HotkeyBindingDto>,
    pub(crate) named_hotkey_defaults: Vec<NamedHotkeyDefaultDto>,
    pub(crate) pending_macro_hotkey: Option<PendingMacroHotkeyDto>,
    pub(crate) invalid_field_buffers: Vec<InvalidFieldDto>,
    pub(crate) ipc_port_text: String,
    pub(crate) ipc_port_invalid: bool,
    pub(crate) emulator_available: bool,
    pub(crate) grab_available: bool,
    pub(crate) razer_permission_warning: bool,
    pub(crate) update_check_state: UpdateCheckStateDto,
    pub(crate) triggers_paused: bool,
    pub(crate) run_log: Vec<blockwork_core::run_log::Entry>,
    pub(crate) import_review: Option<ImportReviewDto>,
    pub(crate) globals: Vec<GlobalDto>,
    pub(crate) templates: Vec<crate::templates::TemplateDto>,
    /// Names only; values stay in the keyring.
    pub(crate) secret_names: Vec<String>,
    /// Every macro, for the library.
    pub(crate) library: Vec<LibraryEntryDto>,
    /// Block ids with a breakpoint in the open macro.
    pub(crate) breakpoints: Vec<String>,
    /// Ids of macros paused by the debugger.
    pub(crate) paused_macros: Vec<String>,
    /// Strands waiting at a breakpoint or step, with their variables.
    pub(crate) paused_strands: Vec<blockwork_core::macros::debug::PausedStrand>,
    /// What the open macro may do without asking; `None` when it's trusted.
    pub(crate) permissions: Option<Vec<String>>,
    pub(crate) webhook: crate::webhook::WebhookDto,
    pub(crate) block_library: crate::block_library::LibraryDto,
    pub(crate) pack_review: Option<crate::block_library::PackReviewDto>,
    /// UI language code; empty follows the system.
    pub(crate) language: String,
    /// UI animation duration multiplier; 0 is instant, 1 normal.
    pub(crate) animation_speed: f64,
}

/// One macro as the library lists it.
#[derive(Serialize, Clone)]
pub(crate) struct LibraryEntryDto {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) folder: String,
    pub(crate) always_listen: bool,
    /// Whether it has trigger strands (anything but When Ran and custom blocks).
    pub(crate) has_triggers: bool,
    /// Asks before risky abilities (see `blockwork_core::permissions`).
    pub(crate) restricted: bool,
}

/// A global variable as the settings page lists it.
#[derive(Serialize, Clone)]
pub(crate) struct GlobalDto {
    pub(crate) name: String,
    pub(crate) value: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "kind", content = "value")]
pub(crate) enum ListItemDto {
    Number(f64),
    Text(String),
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub(crate) struct ListDto {
    pub(crate) name: String,
    pub(crate) items: Vec<ListItemDto>,
    pub(crate) editor_visible: bool,
    pub(crate) editor_x: i32,
    pub(crate) editor_y: i32,
}

fn list_to_dto(list: &ListDef) -> ListDto {
    ListDto {
        name: list.name.clone(),
        items: list
            .items
            .iter()
            .map(|item| match item {
                ListItem::Number(value) => ListItemDto::Number(*value),
                ListItem::Text(value) => ListItemDto::Text(value.clone()),
            })
            .collect(),
        editor_visible: list.editor_visible,
        editor_x: list.editor_x,
        editor_y: list.editor_y,
    }
}

pub(crate) fn dto_to_list_item(item: &ListItemDto) -> ListItem {
    match item {
        ListItemDto::Number(value) => ListItem::Number(*value),
        ListItemDto::Text(value) => ListItem::Text(value.clone()),
    }
}

#[derive(Serialize, Clone)]
pub(crate) struct MacroDto {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) strands: Vec<StrandDto>,
    pub(crate) recording_target_strand_id: Option<String>,
    pub(crate) speed_multiplier: f64,
    pub(crate) floating_values: Vec<FloatingValue>,
    /// Floating/attached notes - see `Comment`.
    pub(crate) comments: Vec<Comment>,
    /// Declared variable names only, for the sidebar/dropdowns - current
    /// values aren't surfaced to the frontend.
    pub(crate) variables: Vec<String>,
    /// Macro-scoped lists, including their literal items for the list editor.
    pub(crate) lists: Vec<ListDto>,
    /// User-defined custom blocks ("My Blocks").
    pub(crate) block_defs: Vec<BlockDef>,
    /// Settings edited from the "Macro Settings" popup - see `MacroSettingsDto`.
    pub(crate) settings: MacroSettingsDto,
}

/// Wire shape for `MacroSettings` - see there for field meanings.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub(crate) struct MacroSettingsDto {
    pub(crate) always_listen: bool,
    pub(crate) max_run_secs: u32,
    pub(crate) max_loop_iterations: u32,
    pub(crate) only_in_window: String,
    pub(crate) power: blockwork_core::macros::conditions::PowerCondition,
    /// `HH:MM`, both empty when there are no active hours.
    pub(crate) active_from: String,
    pub(crate) active_until: String,
    pub(crate) active_days: u8,
    pub(crate) folder: String,
    pub(crate) catch_up_missed: bool,
}

fn macro_settings_to_dto(settings: &MacroSettings) -> MacroSettingsDto {
    use blockwork_core::macros::conditions::clock;
    let (active_from, active_until) = settings.active_hours.map(|(a, b)| (clock(a), clock(b))).unwrap_or_default();
    MacroSettingsDto {
        always_listen: settings.always_listen,
        max_run_secs: settings.max_run_secs,
        max_loop_iterations: settings.max_loop_iterations,
        only_in_window: settings.only_in_window.clone(),
        power: settings.power,
        active_from,
        active_until,
        active_days: settings.active_days,
        folder: settings.folder.clone(),
        catch_up_missed: settings.catch_up_missed,
    }
}

/// One non-default `MacroSettings` field an import wants confirmed - see
/// `commands::non_default_macro_settings`.
#[derive(Serialize, Clone)]
pub(crate) struct CustomMacroSettingDto {
    pub(crate) key: String,
    pub(crate) label: String,
    pub(crate) enabled: bool,
}

/// One thing an imported macro can do - see `commands::macro_capabilities`.
#[derive(Serialize, Clone)]
pub(crate) struct CapabilityDto {
    pub(crate) kind: String,
    pub(crate) label: String,
    /// Runs commands, goes online or writes files.
    pub(crate) risky: bool,
    /// Asks first unless allowed (see `blockwork_core::permissions::GUARDED`).
    pub(crate) guarded: bool,
}

/// What the user reviews before the staged `pending_import` macro is added.
#[derive(Serialize, Clone)]
pub(crate) struct ImportReviewDto {
    pub(crate) name: String,
    /// Every macro in a bundle; one name for a single macro.
    pub(crate) macros: Vec<String>,
    /// What upgrading older macros changed.
    pub(crate) notes: Vec<String>,
    pub(crate) capabilities: Vec<CapabilityDto>,
    pub(crate) custom_settings: Vec<CustomMacroSettingDto>,
}

#[derive(Serialize, Clone)]
pub(crate) struct StrandDto {
    pub(crate) id: String,
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) instructions: Vec<InstructionDto>,
}

#[derive(Serialize, Clone)]
pub(crate) struct KeyCaptureDto {
    pub(crate) kind: String,
    pub(crate) strand_id: Option<String>,
    pub(crate) index: Option<InstrPath>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "type")]
pub(crate) enum InstructionDto {
    Wait {
        id: String,
        duration: Value,
    },
    Text {
        id: String,
        text: Value,
    },
    Key {
        id: String,
        key: Value,
        direction: String,
    },
    WhenKey {
        id: String,
        key: Value,
        direction: String,
    },
    Button {
        id: String,
        button: String,
        direction: String,
    },
    MoveMouse {
        id: String,
        x: Value,
        y: Value,
        coordinate: String,
    },
    Scroll {
        id: String,
        amount: Value,
        axis: String,
    },
    Command {
        id: String,
        command: String,
    },
    Comment {
        id: String,
        comment: String,
    },
    WhenRan {
        id: String,
    },
    WhenBatteryDischargedTo {
        id: String,
        threshold: Value,
    },
    WhenBatteryChargedTo {
        id: String,
        threshold: Value,
    },
    WhenTime {
        id: String,
        schedule: TimeSchedule,
    },
    WhenPowerPluggedIn {
        id: String,
    },
    WhenPowerUnplugged {
        id: String,
    },
    WhenClipboardChanged {
        id: String,
    },
    OpenApp {
        id: String,
        command: String,
        name: String,
        icon: Option<String>,
    },
    CloseApp {
        id: String,
        command: String,
        name: String,
        icon: Option<String>,
    },
    SetVariable {
        id: String,
        name: String,
        value: Value,
    },
    ChangeVariable {
        id: String,
        name: String,
        value: Value,
    },
    SetClipboard {
        id: String,
        value: Value,
    },
    AddToList { id: String, value: Value, name: String },
    DeleteOfList { id: String, index: Value, name: String },
    DeleteAllOfList { id: String, name: String },
    ShiftList { id: String, name: String, amount: Value },
    InsertIntoList { id: String, value: Value, index: Value, name: String },
    ReplaceItemOfList { id: String, index: Value, name: String, value: Value },
    ReverseList { id: String, name: String },
    BlockHeader {
        id: String,
        block_id: String,
    },
    CallBlock {
        id: String,
        block_id: String,
        args: Vec<Value>,
    },
    BranchCallBlock {
        id: String,
        block_id: String,
        args: Vec<Value>,
        branches: Vec<Vec<InstructionDto>>,
    },
    RunBranch {
        id: String,
        name: String,
    },
    Return {
        id: String,
        value: Value,
    },
    If {
        id: String,
        condition: Value,
        body: Vec<InstructionDto>,
    },
    IfElse {
        id: String,
        condition: Value,
        then_body: Vec<InstructionDto>,
        else_body: Vec<InstructionDto>,
    },
    Repeat {
        id: String,
        count: Value,
        body: Vec<InstructionDto>,
    },
    Forever {
        id: String,
        body: Vec<InstructionDto>,
    },
    While {
        id: String,
        condition: Value,
        body: Vec<InstructionDto>,
    },
    EscapeLoop {
        id: String,
    },
    ContinueLoop {
        id: String,
    },
    WriteFile {
        id: String,
        path: Value,
        text: Value,
        append: bool,
    },
    HttpRequest {
        id: String,
        method: String,
        url: Value,
        body: Value,
        headers: Value,
        variable: String,
    },
    Notify {
        id: String,
        title: Value,
        message: Value,
    },
    Log {
        id: String,
        value: Value,
    },
    TryCatch {
        id: String,
        body: Vec<InstructionDto>,
        catch_body: Vec<InstructionDto>,
    },
    ThrowError {
        id: String,
        message: Value,
    },
    StopMacro {
        id: String,
    },
    WhenWindow {
        id: String,
        pattern: Value,
        focused: bool,
    },
    WhenFileChanged {
        id: String,
        path: Value,
    },
    Retry {
        id: String,
        count: Value,
        delay: Value,
        body: Vec<InstructionDto>,
    },
    WindowAction {
        id: String,
        action: String,
        pattern: Value,
        #[serde(default = "match_by_title")]
        by: String,
    },
    MoveWindow {
        id: String,
        pattern: Value,
        #[serde(default = "match_by_title")]
        by: String,
        resize: bool,
        x: Value,
        y: Value,
    },
    RunMacro {
        id: String,
        target: Value,
        wait: bool,
    },
    StopMacroNamed {
        id: String,
        target: Value,
    },
    SetGlobal {
        id: String,
        name: Value,
        value: Value,
    },
    ChangeGlobal {
        id: String,
        name: Value,
        by: Value,
    },
    WaitUntil {
        id: String,
        condition: Value,
        timeout: Value,
    },
    FindImage {
        id: String,
        image: String,
        tolerance: Value,
        x_variable: String,
        y_variable: String,
    },
    ClickImage {
        id: String,
        image: String,
        tolerance: Value,
        button: blockwork_core::input::types::MacroButton,
    },
    WaitForImage {
        id: String,
        image: String,
        tolerance: Value,
        appear: bool,
        timeout: Value,
    },
    WhenTyped {
        id: String,
        text: Value,
        erase: bool,
    },
    PasteText {
        id: String,
        text: Value,
    },
    WhenApp {
        id: String,
        pattern: Value,
        started: bool,
    },
    WhenIdle {
        id: String,
        minutes: Value,
    },
    WhenScreenLock {
        id: String,
        locked: bool,
    },
    WithLock {
        id: String,
        name: Value,
        body: Vec<InstructionDto>,
    },
    WhenCron {
        id: String,
        expression: Value,
    },
    SplitIntoList {
        id: String,
        text: Value,
        separator: Value,
        name: String,
    },
    WhenNetwork {
        id: String,
        connected: bool,
    },
    WhenUsb {
        id: String,
        pattern: Value,
        connected: bool,
    },
    WhenMouseButton {
        id: String,
        button: String,
        pressed: bool,
        modifiers: Value,
    },
    AskText {
        id: String,
        prompt: Value,
        default: Value,
        name: String,
    },
    AskYesNo {
        id: String,
        prompt: Value,
        name: String,
    },
    AskChoice {
        id: String,
        prompt: Value,
        list: String,
        name: String,
    },
    ShowMessage {
        id: String,
        message: Value,
    },
    WhenAt {
        id: String,
        at: Value,
    },
    WhenSignal {
        id: String,
        name: Value,
    },
    SendSignal {
        id: String,
        name: Value,
        wait: bool,
    },
    WhenWebhook {
        id: String,
        path: Value,
        variable: String,
    },
    DetectObjectsInWindow {
        id: String,
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
        id: String,
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
        id: String,
        pattern: Value,
        by: String,
        target_x: Value,
        target_y: Value,
        gain: Value,
        max_step: Value,
        deadzone: Value,
    },
    AimAtDetection {
        id: String,
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

/// One entry in the "Open App" picker's list - see `installed_apps`.
/// `command` is the ready-to-launch string an `InstructionKind::OpenApp` stores
/// as-is; `icon`, when present, is a `data:` URI.
#[derive(Serialize, Clone)]
pub(crate) struct AppEntryDto {
    pub(crate) name: String,
    pub(crate) command: String,
    pub(crate) icon: Option<String>,
}

#[derive(Serialize, Clone)]
pub(crate) struct RecordingPhaseDto {
    pub(crate) phase: String,
    pub(crate) countdown: Option<u8>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "type")]
pub(crate) enum HotkeyActionDto {
    RunMacro,
    StopLoop,
    EmergencyStop,
    NextMacro,
    PrevMacro,
    StartRecordingImmediate,
    StopRecording,
    Undo,
    Redo,
    RunSpecificMacro { macro_id: String },
}

#[derive(Serialize, Clone)]
pub(crate) struct HotkeyBindingDto {
    pub(crate) binding_index: usize,
    pub(crate) action: HotkeyActionDto,
    pub(crate) combo_display: String,
    pub(crate) macro_name: Option<String>,
}

#[derive(Serialize, Clone)]
pub(crate) struct NamedHotkeyDefaultDto {
    pub(crate) action: HotkeyActionDto,
    pub(crate) combo_display: Option<String>,
}

#[derive(Serialize, Clone)]
pub(crate) struct ComboCaptureDto {
    pub(crate) kind: String,
    pub(crate) action: Option<HotkeyActionDto>,
    pub(crate) binding_index: Option<usize>,
}

#[derive(Serialize, Clone)]
pub(crate) struct PendingMacroHotkeyDto {
    pub(crate) macro_index: Option<usize>,
    pub(crate) combo_display: Option<String>,
}

#[derive(Serialize, Clone)]
pub(crate) struct InvalidFieldDto {
    pub(crate) location: ValueLocation,
    pub(crate) text: String,
}

#[derive(Serialize, Clone)]
pub(crate) struct UpdateCheckStateDto {
    pub(crate) state: String,
    pub(crate) version: Option<String>,
    pub(crate) error: Option<String>,
}

// ─── Conversions ───────────────────────────────────────────────────────────

fn direction_to_str(d: &Direction) -> &'static str {
    match d {
        Direction::Click => "Click",
        Direction::Press => "Press",
        Direction::Release => "Release",
    }
}

fn str_to_direction(s: &str) -> Direction {
    match s {
        "Press" => Direction::Press,
        "Release" => Direction::Release,
        _ => Direction::Click,
    }
}

fn coordinate_to_str(c: &Coordinate) -> &'static str {
    match c {
        Coordinate::Abs => "Absolute",
        Coordinate::Rel => "Relative",
    }
}

fn str_to_coordinate(s: &str) -> Coordinate {
    match s {
        "Relative" => Coordinate::Rel,
        _ => Coordinate::Abs,
    }
}

fn axis_to_str(a: &Axis) -> &'static str {
    match a {
        Axis::Vertical => "Vertical",
        Axis::Horizontal => "Horizontal",
    }
}

fn str_to_axis(s: &str) -> Axis {
    match s {
        "Horizontal" => Axis::Horizontal,
        _ => Axis::Vertical,
    }
}

fn match_by_title() -> String {
    "Title".into()
}

pub(crate) fn instruction_to_dto(ins: &Instruction) -> InstructionDto {
    let id = ins.id.clone();
    match &ins.kind {
        InstructionKind::Wait(dur) => InstructionDto::Wait {
            id,
            duration: dur.clone(),
        },
        InstructionKind::Command(cmd) => InstructionDto::Command {
            id,
            command: cmd.clone(),
        },
        InstructionKind::Comment(c) => InstructionDto::Comment {
            id,
            comment: c.clone(),
        },
        InstructionKind::WhenRan => InstructionDto::WhenRan { id },
        InstructionKind::WhenBatteryDischargedTo(threshold) => {
            InstructionDto::WhenBatteryDischargedTo {
                id,
                threshold: threshold.clone(),
            }
        }
        InstructionKind::WhenBatteryChargedTo(threshold) => InstructionDto::WhenBatteryChargedTo {
            id,
            threshold: threshold.clone(),
        },
        InstructionKind::WhenTime(schedule) => InstructionDto::WhenTime {
            id,
            schedule: *schedule,
        },
        InstructionKind::WhenPowerPluggedIn => InstructionDto::WhenPowerPluggedIn { id },
        InstructionKind::WhenPowerUnplugged => InstructionDto::WhenPowerUnplugged { id },
        InstructionKind::WhenClipboardChanged => InstructionDto::WhenClipboardChanged { id },
        InstructionKind::WhenKey(key, direction) => InstructionDto::WhenKey {
            id,
            key: key.clone(),
            direction: direction_to_str(direction).to_string(),
        },
        InstructionKind::OpenApp {
            command,
            name,
            icon,
        } => InstructionDto::OpenApp {
            id,
            command: command.clone(),
            name: name.clone(),
            icon: icon.clone(),
        },
        InstructionKind::CloseApp {
            command,
            name,
            icon,
        } => InstructionDto::CloseApp {
            id,
            command: command.clone(),
            name: name.clone(),
            icon: icon.clone(),
        },
        InstructionKind::SetVariable(name, value) => InstructionDto::SetVariable {
            id,
            name: name.clone(),
            value: value.clone(),
        },
        InstructionKind::ChangeVariable(name, value) => InstructionDto::ChangeVariable {
            id,
            name: name.clone(),
            value: value.clone(),
        },
        InstructionKind::SetClipboard(value) => InstructionDto::SetClipboard {
            id,
            value: value.clone(),
        },
        InstructionKind::AddToList { value, name } => InstructionDto::AddToList { id, value: value.clone(), name: name.clone() },
        InstructionKind::DeleteOfList { index, name } => InstructionDto::DeleteOfList { id, index: index.clone(), name: name.clone() },
        InstructionKind::DeleteAllOfList { name } => InstructionDto::DeleteAllOfList { id, name: name.clone() },
        InstructionKind::ShiftList { name, amount } => InstructionDto::ShiftList { id, name: name.clone(), amount: amount.clone() },
        InstructionKind::InsertIntoList { value, index, name } => InstructionDto::InsertIntoList { id, value: value.clone(), index: index.clone(), name: name.clone() },
        InstructionKind::ReplaceItemOfList { index, name, value } => InstructionDto::ReplaceItemOfList { id, index: index.clone(), name: name.clone(), value: value.clone() },
        InstructionKind::ReverseList { name } => InstructionDto::ReverseList { id, name: name.clone() },
        InstructionKind::BlockHeader(block_id) => InstructionDto::BlockHeader {
            id,
            block_id: block_id.clone(),
        },
        InstructionKind::CallBlock {
            block_id,
            args,
            branches,
        } if branches.is_empty() => InstructionDto::CallBlock {
            id,
            block_id: block_id.clone(),
            args: args.clone(),
        },
        InstructionKind::CallBlock {
            block_id,
            args,
            branches,
        } => InstructionDto::BranchCallBlock {
            id,
            block_id: block_id.clone(),
            args: args.clone(),
            branches: branches
                .iter()
                .map(|branch| branch.iter().map(instruction_to_dto).collect())
                .collect(),
        },
        InstructionKind::RunBranch(name) => InstructionDto::RunBranch {
            id,
            name: name.clone(),
        },
        InstructionKind::Return(value) => InstructionDto::Return {
            id,
            value: value.clone(),
        },
        InstructionKind::If { condition, body } => InstructionDto::If {
            id,
            condition: condition.clone(),
            body: body.iter().map(instruction_to_dto).collect(),
        },
        InstructionKind::IfElse {
            condition,
            then_body,
            else_body,
        } => InstructionDto::IfElse {
            id,
            condition: condition.clone(),
            then_body: then_body.iter().map(instruction_to_dto).collect(),
            else_body: else_body.iter().map(instruction_to_dto).collect(),
        },
        InstructionKind::Repeat { count, body } => InstructionDto::Repeat {
            id,
            count: count.clone(),
            body: body.iter().map(instruction_to_dto).collect(),
        },
        InstructionKind::Forever { body } => InstructionDto::Forever {
            id,
            body: body.iter().map(instruction_to_dto).collect(),
        },
        InstructionKind::While { condition, body } => InstructionDto::While {
            id,
            condition: condition.clone(),
            body: body.iter().map(instruction_to_dto).collect(),
        },
        InstructionKind::EscapeLoop => InstructionDto::EscapeLoop { id },
        InstructionKind::ContinueLoop => InstructionDto::ContinueLoop { id },
        InstructionKind::WriteFile { path, text, append } => InstructionDto::WriteFile {
            id,
            path: path.clone(),
            text: text.clone(),
            append: *append,
        },
        InstructionKind::HttpRequest {
            method,
            url,
            body,
            headers,
            variable,
        } => InstructionDto::HttpRequest {
            id,
            method: method.clone(),
            url: url.clone(),
            body: body.clone(),
            headers: headers.clone(),
            variable: variable.clone(),
        },
        InstructionKind::Notify { title, message } => InstructionDto::Notify {
            id,
            title: title.clone(),
            message: message.clone(),
        },
        InstructionKind::Log(value) => InstructionDto::Log {
            id,
            value: value.clone(),
        },
        InstructionKind::TryCatch { body, catch_body } => InstructionDto::TryCatch {
            id,
            body: body.iter().map(instruction_to_dto).collect(),
            catch_body: catch_body.iter().map(instruction_to_dto).collect(),
        },
        InstructionKind::ThrowError(message) => InstructionDto::ThrowError {
            id,
            message: message.clone(),
        },
        InstructionKind::StopMacro => InstructionDto::StopMacro { id },
        InstructionKind::WhenWindow { pattern, focused } => InstructionDto::WhenWindow {
            id,
            pattern: pattern.clone(),
            focused: *focused,
        },
        InstructionKind::WhenFileChanged(path) => InstructionDto::WhenFileChanged {
            id,
            path: path.clone(),
        },
        InstructionKind::Retry { count, delay, body } => InstructionDto::Retry {
            id,
            count: count.clone(),
            delay: delay.clone(),
            body: body.iter().map(instruction_to_dto).collect(),
        },
        InstructionKind::WindowAction { action, pattern, by } => InstructionDto::WindowAction {
            id,
            action: action.clone(),
            pattern: pattern.clone(),
            by: by.clone(),
        },
        InstructionKind::MoveWindow { pattern, by, resize, x, y } => InstructionDto::MoveWindow {
            id,
            pattern: pattern.clone(),
            by: by.clone(),
            resize: *resize,
            x: x.clone(),
            y: y.clone(),
        },
        InstructionKind::RunMacro { target, wait } => InstructionDto::RunMacro {
            id,
            target: target.clone(),
            wait: *wait,
        },
        InstructionKind::StopMacroNamed(target) => InstructionDto::StopMacroNamed { id, target: target.clone() },
        InstructionKind::SetGlobal { name, value } => InstructionDto::SetGlobal {
            id,
            name: name.clone(),
            value: value.clone(),
        },
        InstructionKind::ChangeGlobal { name, by } => InstructionDto::ChangeGlobal {
            id,
            name: name.clone(),
            by: by.clone(),
        },
        InstructionKind::WaitUntil { condition, timeout } => InstructionDto::WaitUntil {
            id,
            condition: condition.clone(),
            timeout: timeout.clone(),
        },
        InstructionKind::FindImage { image, tolerance, x_variable, y_variable } => InstructionDto::FindImage {
            id,
            image: image.clone(),
            tolerance: tolerance.clone(),
            x_variable: x_variable.clone(),
            y_variable: y_variable.clone(),
        },
        InstructionKind::ClickImage { image, tolerance, button } => InstructionDto::ClickImage {
            id,
            image: image.clone(),
            tolerance: tolerance.clone(),
            button: button.clone(),
        },
        InstructionKind::WaitForImage { image, tolerance, appear, timeout } => InstructionDto::WaitForImage {
            id,
            image: image.clone(),
            tolerance: tolerance.clone(),
            appear: *appear,
            timeout: timeout.clone(),
        },
        InstructionKind::WhenTyped { text, erase } => InstructionDto::WhenTyped { id, text: text.clone(), erase: *erase },
        InstructionKind::PasteText(text) => InstructionDto::PasteText { id, text: text.clone() },
        InstructionKind::WhenApp { pattern, started } => {
            InstructionDto::WhenApp { id, pattern: pattern.clone(), started: *started }
        }
        InstructionKind::WhenIdle(minutes) => InstructionDto::WhenIdle { id, minutes: minutes.clone() },
        InstructionKind::WhenScreenLock { locked } => InstructionDto::WhenScreenLock { id, locked: *locked },
        InstructionKind::WithLock { name, body } => InstructionDto::WithLock {
            id,
            name: name.clone(),
            body: body.iter().map(instruction_to_dto).collect(),
        },
        InstructionKind::WhenCron(expression) => InstructionDto::WhenCron { id, expression: expression.clone() },
        InstructionKind::SplitIntoList { text, separator, name } => InstructionDto::SplitIntoList {
            id,
            text: text.clone(),
            separator: separator.clone(),
            name: name.clone(),
        },
        InstructionKind::WhenNetwork { connected } => InstructionDto::WhenNetwork { id, connected: *connected },
        InstructionKind::WhenUsb { pattern, connected } => {
            InstructionDto::WhenUsb { id, pattern: pattern.clone(), connected: *connected }
        }
        InstructionKind::WhenMouseButton { button, pressed, modifiers } => InstructionDto::WhenMouseButton {
            id,
            button: get_mouse_button_names()[mouse_button_to_index(button)].to_string(),
            pressed: *pressed,
            modifiers: modifiers.clone(),
        },
        InstructionKind::AskText { prompt, default, name } => {
            InstructionDto::AskText { id, prompt: prompt.clone(), default: default.clone(), name: name.clone() }
        }
        InstructionKind::AskYesNo { prompt, name } => InstructionDto::AskYesNo { id, prompt: prompt.clone(), name: name.clone() },
        InstructionKind::AskChoice { prompt, list, name } => {
            InstructionDto::AskChoice { id, prompt: prompt.clone(), list: list.clone(), name: name.clone() }
        }
        InstructionKind::ShowMessage(message) => InstructionDto::ShowMessage { id, message: message.clone() },
        InstructionKind::WhenAt(at) => InstructionDto::WhenAt { id, at: at.clone() },
        InstructionKind::WhenSignal(name) => InstructionDto::WhenSignal { id, name: name.clone() },
        InstructionKind::SendSignal { name, wait } => InstructionDto::SendSignal { id, name: name.clone(), wait: *wait },
        InstructionKind::WhenWebhook { path, variable } => {
            InstructionDto::WhenWebhook { id, path: path.clone(), variable: variable.clone() }
        }
        InstructionKind::DetectObjectsInWindow { pattern, by, detector, label, confidence, sort, results_list, count_variable, region } => {
            InstructionDto::DetectObjectsInWindow {
                region: region.clone(),
                id,
                pattern: pattern.clone(),
                by: by.clone(),
                detector: detector.clone(),
                label: label.clone(),
                confidence: confidence.clone(),
                sort: sort.clone(),
                results_list: results_list.clone(),
                count_variable: count_variable.clone(),
            }
        }
        InstructionKind::WaitForDetection { pattern, by, detector, label, confidence, appear, timeout, region } => {
            InstructionDto::WaitForDetection {
                region: region.clone(),
                id,
                pattern: pattern.clone(),
                by: by.clone(),
                detector: detector.clone(),
                label: label.clone(),
                confidence: confidence.clone(),
                appear: *appear,
                timeout: timeout.clone(),
            }
        }
        InstructionKind::AimStep { pattern, by, target_x, target_y, gain, max_step, deadzone } => InstructionDto::AimStep {
            id,
            pattern: pattern.clone(),
            by: by.clone(),
            target_x: target_x.clone(),
            target_y: target_y.clone(),
            gain: gain.clone(),
            max_step: max_step.clone(),
            deadzone: deadzone.clone(),
        },
        InstructionKind::AimAtDetection { results_list, index, gain, max_step, deadzone, timeout, settle_ms, status_variable } => {
            InstructionDto::AimAtDetection {
                id,
                results_list: results_list.clone(),
                index: index.clone(),
                gain: gain.clone(),
                max_step: max_step.clone(),
                deadzone: deadzone.clone(),
                timeout: timeout.clone(),
                settle_ms: settle_ms.clone(),
                status_variable: status_variable.clone(),
            }
        }
        InstructionKind::Token(token) => match token {
            InputToken::Text(t) => InstructionDto::Text {
                id,
                text: t.clone(),
            },
            InputToken::Key(k, d) => InstructionDto::Key {
                id,
                key: k.clone(),
                direction: direction_to_str(d).to_string(),
            },
            InputToken::Button(b, d) => InstructionDto::Button {
                id,
                button: get_mouse_button_names()[mouse_button_to_index(b)].to_string(),
                direction: direction_to_str(d).to_string(),
            },
            InputToken::MoveMouse(x, y, coord) => InstructionDto::MoveMouse {
                id,
                x: x.clone(),
                y: y.clone(),
                coordinate: coordinate_to_str(coord).to_string(),
            },
            InputToken::Scroll(amt, axis) => InstructionDto::Scroll {
                id,
                amount: amt.clone(),
                axis: axis_to_str(axis).to_string(),
            },
            InputToken::Raw(_, _) => InstructionDto::Comment {
                id,
                comment: "(raw keycode)".to_string(),
            },
        },
    }
}

pub(crate) fn dto_to_instruction(dto: &InstructionDto) -> Option<Instruction> {
    use blockwork_core::input::index_to_mouse_button;
    let (id, kind) = match dto {
        InstructionDto::Wait { id, duration } => {
            (id, InstructionKind::Wait(duration.clone()))
        }
        InstructionDto::Text { id, text } => (
            id,
            InstructionKind::Token(InputToken::Text(text.clone())),
        ),
        InstructionDto::Key { id, key, direction } => (
            id,
            InstructionKind::Token(InputToken::Key(key.clone(), str_to_direction(direction))),
        ),
        InstructionDto::WhenKey { id, key, direction } => (
            id,
            InstructionKind::WhenKey(key.clone(), str_to_direction(direction)),
        ),
        InstructionDto::Button {
            id,
            button,
            direction,
        } => {
            let names = get_mouse_button_names();
            let idx = names
                .iter()
                .position(|&n| n == button.as_str())
                .unwrap_or(0);
            (
                id,
                InstructionKind::Token(InputToken::Button(
                    index_to_mouse_button(idx),
                    str_to_direction(direction),
                )),
            )
        }
        InstructionDto::MoveMouse {
            id,
            x,
            y,
            coordinate,
        } => (
            id,
            InstructionKind::Token(InputToken::MoveMouse(
                x.clone(),
                y.clone(),
                str_to_coordinate(coordinate),
            )),
        ),
        InstructionDto::Scroll { id, amount, axis } => (
            id,
            InstructionKind::Token(InputToken::Scroll(amount.clone(), str_to_axis(axis))),
        ),
        InstructionDto::Command { id, command } => (id, InstructionKind::Command(command.clone())),
        InstructionDto::Comment { id, comment } => (id, InstructionKind::Comment(comment.clone())),
        InstructionDto::WhenRan { id } => (id, InstructionKind::WhenRan),
        InstructionDto::WhenBatteryDischargedTo { id, threshold } => (
            id,
            InstructionKind::WhenBatteryDischargedTo(threshold.clone()),
        ),
        InstructionDto::WhenBatteryChargedTo { id, threshold } => (
            id,
            InstructionKind::WhenBatteryChargedTo(threshold.clone()),
        ),
        InstructionDto::WhenTime { id, schedule } => (id, InstructionKind::WhenTime(*schedule)),
        InstructionDto::WhenPowerPluggedIn { id } => (id, InstructionKind::WhenPowerPluggedIn),
        InstructionDto::WhenPowerUnplugged { id } => (id, InstructionKind::WhenPowerUnplugged),
        InstructionDto::WhenClipboardChanged { id } => (id, InstructionKind::WhenClipboardChanged),
        InstructionDto::OpenApp {
            id,
            command,
            name,
            icon,
        } => (
            id,
            InstructionKind::OpenApp {
                command: command.clone(),
                name: name.clone(),
                icon: icon.clone(),
            },
        ),
        InstructionDto::CloseApp {
            id,
            command,
            name,
            icon,
        } => (
            id,
            InstructionKind::CloseApp {
                command: command.clone(),
                name: name.clone(),
                icon: icon.clone(),
            },
        ),
        InstructionDto::SetVariable { id, name, value } => (
            id,
            InstructionKind::SetVariable(name.clone(), value.clone()),
        ),
        InstructionDto::ChangeVariable { id, name, value } => (
            id,
            InstructionKind::ChangeVariable(name.clone(), value.clone()),
        ),
        InstructionDto::SetClipboard { id, value } => (
            id,
            InstructionKind::SetClipboard(value.clone()),
        ),
        InstructionDto::AddToList { id, value, name } => (id, InstructionKind::AddToList { value: value.clone(), name: name.clone() }),
        InstructionDto::DeleteOfList { id, index, name } => (id, InstructionKind::DeleteOfList { index: index.clone(), name: name.clone() }),
        InstructionDto::DeleteAllOfList { id, name } => (id, InstructionKind::DeleteAllOfList { name: name.clone() }),
        InstructionDto::ShiftList { id, name, amount } => (id, InstructionKind::ShiftList { name: name.clone(), amount: amount.clone() }),
        InstructionDto::InsertIntoList { id, value, index, name } => (id, InstructionKind::InsertIntoList { value: value.clone(), index: index.clone(), name: name.clone() }),
        InstructionDto::ReplaceItemOfList { id, index, name, value } => (id, InstructionKind::ReplaceItemOfList { index: index.clone(), name: name.clone(), value: value.clone() }),
        InstructionDto::ReverseList { id, name } => (id, InstructionKind::ReverseList { name: name.clone() }),
        InstructionDto::BlockHeader { id, block_id } => {
            (id, InstructionKind::BlockHeader(block_id.clone()))
        }
        InstructionDto::CallBlock { id, block_id, args } => (
            id,
            InstructionKind::CallBlock {
                block_id: block_id.clone(),
                args: args.clone(),
                branches: Vec::new(),
            },
        ),
        InstructionDto::BranchCallBlock {
            id,
            block_id,
            args,
            branches,
        } => (
            id,
            InstructionKind::CallBlock {
                block_id: block_id.clone(),
                args: args.clone(),
                branches: branches
                    .iter()
                    .map(|branch| branch.iter().filter_map(dto_to_instruction).collect())
                    .collect(),
            },
        ),
        InstructionDto::RunBranch { id, name } => (id, InstructionKind::RunBranch(name.clone())),
        InstructionDto::Return { id, value } => (id, InstructionKind::Return(value.clone())),
        InstructionDto::If {
            id,
            condition,
            body,
        } => (
            id,
            InstructionKind::If {
                condition: condition.clone(),
                body: body
                    .iter()
                    .map(dto_to_instruction)
                    .collect::<Option<Vec<_>>>()?,
            },
        ),
        InstructionDto::IfElse {
            id,
            condition,
            then_body,
            else_body,
        } => (
            id,
            InstructionKind::IfElse {
                condition: condition.clone(),
                then_body: then_body
                    .iter()
                    .map(dto_to_instruction)
                    .collect::<Option<Vec<_>>>()?,
                else_body: else_body
                    .iter()
                    .map(dto_to_instruction)
                    .collect::<Option<Vec<_>>>()?,
            },
        ),
        InstructionDto::Repeat { id, count, body } => (
            id,
            InstructionKind::Repeat {
                count: count.clone(),
                body: body
                    .iter()
                    .map(dto_to_instruction)
                    .collect::<Option<Vec<_>>>()?,
            },
        ),
        InstructionDto::Forever { id, body } => (
            id,
            InstructionKind::Forever {
                body: body
                    .iter()
                    .map(dto_to_instruction)
                    .collect::<Option<Vec<_>>>()?,
            },
        ),
        InstructionDto::While {
            id,
            condition,
            body,
        } => (
            id,
            InstructionKind::While {
                condition: condition.clone(),
                body: body
                    .iter()
                    .map(dto_to_instruction)
                    .collect::<Option<Vec<_>>>()?,
            },
        ),
        InstructionDto::EscapeLoop { id } => (id, InstructionKind::EscapeLoop),
        InstructionDto::ContinueLoop { id } => (id, InstructionKind::ContinueLoop),
        InstructionDto::WriteFile {
            id,
            path,
            text,
            append,
        } => (
            id,
            InstructionKind::WriteFile {
                path: path.clone(),
                text: text.clone(),
                append: *append,
            },
        ),
        InstructionDto::HttpRequest {
            id,
            method,
            url,
            body,
            headers,
            variable,
        } => (
            id,
            InstructionKind::HttpRequest {
                method: method.clone(),
                url: url.clone(),
                body: body.clone(),
                headers: headers.clone(),
                variable: variable.clone(),
            },
        ),
        InstructionDto::Notify { id, title, message } => (
            id,
            InstructionKind::Notify {
                title: title.clone(),
                message: message.clone(),
            },
        ),
        InstructionDto::Log { id, value } => (id, InstructionKind::Log(value.clone())),
        InstructionDto::TryCatch {
            id,
            body,
            catch_body,
        } => (
            id,
            InstructionKind::TryCatch {
                body: body
                    .iter()
                    .map(dto_to_instruction)
                    .collect::<Option<Vec<_>>>()?,
                catch_body: catch_body
                    .iter()
                    .map(dto_to_instruction)
                    .collect::<Option<Vec<_>>>()?,
            },
        ),
        InstructionDto::ThrowError { id, message } => {
            (id, InstructionKind::ThrowError(message.clone()))
        }
        InstructionDto::StopMacro { id } => (id, InstructionKind::StopMacro),
        InstructionDto::WhenWindow {
            id,
            pattern,
            focused,
        } => (
            id,
            InstructionKind::WhenWindow {
                pattern: pattern.clone(),
                focused: *focused,
            },
        ),
        InstructionDto::WhenFileChanged { id, path } => {
            (id, InstructionKind::WhenFileChanged(path.clone()))
        }
        InstructionDto::Retry { id, count, delay, body } => (
            id,
            InstructionKind::Retry {
                count: count.clone(),
                delay: delay.clone(),
                body: body.iter().map(dto_to_instruction).collect::<Option<Vec<_>>>()?,
            },
        ),
        InstructionDto::WindowAction { id, action, pattern, by } => (
            id,
            InstructionKind::WindowAction { action: action.clone(), pattern: pattern.clone(), by: by.clone() },
        ),
        InstructionDto::MoveWindow { id, pattern, by, resize, x, y } => (
            id,
            InstructionKind::MoveWindow {
                pattern: pattern.clone(),
                by: by.clone(),
                resize: *resize,
                x: x.clone(),
                y: y.clone(),
            },
        ),
        InstructionDto::RunMacro { id, target, wait } => {
            (id, InstructionKind::RunMacro { target: target.clone(), wait: *wait })
        }
        InstructionDto::StopMacroNamed { id, target } => (id, InstructionKind::StopMacroNamed(target.clone())),
        InstructionDto::SetGlobal { id, name, value } => {
            (id, InstructionKind::SetGlobal { name: name.clone(), value: value.clone() })
        }
        InstructionDto::ChangeGlobal { id, name, by } => {
            (id, InstructionKind::ChangeGlobal { name: name.clone(), by: by.clone() })
        }
        InstructionDto::WaitUntil { id, condition, timeout } => {
            (id, InstructionKind::WaitUntil { condition: condition.clone(), timeout: timeout.clone() })
        }
        InstructionDto::FindImage { id, image, tolerance, x_variable, y_variable } => (
            id,
            InstructionKind::FindImage {
                image: image.clone(),
                tolerance: tolerance.clone(),
                x_variable: x_variable.clone(),
                y_variable: y_variable.clone(),
            },
        ),
        InstructionDto::ClickImage { id, image, tolerance, button } => (
            id,
            InstructionKind::ClickImage { image: image.clone(), tolerance: tolerance.clone(), button: button.clone() },
        ),
        InstructionDto::WaitForImage { id, image, tolerance, appear, timeout } => (
            id,
            InstructionKind::WaitForImage {
                image: image.clone(),
                tolerance: tolerance.clone(),
                appear: *appear,
                timeout: timeout.clone(),
            },
        ),
        InstructionDto::WhenTyped { id, text, erase } => (id, InstructionKind::WhenTyped { text: text.clone(), erase: *erase }),
        InstructionDto::PasteText { id, text } => (id, InstructionKind::PasteText(text.clone())),
        InstructionDto::WhenApp { id, pattern, started } => {
            (id, InstructionKind::WhenApp { pattern: pattern.clone(), started: *started })
        }
        InstructionDto::WhenIdle { id, minutes } => (id, InstructionKind::WhenIdle(minutes.clone())),
        InstructionDto::WhenScreenLock { id, locked } => (id, InstructionKind::WhenScreenLock { locked: *locked }),
        InstructionDto::WithLock { id, name, body } => (
            id,
            InstructionKind::WithLock {
                name: name.clone(),
                body: body.iter().map(dto_to_instruction).collect::<Option<Vec<_>>>()?,
            },
        ),
        InstructionDto::WhenCron { id, expression } => (id, InstructionKind::WhenCron(expression.clone())),
        InstructionDto::SplitIntoList { id, text, separator, name } => (
            id,
            InstructionKind::SplitIntoList { text: text.clone(), separator: separator.clone(), name: name.clone() },
        ),
        InstructionDto::WhenNetwork { id, connected } => (id, InstructionKind::WhenNetwork { connected: *connected }),
        InstructionDto::WhenUsb { id, pattern, connected } => {
            (id, InstructionKind::WhenUsb { pattern: pattern.clone(), connected: *connected })
        }
        InstructionDto::WhenMouseButton { id, button, pressed, modifiers } => {
            let index = get_mouse_button_names().iter().position(|&n| n == button.as_str())?;
            (
                id,
                InstructionKind::WhenMouseButton {
                    button: index_to_mouse_button(index),
                    pressed: *pressed,
                    modifiers: modifiers.clone(),
                },
            )
        }
        InstructionDto::AskText { id, prompt, default, name } => {
            (id, InstructionKind::AskText { prompt: prompt.clone(), default: default.clone(), name: name.clone() })
        }
        InstructionDto::AskYesNo { id, prompt, name } => (id, InstructionKind::AskYesNo { prompt: prompt.clone(), name: name.clone() }),
        InstructionDto::AskChoice { id, prompt, list, name } => {
            (id, InstructionKind::AskChoice { prompt: prompt.clone(), list: list.clone(), name: name.clone() })
        }
        InstructionDto::ShowMessage { id, message } => (id, InstructionKind::ShowMessage(message.clone())),
        InstructionDto::WhenAt { id, at } => (id, InstructionKind::WhenAt(at.clone())),
        InstructionDto::WhenSignal { id, name } => (id, InstructionKind::WhenSignal(name.clone())),
        InstructionDto::SendSignal { id, name, wait } => (id, InstructionKind::SendSignal { name: name.clone(), wait: *wait }),
        InstructionDto::WhenWebhook { id, path, variable } => {
            (id, InstructionKind::WhenWebhook { path: path.clone(), variable: variable.clone() })
        }
        InstructionDto::DetectObjectsInWindow { id, pattern, by, detector, label, confidence, sort, results_list, count_variable, region } => (
            id,
            InstructionKind::DetectObjectsInWindow {
                region: region.clone(),
                pattern: pattern.clone(),
                by: by.clone(),
                detector: detector.clone(),
                label: label.clone(),
                confidence: confidence.clone(),
                sort: sort.clone(),
                results_list: results_list.clone(),
                count_variable: count_variable.clone(),
            },
        ),
        InstructionDto::WaitForDetection { id, pattern, by, detector, label, confidence, appear, timeout, region } => (
            id,
            InstructionKind::WaitForDetection {
                region: region.clone(),
                pattern: pattern.clone(),
                by: by.clone(),
                detector: detector.clone(),
                label: label.clone(),
                confidence: confidence.clone(),
                appear: *appear,
                timeout: timeout.clone(),
            },
        ),
        InstructionDto::AimStep { id, pattern, by, target_x, target_y, gain, max_step, deadzone } => (
            id,
            InstructionKind::AimStep {
                pattern: pattern.clone(),
                by: by.clone(),
                target_x: target_x.clone(),
                target_y: target_y.clone(),
                gain: gain.clone(),
                max_step: max_step.clone(),
                deadzone: deadzone.clone(),
            },
        ),
        InstructionDto::AimAtDetection { id, results_list, index, gain, max_step, deadzone, timeout, settle_ms, status_variable } => (
            id,
            InstructionKind::AimAtDetection {
                results_list: results_list.clone(),
                index: index.clone(),
                gain: gain.clone(),
                max_step: max_step.clone(),
                deadzone: deadzone.clone(),
                timeout: timeout.clone(),
                settle_ms: settle_ms.clone(),
                status_variable: status_variable.clone(),
            },
        ),
    };
    Some(Instruction {
        id: id.clone(),
        kind,
    })
}

fn strand_to_dto(strand: &Strand) -> StrandDto {
    StrandDto {
        id: strand.id.clone(),
        x: strand.x,
        y: strand.y,
        instructions: strand.instructions.iter().map(instruction_to_dto).collect(),
    }
}

fn macro_to_dto(mac: &Macro) -> MacroDto {
    MacroDto {
        id: mac.id.clone(),
        name: mac.name.clone(),
        description: mac.description.clone(),
        strands: mac.strands.iter().map(strand_to_dto).collect(),
        recording_target_strand_id: mac.recording_target_id(),
        speed_multiplier: mac.speed_multiplier,
        floating_values: mac.floating_values.clone(),
        comments: mac.comments.clone(),
        variables: mac.variables.iter().map(|v| v.name.clone()).collect(),
        lists: mac.lists.iter().map(list_to_dto).collect(),
        block_defs: mac.block_defs.clone(),
        settings: macro_settings_to_dto(&mac.settings),
    }
}

fn hotkey_action_to_dto(action: &HotkeyAction) -> HotkeyActionDto {
    match action {
        HotkeyAction::RunMacro => HotkeyActionDto::RunMacro,
        HotkeyAction::StopLoop => HotkeyActionDto::StopLoop,
        HotkeyAction::EmergencyStop => HotkeyActionDto::EmergencyStop,
        HotkeyAction::NextMacro => HotkeyActionDto::NextMacro,
        HotkeyAction::PrevMacro => HotkeyActionDto::PrevMacro,
        HotkeyAction::StartRecordingImmediate => HotkeyActionDto::StartRecordingImmediate,
        HotkeyAction::StopRecording => HotkeyActionDto::StopRecording,
        HotkeyAction::Undo => HotkeyActionDto::Undo,
        HotkeyAction::Redo => HotkeyActionDto::Redo,
        HotkeyAction::RunSpecificMacro(id) => HotkeyActionDto::RunSpecificMacro {
            macro_id: id.clone(),
        },
    }
}

pub(crate) fn dto_to_hotkey_action(dto: &HotkeyActionDto) -> HotkeyAction {
    match dto {
        HotkeyActionDto::RunMacro => HotkeyAction::RunMacro,
        HotkeyActionDto::StopLoop => HotkeyAction::StopLoop,
        HotkeyActionDto::EmergencyStop => HotkeyAction::EmergencyStop,
        HotkeyActionDto::NextMacro => HotkeyAction::NextMacro,
        HotkeyActionDto::PrevMacro => HotkeyAction::PrevMacro,
        HotkeyActionDto::StartRecordingImmediate => HotkeyAction::StartRecordingImmediate,
        HotkeyActionDto::StopRecording => HotkeyAction::StopRecording,
        HotkeyActionDto::Undo => HotkeyAction::Undo,
        HotkeyActionDto::Redo => HotkeyAction::Redo,
        HotkeyActionDto::RunSpecificMacro { macro_id } => {
            HotkeyAction::RunSpecificMacro(macro_id.clone())
        }
    }
}

pub(crate) fn build_state_dto(s: &AppState) -> StateDto {
    let current_macro = s.current_macro.as_ref().map(macro_to_dto);

    let recording_phase = match &s.recording_phase {
        RecordingPhase::Idle => RecordingPhaseDto {
            phase: "Idle".to_string(),
            countdown: None,
        },
        RecordingPhase::Countdown(n) => RecordingPhaseDto {
            phase: "Countdown".to_string(),
            countdown: Some(*n),
        },
        RecordingPhase::Active => RecordingPhaseDto {
            phase: "Active".to_string(),
            countdown: None,
        },
    };

    let combo_capture = s.combo_capture.as_ref().map(|cc| match cc {
        ComboCapture::Named(action) => ComboCaptureDto {
            kind: "Named".to_string(),
            action: Some(hotkey_action_to_dto(action)),
            binding_index: None,
        },
        ComboCapture::Binding(index) => ComboCaptureDto {
            kind: "Binding".to_string(),
            action: None,
            binding_index: Some(*index),
        },
        ComboCapture::Pending => ComboCaptureDto {
            kind: "Pending".to_string(),
            action: None,
            binding_index: None,
        },
    });

    let macros_data: Vec<MacroDto> = s.macros_list.iter().map(macro_to_dto).collect();

    let macros_list = &s.macros_list;
    let hotkey_bindings: Vec<HotkeyBindingDto> = s
        .hotkey_bindings
        .iter()
        .enumerate()
        .map(|(i, b)| {
            let macro_name = if let HotkeyAction::RunSpecificMacro(ref id) = b.action {
                macros_list
                    .iter()
                    .find(|m| &m.id == id)
                    .map(|m| m.name.clone())
                    .or_else(|| Some("(deleted)".to_string()))
            } else {
                None
            };
            HotkeyBindingDto {
                binding_index: i,
                action: hotkey_action_to_dto(&b.action),
                combo_display: b.combo.format(),
                macro_name,
            }
        })
        .collect();

    const NAMED_HOTKEY_ACTIONS: [HotkeyAction; 9] = [
        HotkeyAction::RunMacro,
        HotkeyAction::StopLoop,
        HotkeyAction::EmergencyStop,
        HotkeyAction::NextMacro,
        HotkeyAction::PrevMacro,
        HotkeyAction::StartRecordingImmediate,
        HotkeyAction::StopRecording,
        HotkeyAction::Undo,
        HotkeyAction::Redo,
    ];
    let named_hotkey_defaults: Vec<NamedHotkeyDefaultDto> = NAMED_HOTKEY_ACTIONS
        .iter()
        .map(|action| NamedHotkeyDefaultDto {
            action: hotkey_action_to_dto(action),
            combo_display: blockwork_core::config::default_combo_for_action(action)
                .map(|c| c.format()),
        })
        .collect();

    let pending_macro_hotkey =
        s.pending_macro_hotkey
            .as_ref()
            .map(|(idx, combo)| PendingMacroHotkeyDto {
                macro_index: *idx,
                combo_display: combo.as_ref().map(|c| c.format()),
            });

    let invalid_field_buffers: Vec<InvalidFieldDto> = s
        .invalid_field_buffers
        .iter()
        .map(|(location, text)| InvalidFieldDto {
            location: location.clone(),
            text: text.clone(),
        })
        .collect();

    let key_capture = s.key_capture.as_ref().map(|target| match target {
        KeyCaptureTarget::Strand(strand_id, index) => KeyCaptureDto {
            kind: "Strand".to_string(),
            strand_id: Some(strand_id.clone()),
            index: Some(index.clone()),
        },
        KeyCaptureTarget::Standalone => KeyCaptureDto {
            kind: "Standalone".to_string(),
            strand_id: None,
            index: None,
        },
    });

    let is_looping = s.is_looping.lock().map(|g| *g).unwrap_or(false);

    let update_check_state = match &s.update_check_state {
        UpdateCheckState::Idle => UpdateCheckStateDto {
            state: "Idle".to_string(),
            version: None,
            error: None,
        },
        UpdateCheckState::Checking => UpdateCheckStateDto {
            state: "Checking".to_string(),
            version: None,
            error: None,
        },
        UpdateCheckState::UpToDate => UpdateCheckStateDto {
            state: "UpToDate".to_string(),
            version: None,
            error: None,
        },
        UpdateCheckState::UpdateAvailable(v) => UpdateCheckStateDto {
            state: "UpdateAvailable".to_string(),
            version: Some(v.clone()),
            error: None,
        },
        UpdateCheckState::Applying => UpdateCheckStateDto {
            state: "Applying".to_string(),
            version: None,
            error: None,
        },
        UpdateCheckState::Error(e) => UpdateCheckStateDto {
            state: "Error".to_string(),
            version: None,
            error: Some(e.clone()),
        },
    };

    StateDto {
        macro_names: s.macro_strs.clone(),
        macro_selected: s.macro_selected,
        current_macro,
        macros_data,
        global_speed_multiplier: s.global_speed_multiplier,
        is_looping,
        ipc_active_port: s.ipc_active_port,
        ipc_auto_start: s.ipc_auto_start,
        close_to_tray: s.close_to_tray,
        start_at_startup: s.start_at_startup,
        onboarding_done: s.onboarding_done,
        palette_layout: s.palette_layout.clone(),
        confirm_clear_instructions: s.confirm_clear_instructions,
        confirm_clear_instructions_remaining_secs: s.clear_confirm_remaining_secs,
        key_capture,
        standalone_key: s.pending_standalone_key.clone(),
        can_undo: s.history.can_undo(),
        can_redo: s.history.can_redo(),
        recording_phase,
        record_mouse_relative: s.record_mouse_relative,
        record_mouse_movement: s.record_mouse_movement,
        absolute_mouse_position_available: s.absolute_mouse_position_available,
        wayland_session: s.wayland_session,
        page: match s.page {
            Page::Main => "Main".to_string(),
            Page::Settings => "Settings".to_string(),
        },
        combo_capture,
        hotkey_bindings,
        named_hotkey_defaults,
        pending_macro_hotkey,
        invalid_field_buffers,
        ipc_port_text: s.ipc_port_text.clone(),
        ipc_port_invalid: s.ipc_port_invalid,
        emulator_available: !blockwork_core::macros::runner::emulator_failed(),
        grab_available: !blockwork_core::recording::grab_failed(),
        razer_permission_warning: crate::razer::permission_warning(),
        update_check_state,
        triggers_paused: s.triggers_paused,
        run_log: blockwork_core::run_log::snapshot(),
        import_review: s.pending_import.as_ref().map(|(_, review)| review.clone()),
        globals: blockwork_core::globals::all()
            .into_iter()
            .map(|(name, value)| GlobalDto { name, value: blockwork_core::secrets::redact(&value.as_text()) })
            .collect(),
        templates: crate::templates::list(),
        secret_names: blockwork_core::secrets::names(),
        library: s.macros_list.iter().map(library_entry).collect(),
        breakpoints: s.current_macro.as_ref().map(|m| blockwork_core::macros::debug::breakpoints(&m.id)).unwrap_or_default(),
        paused_macros: s
            .macros_list
            .iter()
            .filter(|m| blockwork_core::macros::debug::is_paused(&m.id))
            .map(|m| m.id.clone())
            .collect(),
        paused_strands: blockwork_core::macros::debug::paused_strands(),
        permissions: s.current_macro.as_ref().and_then(|m| blockwork_core::permissions::grants(&m.id)).map(|g| g.allowed),
        webhook: crate::webhook::dto(),
        block_library: crate::block_library::dto(s),
        pack_review: s.pending_pack.as_ref().map(|p| p.review.clone()),
        language: crate::ui_language(),
        animation_speed: crate::animation_speed(),
    }
}

fn library_entry(mac: &Macro) -> LibraryEntryDto {
    use blockwork_core::macros::BlockKind;
    LibraryEntryDto {
        id: mac.id.clone(),
        name: mac.name.clone(),
        folder: mac.settings.folder.clone(),
        always_listen: mac.settings.always_listen,
        has_triggers: mac.strands.iter().any(|st| {
            st.instructions.first().is_some_and(|i| {
                i.kind.is_header()
                    && !matches!(i.kind, InstructionKind::WhenRan | InstructionKind::BlockHeader(_) | InstructionKind::WhenSignal(_))
            })
        }),
        restricted: blockwork_core::permissions::grants(&mac.id).is_some(),
    }
}

pub(crate) fn emit_state_updated(app: &AppHandle, s: &AppState) {
    blockwork_core::recording::set_hotkeys_suspended(s.combo_capture.is_some());
    let dto = build_state_dto(s);
    app.emit_state(&dto);
}

#[cfg(test)]
mod tests {
    use super::*;
    use blockwork_core::macros::Instruction;

    #[test]
    fn detection_blocks_round_trip_through_dtos() {
        let (n, t) = (Value::number, Value::text);
        let kinds = [
            InstructionKind::DetectObjectsInWindow {
                pattern: t("Minecraft"),
                by: "Title".into(),
                detector: t("http://127.0.0.1:8765/detect"),
                label: t("player"),
                confidence: n(50.0),
                sort: "Closest".into(),
                results_list: "players".into(),
                count_variable: "count".into(),
                region: "10,20,300,200".into(),
            },
            InstructionKind::WaitForDetection {
                pattern: t("Minecraft"),
                by: "App".into(),
                detector: t("http://localhost:1/x"),
                label: t(""),
                confidence: n(40.0),
                appear: false,
                timeout: n(500.0),
                region: String::new(),
            },
            InstructionKind::AimStep {
                pattern: t(""),
                by: "Title".into(),
                target_x: n(10.0),
                target_y: n(20.0),
                gain: n(0.4),
                max_step: n(40.0),
                deadzone: n(6.0),
            },
            InstructionKind::AimAtDetection {
                results_list: "players".into(),
                index: n(1.0),
                gain: n(0.4),
                max_step: n(40.0),
                deadzone: n(6.0),
                timeout: n(3000.0),
                settle_ms: n(150.0),
                status_variable: "status".into(),
            },
        ];
        for kind in kinds {
            let dto = instruction_to_dto(&Instruction::new(kind.clone()));
            // Through JSON too, the way the frontend sends it back.
            let dto: InstructionDto = serde_json::from_str(&serde_json::to_string(&dto).unwrap()).unwrap();
            assert_eq!(dto_to_instruction(&dto).map(|i| i.kind), Some(kind));
        }
    }
}
