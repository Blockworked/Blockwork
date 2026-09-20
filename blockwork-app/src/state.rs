use blockwork_core::hotkey_types::{HotkeyAction, HotkeyBinding, KeyCombo};
use blockwork_core::input::schedule::TimeSchedule;
use blockwork_core::input::types::{Axis, Coordinate, Direction, InputToken};
use blockwork_core::input::{get_mouse_button_names, key_to_string, mouse_button_to_index};
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
    BlockDef, BlockPiece, Comment, FloatingValue, Instruction, InstructionKind, Macro,
    MacroSettings, Strand,
};
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
    pub(crate) thread_pool: ThreadPool,
    pub(crate) is_looping: Arc<Mutex<bool>>,
    pub(crate) loop_mode_enabled: bool,
    pub(crate) global_speed_multiplier: f64,
    pub(crate) ipc_server: Option<tokio::task::JoinHandle<()>>,
    pub(crate) ipc_shutdown_tx: Option<tokio::sync::watch::Sender<bool>>,
    pub(crate) ipc_active_port: Option<u16>,
    pub(crate) ipc_auto_start: bool,
    pub(crate) close_to_tray: bool,
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
    /// A macro file the user just picked via `import_macro` but whose import
    /// is paused on the "contains a Command instruction" warning popup -
    /// `confirm_import_macro`/`cancel_import_macro` resolve it. `None` the
    /// rest of the time (including immediately after a no-warning-needed
    /// import, which commits inline instead of staging here).
    pub(crate) pending_import: Option<Macro>,
}

// ─── Serializable DTO ──────────────────────────────────────────────────────

#[derive(Serialize, Clone)]
pub(crate) struct StateDto {
    pub(crate) macro_names: Vec<String>,
    pub(crate) macro_selected: Option<usize>,
    pub(crate) current_macro: Option<MacroDto>,
    pub(crate) macros_data: Vec<MacroDto>,
    pub(crate) loop_mode_enabled: bool,
    pub(crate) global_speed_multiplier: f64,
    pub(crate) is_looping: bool,
    pub(crate) ipc_active_port: Option<u16>,
    pub(crate) ipc_auto_start: bool,
    pub(crate) close_to_tray: bool,
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
    /// User-defined custom blocks ("My Blocks").
    pub(crate) block_defs: Vec<BlockDef>,
    /// Settings edited from the "Macro Settings" popup - see `MacroSettingsDto`.
    pub(crate) settings: MacroSettingsDto,
}

/// Wire shape for `MacroSettings` - see there for field meanings.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub(crate) struct MacroSettingsDto {
    pub(crate) always_listen: bool,
}

fn macro_settings_to_dto(settings: &MacroSettings) -> MacroSettingsDto {
    MacroSettingsDto {
        always_listen: settings.always_listen,
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

/// What `import_macro` needs the user to resolve before the staged
/// `pending_import` macro can be committed.
#[derive(Serialize, Clone)]
pub(crate) struct ImportPromptDto {
    pub(crate) needs_command_warning: bool,
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
        key: String,
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
    BlockHeader {
        id: String,
        block_id: String,
    },
    CallBlock {
        id: String,
        block_id: String,
        args: Vec<Value>,
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
    NextMacro,
    PrevMacro,
    ToggleLoop,
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
        InstructionKind::BlockHeader(block_id) => InstructionDto::BlockHeader {
            id,
            block_id: block_id.clone(),
        },
        InstructionKind::CallBlock { block_id, args } => InstructionDto::CallBlock {
            id,
            block_id: block_id.clone(),
            args: args.clone(),
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
        InstructionKind::Token(token) => match token {
            InputToken::Text(t) => InstructionDto::Text {
                id,
                text: t.clone(),
            },
            InputToken::Key(k, d) => InstructionDto::Key {
                id,
                key: key_to_string(k).unwrap_or("Unknown").to_string(),
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
    use blockwork_core::input::{index_to_mouse_button, key_names::string_to_key};
    let (id, kind) = match dto {
        InstructionDto::Wait { id, duration } => {
            (id, InstructionKind::Wait(duration.clone()))
        }
        InstructionDto::Text { id, text } => (
            id,
            InstructionKind::Token(InputToken::Text(text.clone())),
        ),
        InstructionDto::Key { id, key, direction } => {
            let mk = string_to_key(key).ok()?;
            (
                id,
                InstructionKind::Token(InputToken::Key(mk, str_to_direction(direction))),
            )
        }
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
        InstructionDto::BlockHeader { id, block_id } => {
            (id, InstructionKind::BlockHeader(block_id.clone()))
        }
        InstructionDto::CallBlock { id, block_id, args } => (
            id,
            InstructionKind::CallBlock {
                block_id: block_id.clone(),
                args: args.clone(),
            },
        ),
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
        block_defs: mac.block_defs.clone(),
        settings: macro_settings_to_dto(&mac.settings),
    }
}

fn hotkey_action_to_dto(action: &HotkeyAction) -> HotkeyActionDto {
    match action {
        HotkeyAction::RunMacro => HotkeyActionDto::RunMacro,
        HotkeyAction::StopLoop => HotkeyActionDto::StopLoop,
        HotkeyAction::NextMacro => HotkeyActionDto::NextMacro,
        HotkeyAction::PrevMacro => HotkeyActionDto::PrevMacro,
        HotkeyAction::ToggleLoop => HotkeyActionDto::ToggleLoop,
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
        HotkeyActionDto::NextMacro => HotkeyAction::NextMacro,
        HotkeyActionDto::PrevMacro => HotkeyAction::PrevMacro,
        HotkeyActionDto::ToggleLoop => HotkeyAction::ToggleLoop,
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
        },
        ComboCapture::Pending => ComboCaptureDto {
            kind: "Pending".to_string(),
            action: None,
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
        HotkeyAction::NextMacro,
        HotkeyAction::PrevMacro,
        HotkeyAction::ToggleLoop,
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
        loop_mode_enabled: s.loop_mode_enabled,
        global_speed_multiplier: s.global_speed_multiplier,
        is_looping,
        ipc_active_port: s.ipc_active_port,
        ipc_auto_start: s.ipc_auto_start,
        close_to_tray: s.close_to_tray,
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
        emulator_available: s.emulator.is_some(),
        grab_available: !blockwork_core::recording::grab_failed(),
        razer_permission_warning: crate::razer::permission_warning(),
        update_check_state,
    }
}

pub(crate) fn emit_state_updated(app: &AppHandle, s: &AppState) {
    let dto = build_state_dto(s);
    app.emit_state(&dto);
}
