use blockwork_core::hotkey_types::{HotkeyAction, HotkeyBinding, KeyCombo};
use blockwork_core::macros::MacroGraph;
use blockwork_core::macros::backend::InputBackend;
use blockwork_core::macros::thread_pool::ThreadPool;
// Canvas addressing and undo come from blockstitch. Both these groups are
// re-exported so the rest of the crate still reaches them via `crate::state`.
pub(crate) use blockstitch_core::editor::{
    EditSession, History, InstrPath, PathStep, ValueBuffers, ValueLocation,
};
pub(crate) use blockwork_core::input::value::Evaluated;
pub(crate) use blockwork_core::macros::{BlockPiece, InstructionKind, Macro};
pub(crate) use blockstitch_core::graph::ListItem;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::AppHandle;
use std::sync::{Arc, Mutex};

pub(crate) type EditorSnapshot = (MacroGraph, Vec<String>, Vec<blockwork_core::block_packs::PackRecord>);

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
    pub(crate) history: History<EditorSnapshot>,
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
    pub(crate) recording_countdown_secs: u8,
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
    /// "12h", "24h", or empty to follow the system locale.
    pub(crate) clock_format: String,
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

// The canvas's wire shape is shared with the website's editor.
pub(crate) use blockwork_core::frontend::{
    FragmentDto, InstructionDto, InvalidFieldDto, KeyCaptureDto, ListItemDto, MacroDto,
    dto_to_fragment, dto_to_instruction, dto_to_list_item, fragment_to_dto, macro_to_dto,
};

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
pub(crate) struct UpdateCheckStateDto {
    pub(crate) state: String,
    pub(crate) version: Option<String>,
    pub(crate) error: Option<String>,
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
        recording_countdown_secs: crate::recording_countdown_secs(),
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
        clock_format: blockwork_core::config::load_settings().clock_format.unwrap_or_default(),
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
    use blockwork_core::frontend::instruction_to_dto;
    use blockwork_core::input::value::Value;
    use blockwork_core::macros::{Comment, Instruction};

    #[test]
    fn community_definitions_are_read_only_and_bodies_are_not_sent_to_editor() {
        use blockwork_core::block_packs::{self, PackInfo};
        use blockwork_core::macros::BlockShape;
        let mut author = Macro::new("Author".into(), String::new(), vec![]);
        let id = author.create_block(vec![], BlockShape::Normal, "#4C97FF".into(), 900, 400);
        author.graph.block_def_mut(&id).unwrap().identifier = "Wait".into();
        author.graph.block_def_mut(&id).unwrap().description = "Waits for an interval.".into();
        let body = author.block_body(&id).unwrap().clone();
        let note = Comment::new(0, 0, "Hidden implementation note".into(), Some(body.instructions[0].id.clone()));
        author.graph.comments.push(note);
        let pack = block_packs::pack_from_macro(&author, &[id.clone()], PackInfo {
            name: "Timing".into(), author: "Alice".into(), ..Default::default()
        }).unwrap();
        let mut mac = Macro::new("User".into(), String::new(), vec![]);
        block_packs::merge_into(&mut mac, &pack, "");
        let dto = macro_to_dto(&mac);
        assert_eq!(dto.strands.len(), 1);
        assert!(dto.comments.is_empty());
        assert!(dto.block_defs[0].read_only);
        assert_eq!(dto.block_defs[0].definition.identifier, "Alice_Wait");
        assert_eq!(dto.block_defs[0].definition.description, "Waits for an interval.");
        let json = serde_json::to_value(&dto).unwrap();
        assert_eq!(json["block_defs"][0]["id"], id);
        assert_eq!(json["block_defs"][0]["read_only"], true);
        assert_eq!(json["block_defs"][0]["packs"][0]["id"], "timing");
        assert_eq!(json["block_defs"][0]["packs"][0]["name"], "Timing");
        assert!(mac.block_body(&id).is_some());
        assert!(!serde_json::to_string(&dto).unwrap().contains("Hidden implementation note"));
    }

    #[test]
    fn signals_round_trip_through_frontend_without_losing_expression_mode() {
        use blockwork_core::macros::SignalName;
        for expression in [false, true] {
            let target = SignalName::new(Value::text("finished"), expression);
            for kind in [
                InstructionKind::WhenSignal(target.clone()),
                InstructionKind::SendSignal {
                    name: target.clone(),
                    wait: true,
                },
            ] {
                let instruction = Instruction::new(kind.clone());
                let dto = instruction_to_dto(&instruction);
                let json = serde_json::to_value(&dto).unwrap();
                assert_eq!(json["expression"], expression);
                let dto = serde_json::from_value(json).unwrap();
                assert_eq!(dto_to_instruction(&dto).unwrap().kind, kind);
            }
        }
    }

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
