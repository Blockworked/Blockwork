use crate::macros_thread;
use crate::state::{
    AppState, ComboCapture, EditSession, HotkeyActionDto, InstructionDto, KeyCaptureTarget,
    ListItemDto, Page, PathStep, RecordingPhase, SharedState, StateDto, UpdateCheckState,
    ValueLocation, build_state_dto, dto_to_hotkey_action, dto_to_instruction,
    emit_state_updated,
};
use blockstitch_core::editor::{
    ValueEdit, drop_strand_buffers, prune_value_buffers, retain_live_buffers,
};
use blockwork_core::config;
use blockwork_core::hotkey_types::{HotkeyAction, HotkeyBinding, KeyCombo};
use blockwork_core::input::types::InputToken;
use blockwork_core::input::value::{Evaluated, Value};
use blockwork_core::macros::runner::VariableStore;
use blockstitch_core::graph::{ListItem, ListStore, resolve_list_reporters};
use blockwork_core::macros::{
    BlockDef, BlockPiece, BlockShape, Instruction, InstructionKind, Macro,
    MacroGraph, SPEED_MULTIPLIER_RANGE, Strand, loop_control,
    normalize_block_color as normalize_persisted_block_color,
};
use blockwork_core::recording;
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use crate::AppHandle;
use tracing::warn;

const CLEAR_CONFIRM_TIMEOUT_SECS: u64 = 3;

/// Checkpoints the canvas before a structural edit, and ends the current
/// text-edit group so the next keystroke starts a fresh step.
fn push_undo(s: &mut AppState) {
    push_undo_for(s, None);
}

/// [`push_undo`] for an edit that coalesces with the keystrokes already in
/// progress at `session` - only the first one checkpoints.
fn push_undo_for(s: &mut AppState, session: Option<EditSession>) {
    let snapshot = s.current_macro.as_ref().map(|mac| mac.graph.clone());
    match (snapshot, session) {
        (Some(snapshot), Some(session)) => {
            s.history.push_for_session(snapshot, session);
        }
        (Some(snapshot), None) => s.history.push(snapshot),
        (None, _) => s.history.end_session(),
    }
}

/// Max wait for the libei permission prompt; a portal that never replies
/// would otherwise hang the request forever.
const LIBEI_REQUEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(25);

/// Starts the libei portal session without holding the app-state lock, which
/// would stall the daemon while the prompt is open.
///
/// Runs on `spawn_blocking`: the blocking portal call wedged every other
/// connection when run on an async worker. The emulator stays locked until
/// it returns, so the timeout only bounds this request, not playback.
pub(crate) async fn request_absolute_mouse_support(
    state: &SharedState,
    app: &AppHandle,
) -> Result<(), String> {
    let emulator = state
        .lock()
        .map_err(|e| e.to_string())?
        .emulator
        .clone()
        .ok_or_else(|| "The input backend is unavailable.".to_string())?;
    let task = tokio::task::spawn_blocking(move || {
        let mut emulator = emulator.lock().map_err(|e| e.to_string())?;
        emulator.ensure_absolute_mouse_support()
    });
    let result = match tokio::time::timeout(LIBEI_REQUEST_TIMEOUT, task).await {
        Ok(joined) => joined.map_err(|e| e.to_string())?,
        Err(_) => Err("timed out waiting for the permission dialog".to_string()),
    };
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.absolute_mouse_position_available = result.is_ok()
        && blockwork_core::macros::backend::absolute_mouse_position_available();
    emit_state_updated(app, &s);
    result
}

fn refresh_macro_list(s: &mut crate::state::AppState) {
    let macros = config::get_macros_from_config();
    s.macro_strs = macros.iter().map(|m| m.name.clone()).collect();
    // Keep current_macro in sync if it was modified
    if let Some(ref cur) = s.current_macro.clone() {
        if let Some(updated) = macros.iter().find(|m| m.id == cur.id) {
            s.current_macro = Some(updated.clone());
        }
    }
    // Update macro_selected index (name may have changed)
    if let Some(ref cur) = s.current_macro {
        s.macro_selected = macros.iter().position(|m| m.id == cur.id);
    }
    s.macros_list = macros;
}

/// Resyncs the live variable store to `current_macro`'s declared variables -
/// called after `current_macro` is (re)assigned, so stale entries don't linger.
fn sync_variable_values(s: &mut crate::state::AppState) {
    let values = match &s.current_macro {
        Some(mac) => mac
            .variables
            .iter()
            .map(|v| (v.name.clone(), v.value.clone()))
            .collect(),
        None => HashMap::new(),
    };
    if let Ok(mut store) = s.variable_values.lock() {
        *store = values;
    }
}

fn sync_list_values(s: &mut crate::state::AppState) {
    let values = match &s.current_macro {
        Some(mac) => mac
            .lists
            .iter()
            .map(|list| (list.name.clone(), list.items.clone()))
            .collect(),
        None => HashMap::new(),
    };
    if let Ok(mut store) = s.list_values.lock() {
        *store = values;
    }
}

fn auto_save(s: &crate::state::AppState) {
    if let Some(mac) = &s.current_macro {
        if let Err(e) = mac.save() {
            warn!("Failed to auto-save macro: {e}");
        } else {
            config::set_selected_macro_id(Some(&mac.id));
        }
    }
}

// ─── Read ──────────────────────────────────────────────────────────────────

pub(crate) fn get_state(state: &SharedState) -> Result<StateDto, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    Ok(build_state_dto(&s))
}

// ─── Macro library ─────────────────────────────────────────────────────────

pub(crate) fn select_macro(
    state: &SharedState,
    app: &AppHandle,
    index: usize,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if let Some(mac) = s.macros_list.get(index).cloned() {
        s.macro_selected = Some(index);
        s.current_macro = Some(mac.clone());
        config::set_selected_macro_id(Some(&mac.id));
    } else {
        s.macro_selected = None;
        s.current_macro = None;
        config::set_selected_macro_id(None);
    }
    s.history.clear();
    s.invalid_field_buffers.clear();
    sync_variable_values(&mut s);
    sync_list_values(&mut s);
    emit_state_updated(&app, &s);
    Ok(())
}

pub(crate) fn new_macro(
    state: &SharedState,
    app: &AppHandle,
) -> Result<(), String> {
    let new_macro = Macro::new("New Macro".into(), "".into(), vec![]);
    let new_id = new_macro.id.clone();
    if let Err(e) = new_macro.add() {
        return Err(format!("Failed to create macro: {e}"));
    }
    let mut s = state.lock().map_err(|e| e.to_string())?;
    refresh_macro_list(&mut s);
    // Select the newly created macro
    if let Some((idx, mac)) = s
        .macros_list
        .iter()
        .enumerate()
        .find(|(_, m)| m.id == new_id)
        .map(|(i, m)| (i, m.clone()))
    {
        s.macro_selected = Some(idx);
        s.current_macro = Some(mac);
        config::set_selected_macro_id(Some(&new_id));
    }
    s.invalid_field_buffers.clear();
    sync_variable_values(&mut s);
    sync_list_values(&mut s);
    emit_state_updated(&app, &s);
    Ok(())
}

/// Deletes the current macro. The frontend confirms with the user via a
/// popup (`RemoveMacroDialog.vue`) before ever calling this, so it deletes
/// unconditionally.
pub(crate) fn remove_macro(
    state: &SharedState,
    app: &AppHandle,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if let Some(mac) = s.current_macro.take() {
        if let Err(e) = mac.remove() {
            warn!("Failed to remove macro: {e}");
        }
    }
    s.macro_selected = None;
    s.invalid_field_buffers.clear();
    refresh_macro_list(&mut s);
    config::set_selected_macro_id(None);
    emit_state_updated(&app, &s);
    Ok(())
}

pub(crate) fn set_title(
    state: &SharedState,
    app: &AppHandle,
    title: String,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if let Some(mac) = &mut s.current_macro {
        mac.name = title;
        auto_save(&s);
        // Rebuild list so dropdown reflects the new name
        let macros = config::get_macros_from_config();
        s.macro_strs = macros.iter().map(|m| m.name.clone()).collect();
        s.macros_list = macros;
    }
    emit_state_updated(&app, &s);
    Ok(())
}

pub(crate) fn set_macro_speed_multiplier(
    state: &SharedState,
    app: &AppHandle,
    multiplier: f64,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if let Some(mac) = &mut s.current_macro {
        mac.speed_multiplier = multiplier.clamp(
            *SPEED_MULTIPLIER_RANGE.start(),
            *SPEED_MULTIPLIER_RANGE.end(),
        );
        auto_save(&s);
    }
    emit_state_updated(&app, &s);
    Ok(())
}

/// Sets the current macro's "always listen for events even when a different
/// macro is selected" setting - see `MacroSettings::always_listen`. Edited
/// from the "Macro Settings" popup next to the macro dropdown.
pub(crate) fn set_macro_always_listen(
    state: &SharedState,
    app: &AppHandle,
    enabled: bool,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if let Some(mac) = &mut s.current_macro {
        mac.settings.always_listen = enabled;
        auto_save(&s);
        refresh_macro_list(&mut s);
    }
    emit_state_updated(&app, &s);
    Ok(())
}

/// Declares a new macro-wide variable, starting at `0`. No `push_undo` -
/// a naming/creation action, not an undoable structural edit.
pub(crate) fn create_variable(
    state: &SharedState,
    app: &AppHandle,
    name: String,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    let mac = s.current_macro.as_mut().ok_or("No macro selected")?;
    let trimmed = mac.create_variable(&name)?;
    if let Ok(mut store) = s.variable_values.lock() {
        store.insert(trimmed, Evaluated::Number(0.0));
    }
    auto_save(&s);
    emit_state_updated(&app, &s);
    Ok(())
}

/// Renames a declared variable and every reference to it.
pub(crate) fn rename_variable(
    state: &SharedState,
    app: &AppHandle,
    old_name: String,
    new_name: String,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    // Rename on a copy first, so a rejected or no-op rename doesn't add a
    // useless undo entry.
    let mut graph = s
        .current_macro
        .as_ref()
        .map(|mac| mac.graph.clone())
        .ok_or("No macro selected")?;
    let trimmed = graph.rename_variable(&old_name, &new_name)?;
    if trimmed == old_name {
        return Ok(());
    }
    push_undo(&mut s);
    if let Some(mac) = &mut s.current_macro {
        mac.graph = graph;
    }
    if let Ok(mut store) = s.variable_values.lock() {
        if let Some(v) = store.remove(&old_name) {
            store.insert(trimmed, v);
        }
    }
    auto_save(&s);
    emit_state_updated(&app, &s);
    Ok(())
}

/// Deletes a declared variable. No `push_undo` - same precedent as
/// `create_variable`.
pub(crate) fn delete_variable(
    state: &SharedState,
    app: &AppHandle,
    name: String,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    let mac = s.current_macro.as_mut().ok_or("No macro selected")?;
    mac.remove_variable(&name);
    if let Ok(mut store) = s.variable_values.lock() {
        store.remove(&name);
    }
    auto_save(&s);
    emit_state_updated(&app, &s);
    Ok(())
}

fn create_list_in(mac: &mut Macro, name: &str) -> Result<String, String> {
    let trimmed = mac.graph.create_list(name)?;
    if let Some(list) = mac.graph.lists.iter_mut().find(|list| list.name == trimmed) {
        list.editor_x = 36;
        list.editor_y = 36;
    }
    Ok(trimmed)
}

pub(crate) fn create_list(
    state: &SharedState,
    app: &AppHandle,
    name: String,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    // Validate before checkpointing so a rejected name never creates a dead
    // undo entry.
    {
        let mac = s.current_macro.as_ref().ok_or("No macro selected")?;
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err("List name can't be empty".to_string());
        }
        if mac.lists.iter().any(|list| list.name == trimmed) {
            return Err(format!("A list named \"{trimmed}\" already exists"));
        }
    }
    push_undo(&mut s);
    create_list_in(s.current_macro.as_mut().ok_or("No macro selected")?, &name)?;
    sync_list_values(&mut s);
    auto_save(&s);
    emit_state_updated(&app, &s);
    Ok(())
}

pub(crate) fn rename_list(
    state: &SharedState,
    app: &AppHandle,
    old_name: String,
    new_name: String,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    let trimmed = new_name.trim().to_string();
    {
        let mac = s.current_macro.as_ref().ok_or("No macro selected")?;
        if trimmed.is_empty() {
            return Err("List name can't be empty".to_string());
        }
        if trimmed != old_name && mac.lists.iter().any(|list| list.name == trimmed) {
            return Err(format!("A list named \"{trimmed}\" already exists"));
        }
        if !mac.lists.iter().any(|list| list.name == old_name) {
            return Err("List not found".to_string());
        }
    }
    if trimmed == old_name {
        return Ok(());
    }

    push_undo(&mut s);
    s.current_macro
        .as_mut()
        .ok_or("No macro selected")?
        .graph
        .rename_list(&old_name, &trimmed)?;
    sync_list_values(&mut s);
    auto_save(&s);
    emit_state_updated(&app, &s);
    Ok(())
}

pub(crate) fn delete_list(
    state: &SharedState,
    app: &AppHandle,
    name: String,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    let exists = s
        .current_macro
        .as_ref()
        .ok_or("No macro selected")?
        .lists
        .iter()
        .any(|list| list.name == name);
    if exists {
        push_undo(&mut s);
        s.current_macro
            .as_mut()
            .expect("checked above")
            .graph
            .remove_list(&name);
    }
    sync_list_values(&mut s);
    auto_save(&s);
    emit_state_updated(&app, &s);
    Ok(())
}

/// Replaces a list's visible-editor contents. `ListItemDto` intentionally has
/// no boolean/expression case, which enforces the literal-only list contract.
pub(crate) fn set_list_items(
    state: &SharedState,
    app: &AppHandle,
    name: String,
    items: Vec<ListItemDto>,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    let items: Vec<ListItem> = items.iter().map(crate::state::dto_to_list_item).collect();
    s.current_macro
        .as_mut()
        .ok_or("No macro selected")?
        .graph
        .set_list_items(&name, items)?;
    sync_list_values(&mut s);
    auto_save(&s);
    emit_state_updated(&app, &s);
    Ok(())
}

/// Saves whether a list's editable canvas monitor is open and where it sits.
/// This is a presentation preference rather than an undoable macro edit.
pub(crate) fn set_list_editor_state(
    state: &SharedState,
    app: &AppHandle,
    name: String,
    visible: bool,
    x: i32,
    y: i32,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.current_macro
        .as_mut()
        .ok_or("No macro selected")?
        .graph
        .set_list_editor_state(&name, visible, x, y)?;
    auto_save(&s);
    emit_state_updated(&app, &s);
    Ok(())
}

// ─── Custom blocks ("My Blocks") ────────────────────────────────────────────

/// Colors are stored as six-digit CSS hex values. Restricting the persisted
/// value keeps imported/user-created macros from injecting arbitrary CSS when
/// the frontend uses it as a custom property on a block row.
fn normalize_block_color(color: &str) -> Result<String, String> {
    normalize_persisted_block_color(color).ok_or_else(|| "Choose a valid block color".to_string())
}

/// Defines a new custom block, creating its (initially empty) header strand
/// next to the macro's other strands. Pushes undo, since it has real canvas
/// footprint.
pub(crate) fn create_block(
    state: &SharedState,
    app: &AppHandle,
    pieces: Vec<BlockPiece>,
    shape: BlockShape,
    color: String,
) -> Result<String, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    BlockDef::validate_pieces(&pieces)?;
    let color = normalize_block_color(&color)?;
    push_undo(&mut s);
    let mac = s.current_macro.as_mut().ok_or("No macro selected")?;
    let (x, y) = mac.next_strand_position();
    let id = mac.create_block(pieces, shape, color, x, y);
    auto_save(&s);
    emit_state_updated(&app, &s);
    Ok(id)
}

/// Updates a block's prototype/return-type, reconciling every call site's
/// `args` to the new input list and renaming body params that kept their
/// identity but changed name. Pushes undo.
pub(crate) fn edit_block(
    state: &SharedState,
    app: &AppHandle,
    block_id: String,
    pieces: Vec<BlockPiece>,
    shape: BlockShape,
    color: String,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    // `update_block` re-checks both; doing it first keeps a rejected edit
    // from leaving a useless undo step.
    BlockDef::validate_pieces(&pieces)?;
    normalize_block_color(&color)?;
    push_undo(&mut s);
    let mac = s.current_macro.as_mut().ok_or("No macro selected")?;
    if let Some(def) = mac.block_defs.iter().find(|b| b.id == block_id) {
        let old_pieces = def.pieces.clone();
        let renames: Vec<(String, String)> = pieces
            .iter()
            .filter_map(|new_piece| {
                let BlockPiece::Branch { id, name: new_name } = new_piece else {
                    return None;
                };
                let old_name = old_pieces.iter().find_map(|p| match p {
                    BlockPiece::Branch { id: old_id, name } if old_id == id => Some(name),
                    _ => None,
                })?;
                (old_name != new_name).then(|| (old_name.clone(), new_name.clone()))
            })
            .collect();
        for (old_name, new_name) in &renames {
            mac.rename_block_branch_body(&block_id, old_name, new_name);
        }
    }
    s.current_macro
        .as_mut()
        .ok_or("No macro selected")?
        .update_block(&block_id, pieces, shape, &color)?;
    auto_save(&s);
    emit_state_updated(&app, &s);
    Ok(())
}

/// Deletes a custom block entirely (see `Macro::remove_block`). Pushes
/// undo, same reasoning as `create_block`.
pub(crate) fn delete_block(
    state: &SharedState,
    app: &AppHandle,
    block_id: String,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    push_undo(&mut s);
    if let Some(mac) = &mut s.current_macro {
        mac.remove_block(&block_id);
        let surviving: Vec<String> = mac.strands.iter().map(|st| st.id.clone()).collect();
        retain_live_buffers(&mut s.invalid_field_buffers, &surviving);
        auto_save(&s);
    }
    emit_state_updated(&app, &s);
    Ok(())
}

pub(crate) fn save_macro(
    state: &SharedState,
    app: &AppHandle,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if let Some(mac) = &s.current_macro {
        if let Err(e) = mac.save() {
            return Err(format!("Failed to save macro: {e}"));
        }
    }
    refresh_macro_list(&mut s);
    emit_state_updated(&app, &s);
    Ok(())
}

/// Strips characters illegal in filenames on common platforms, so a macro's
/// freeform name can double as a sane default export filename.
fn sanitize_filename(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| {
            if matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') {
                '_'
            } else {
                c
            }
        })
        .collect();
    let trimmed = cleaned.trim();
    if trimmed.is_empty() {
        "macro".to_string()
    } else {
        trimmed.to_string()
    }
}

/// Default file name offered by the UI's save dialog when exporting a macro.
pub(crate) fn export_macro_file_name(macro_id: String) -> Result<String, String> {
    let mac = config::get_macros_from_config()
        .into_iter()
        .find(|m| m.id == macro_id)
        .ok_or("Macro not found")?;
    Ok(format!("{}.macro", sanitize_filename(&mac.name)))
}

/// Exports a single macro to a `.macro` file the user picked in the UI (same
/// JSON shape as the app's own on-disk storage, just under a different
/// extension so it reads as a portable macro file rather than an app-internal
/// one).
pub(crate) fn export_macro(macro_id: String, path: String) -> Result<(), String> {
    let macros = config::get_macros_from_config();
    let mac = macros
        .into_iter()
        .find(|m| m.id == macro_id)
        .ok_or("Macro not found")?;
    config::write_macro_file(std::path::Path::new(&path), &mac)
}

/// Whether `body` (or anything nested inside its `If`/`IfElse`/`Repeat`/
/// `Forever`/`While` blocks) contains a `Command` instruction - the check
/// behind `import_macro`'s "this macro can run arbitrary commands" warning.
fn body_contains_command(body: &[Instruction]) -> bool {
    body.iter().any(|ins| match &ins.kind {
        // `OpenApp`'s `command` is just as capable of running arbitrary
        // shell as a plain `Command` - a hand-edited macro file could carry
        // any string there, not just what the picker would produce.
        InstructionKind::Command(_) | InstructionKind::OpenApp { .. } => true,
        InstructionKind::If { body, .. }
        | InstructionKind::Repeat { body, .. }
        | InstructionKind::Forever { body }
        | InstructionKind::While { body, .. } => body_contains_command(body),
        InstructionKind::IfElse {
            then_body,
            else_body,
            ..
        } => body_contains_command(then_body) || body_contains_command(else_body),
        _ => false,
    })
}

fn macro_contains_command(mac: &Macro) -> bool {
    mac.strands
        .iter()
        .any(|strand| body_contains_command(&strand.instructions))
}

/// One (key, label, getter, setter) entry per `MacroSettings` field - the
/// single place a new setting needs registering to participate in the
/// "review custom settings on import" flow below. `key` is the wire
/// identifier the frontend's toggle list and `confirm_import_macro`'s
/// `keep_settings` map address it by; `label` is the human-readable text.
type SettingAccessor = (
    &'static str,
    &'static str,
    fn(&blockwork_core::macros::MacroSettings) -> bool,
    fn(&mut blockwork_core::macros::MacroSettings, bool),
);

const SETTING_ACCESSORS: &[SettingAccessor] = &[(
    "always_listen",
    "Always listen for events, even when a different macro is selected",
    |s| s.always_listen,
    |s, v| s.always_listen = v,
)];

/// Every `MacroSettings` field on `settings` that differs from its default -
/// shown to the user by `import_macro` for confirmation, since non-default
/// behavior deserves a second look before it's silently applied.
fn non_default_macro_settings(
    settings: &blockwork_core::macros::MacroSettings,
) -> Vec<crate::state::CustomMacroSettingDto> {
    let default = blockwork_core::macros::MacroSettings::default();
    SETTING_ACCESSORS
        .iter()
        .filter_map(|(key, label, get, _)| {
            let value = get(settings);
            (value != get(&default)).then(|| crate::state::CustomMacroSettingDto {
                key: key.to_string(),
                label: label.to_string(),
                enabled: value,
            })
        })
        .collect()
}

/// Resets every setting the user unchecked in the import review popup (i.e.
/// present in `keep` as `false`) back to its default; anything not mentioned
/// in `keep` is left as imported.
fn apply_setting_overrides(
    settings: &mut blockwork_core::macros::MacroSettings,
    keep: &HashMap<String, bool>,
) {
    let default = blockwork_core::macros::MacroSettings::default();
    for (key, _, get, set) in SETTING_ACCESSORS {
        if !keep.get(*key).copied().unwrap_or(true) {
            set(settings, get(&default));
        }
    }
}

/// Assigns a fresh id to `mac` (so importing never collides with or
/// overwrites an existing macro), saves it, and makes it the
/// selected/current macro.
fn commit_imported_macro(
    mut mac: Macro,
    state: &SharedState,
    app: &AppHandle,
) -> Result<(), String> {
    mac.id = uuid::Uuid::new_v4().simple().to_string();
    let new_id = mac.id.clone();
    mac.add()?;

    let mut s = state.lock().map_err(|e| e.to_string())?;
    refresh_macro_list(&mut s);
    if let Some((idx, found)) = s
        .macros_list
        .iter()
        .enumerate()
        .find(|(_, m)| m.id == new_id)
        .map(|(i, m)| (i, m.clone()))
    {
        s.macro_selected = Some(idx);
        s.current_macro = Some(found);
        config::set_selected_macro_id(Some(&new_id));
    }
    s.invalid_field_buffers.clear();
    sync_variable_values(&mut s);
    sync_list_values(&mut s);
    emit_state_updated(app, &s);
    Ok(())
}

/// Reads the `.macro` file the user picked in the UI. If it needs a confirmation prompt
/// before it can be committed - a `Command` instruction, and/or non-default
/// macro settings (see `non_default_macro_settings`) - the parsed macro is
/// staged in `pending_import` and the prompt to show is returned; the
/// frontend resolves it via `confirm_import_macro`/`cancel_import_macro`.
/// Otherwise the macro is imported immediately and `None` is returned.
pub(crate) fn import_macro(
    state: &SharedState,
    app: &AppHandle,
    path: String,
) -> Result<Option<crate::state::ImportPromptDto>, String> {
    let mac = config::read_macro_file(std::path::Path::new(&path))?;

    let needs_command_warning = macro_contains_command(&mac);
    let custom_settings = non_default_macro_settings(&mac.settings);

    if needs_command_warning || !custom_settings.is_empty() {
        let prompt = crate::state::ImportPromptDto {
            needs_command_warning,
            custom_settings,
        };
        let mut s = state.lock().map_err(|e| e.to_string())?;
        s.pending_import = Some(mac);
        return Ok(Some(prompt));
    }

    commit_imported_macro(mac, &state, &app)?;
    Ok(None)
}

/// Finishes an import staged by `import_macro` after the user resolves its
/// prompt. `keep_settings` maps each `ImportPromptDto::custom_settings`
/// entry's `key` to whether the user kept its imported value (`true`) or
/// reset it to default (`false`); a key the popup never showed is absent,
/// which `apply_setting_overrides` treats as "keep".
pub(crate) async fn confirm_import_macro(
    state: &SharedState,
    app: &AppHandle,
    keep_settings: HashMap<String, bool>,
) -> Result<(), String> {
    let mac = {
        state
            .lock()
            .map_err(|e| e.to_string())?
            .pending_import
            .take()
    };
    let Some(mut mac) = mac else {
        return Ok(());
    };
    apply_setting_overrides(&mut mac.settings, &keep_settings);
    commit_imported_macro(mac, &state, &app)
}

/// Discards an import staged by `import_macro` after the user declines the
/// Command warning popup.
pub(crate) fn cancel_import_macro(state: &SharedState) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.pending_import = None;
    Ok(())
}

// ─── Instructions ──────────────────────────────────────────────────────────

pub(crate) async fn add_instruction(
    state: &SharedState,
    app: &AppHandle,
    strand_id: String,
    path: Vec<PathStep>,
    instruction: InstructionDto,
) -> Result<(), String> {
    let ins = dto_to_instruction(&instruction).ok_or("Unknown instruction type")?;
    if matches!(
        &ins.kind,
        InstructionKind::Token(InputToken::MoveMouse(_, _, blockwork_core::input::types::Coordinate::Abs))
    ) {
        request_absolute_mouse_support(state, app).await?;
    }
    let mut s = state.lock().map_err(|e| e.to_string())?;
    // Checked before checkpointing, so a rejected drop costs no undo step.
    if let Some(mac) = &s.current_macro {
        mac.check_header_placement(&strand_id, &path, &ins)?;
        if let Some(strand) = mac.strand(&strand_id) {
            check_return_placement(mac, strand, &ins)?;
            check_loop_control_placement(strand, &path, &ins)?;
        }
    }
    push_undo(&mut s);
    if let Some(mac) = &mut s.current_macro
        && mac.insert_instruction(&strand_id, &path, ins)?
    {
        s.invalid_field_buffers.clear();
        auto_save(&s);
    }
    emit_state_updated(&app, &s);
    Ok(())
}

/// A `Return` block only makes sense inside a value-returning custom
/// block's body - enforced here so it can't be placed somewhere confusing
/// (the interpreter otherwise tolerates a stray one harmlessly).
fn check_return_placement(mac: &Macro, strand: &Strand, ins: &Instruction) -> Result<(), String> {
    if !matches!(&ins.kind, InstructionKind::Return(_)) {
        return Ok(());
    }
    let valid = matches!(strand.instructions.first().map(|i| &i.kind), Some(InstructionKind::BlockHeader(id))
        if mac.block_defs.iter().any(|b| &b.id == id && b.shape.returns_value()));
    if valid {
        Ok(())
    } else {
        Err(
            "A Return block can only be used inside a custom block that returns a value"
                .to_string(),
        )
    }
}

/// `escape loop`/`continue loop` only make sense inside a `Repeat`/`Forever`/
/// `While` body - enforced by walking every ancestor bracket named in
/// `path` and checking whether any of them is a loop instruction.
fn check_loop_control_placement(
    strand: &Strand,
    path: &[PathStep],
    ins: &Instruction,
) -> Result<(), String> {
    if !matches!(
        &ins.kind,
        InstructionKind::EscapeLoop | InstructionKind::ContinueLoop
    ) {
        return Ok(());
    }
    let mut list: &[Instruction] = &strand.instructions;
    for step in path {
        let Some(slot) = step.slot else { break };
        let Some(ancestor) = list.get(step.index) else {
            break;
        };
        if matches!(
            &ancestor.kind,
            InstructionKind::Repeat { .. }
                | InstructionKind::Forever { .. }
                | InstructionKind::While { .. }
        ) {
            return Ok(());
        }
        let Some(body) = ancestor.body(slot) else {
            break;
        };
        list = body;
    }
    Err(
        "An Escape Loop/Continue Loop block can only be used inside a Repeat/Forever/While loop"
            .to_string(),
    )
}

#[cfg(test)]
mod loop_control_placement_tests {
    use super::*;

    fn strand_with(instructions: Vec<Instruction>) -> Strand {
        Strand {
            id: "s1".into(),
            x: 0,
            y: 0,
            instructions,
        }
    }

    #[test]
    fn non_loop_control_instructions_are_always_allowed() {
        let strand = strand_with(vec![Instruction::new(InstructionKind::WhenRan)]);
        assert!(check_loop_control_placement(
            &strand,
            &[PathStep {
                index: 1,
                slot: None
            }],
            &Instruction::new(InstructionKind::Comment("x".into()))
        )
        .is_ok());
    }

    #[test]
    fn escape_loop_at_strand_top_level_is_rejected() {
        let strand = strand_with(vec![Instruction::new(InstructionKind::WhenRan)]);
        let path = vec![PathStep {
            index: 1,
            slot: None,
        }];
        assert!(check_loop_control_placement(
            &strand,
            &path,
            &Instruction::new(InstructionKind::EscapeLoop)
        )
        .is_err());
    }

    #[test]
    fn escape_loop_directly_inside_repeat_body_is_allowed() {
        let strand = strand_with(vec![
            Instruction::new(InstructionKind::WhenRan),
            Instruction::new(InstructionKind::Repeat {
                count: Value::number(3.0),
                body: vec![],
            }),
        ]);
        // Path: strand[1] (Repeat) -> slot 0 (its body) -> insertion index 0.
        let path = vec![
            PathStep {
                index: 1,
                slot: Some(0),
            },
            PathStep {
                index: 0,
                slot: None,
            },
        ];
        assert!(check_loop_control_placement(
            &strand,
            &path,
            &Instruction::new(InstructionKind::ContinueLoop)
        )
        .is_ok());
    }

    #[test]
    fn escape_loop_inside_if_inside_while_is_allowed() {
        let strand = strand_with(vec![
            Instruction::new(InstructionKind::WhenRan),
            Instruction::new(InstructionKind::While {
                condition: Value::Bool,
                body: vec![Instruction::new(InstructionKind::If {
                    condition: Value::Bool,
                    body: vec![],
                })],
            }),
        ]);
        // strand[1] (While) -> slot 0 -> [0] (If) -> slot 0 -> insertion index 0.
        let path = vec![
            PathStep {
                index: 1,
                slot: Some(0),
            },
            PathStep {
                index: 0,
                slot: Some(0),
            },
            PathStep {
                index: 0,
                slot: None,
            },
        ];
        assert!(check_loop_control_placement(
            &strand,
            &path,
            &Instruction::new(InstructionKind::EscapeLoop)
        )
        .is_ok());
    }

    #[test]
    fn escape_loop_inside_if_with_no_enclosing_loop_is_rejected() {
        let strand = strand_with(vec![
            Instruction::new(InstructionKind::WhenRan),
            Instruction::new(InstructionKind::If {
                condition: Value::Bool,
                body: vec![],
            }),
        ]);
        let path = vec![
            PathStep {
                index: 1,
                slot: Some(0),
            },
            PathStep {
                index: 0,
                slot: None,
            },
        ];
        assert!(check_loop_control_placement(
            &strand,
            &path,
            &Instruction::new(InstructionKind::EscapeLoop)
        )
        .is_err());
    }
}

pub(crate) async fn edit_instruction(
    state: &SharedState,
    app: &AppHandle,
    strand_id: String,
    path: Vec<PathStep>,
    instruction: InstructionDto,
) -> Result<(), String> {
    let ins = dto_to_instruction(&instruction).ok_or("Unknown instruction type")?;
    if matches!(
        &ins.kind,
        InstructionKind::Token(InputToken::MoveMouse(_, _, blockwork_core::input::types::Coordinate::Abs))
    ) {
        request_absolute_mouse_support(state, app).await?;
    }
    let mut s = state.lock().map_err(|e| e.to_string())?;
    // Freeform text fields coalesce keystrokes into one undo group, like
    // `edit_value_field`; every other kind gets its own undo step.
    let session = matches!(
        &ins.kind,
        InstructionKind::Command(_) | InstructionKind::Comment(_)
    )
    .then(|| EditSession::Instruction {
        strand_id: strand_id.clone(),
        index: path.clone(),
    });
    push_undo_for(&mut s, session);
    if let Some(mac) = &mut s.current_macro
        && mac.replace_instruction(&strand_id, &path, ins)
    {
        auto_save(&s);
    }
    emit_state_updated(&app, &s);
    Ok(())
}

pub(crate) fn edit_value_field(
    state: &SharedState,
    app: &AppHandle,
    location: ValueLocation,
    text: String,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    // Coalesce a run of keystrokes into this field into a single undo step.
    push_undo_for(&mut s, Some(EditSession::Value(location.clone())));
    if let Some(mac) = &mut s.current_macro {
        match mac.edit_value_text(&location, text.clone()) {
            // Text leaves are always valid - no invalid-buffer bookkeeping needed.
            ValueEdit::Text => auto_save(&s),
            // A numeric field keeps the raw text either way, so a
            // half-written number doesn't snap back mid-edit.
            ValueEdit::Number { parsed } => {
                s.invalid_field_buffers.insert(location, text);
                if parsed {
                    auto_save(&s);
                }
            }
            ValueEdit::Missing => {}
        }
    }
    emit_state_updated(&app, &s);
    Ok(())
}

pub(crate) fn set_value_kind(
    state: &SharedState,
    app: &AppHandle,
    location: ValueLocation,
    kind: String,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    push_undo(&mut s);
    let env: HashMap<String, Evaluated> = s
        .variable_values
        .lock()
        .map(|g| g.clone())
        .unwrap_or_default();
    let result = match &mut s.current_macro {
        Some(mac) => mac.set_value_kind(&location, &kind, &env),
        None => Ok(()),
    };
    prune_value_buffers(&mut s.invalid_field_buffers, &location);
    auto_save(&s);
    emit_state_updated(&app, &s);
    result
}

/// Removes the value at `location` and returns it, leaving a `Field`
/// location holding whatever it was shadowing (or its typed blank value); a
/// root `Floating` location is deleted entirely. Pairs with `put_value`/
/// `create_floating_value` on the frontend side of a drag.
pub(crate) fn take_value(
    state: &SharedState,
    app: &AppHandle,
    location: ValueLocation,
) -> Result<Value, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    push_undo(&mut s);
    let taken = s
        .current_macro
        .as_mut()
        .and_then(|mac| mac.take_value(&location));
    prune_value_buffers(&mut s.invalid_field_buffers, &location);
    auto_save(&s);
    emit_state_updated(&app, &s);
    taken.ok_or_else(|| "Nothing to take at that location".to_string())
}

/// Overwrites the node at `location` with `value` - the "put" half of
/// moving a block into a field/subfield slot. An incoming operator's
/// shadowed value is overwritten with the destination's prior content.
pub(crate) fn put_value(
    state: &SharedState,
    app: &AppHandle,
    location: ValueLocation,
    value: Value,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    push_undo(&mut s);
    if let Some(mac) = &mut s.current_macro {
        mac.put_value(&location, value);
    }
    prune_value_buffers(&mut s.invalid_field_buffers, &location);
    auto_save(&s);
    emit_state_updated(&app, &s);
    Ok(())
}

/// One-shot sample evaluation of a value tree, for the click-to-preview
/// tooltip on operator blocks - stateless. Uses `eval_text` so text-only
/// ops (`Join`/`NewLine`/`Tab`) preview without erroring as "not a number".
pub(crate) fn preview_value(state: &SharedState, value: Value) -> Result<String, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    let env: HashMap<String, Evaluated> = s
        .variable_values
        .lock()
        .map(|g| g.clone())
        .unwrap_or_default();
    let lists = s.list_values.lock().map(|g| g.clone()).unwrap_or_default();
    preview_value_with_env_and_lists(&value, &env, &lists)
}

/// The actual evaluation logic behind `preview_value`, factored out so it's
/// testable without a real `tauri::State`.
#[cfg(test)]
fn preview_value_with_env(
    value: &Value,
    env: &HashMap<String, Evaluated>,
) -> Result<String, String> {
    preview_value_with_env_and_lists(value, env, &HashMap::new())
}

fn preview_value_with_env_and_lists(
    value: &Value,
    env: &HashMap<String, Evaluated>,
    lists: &HashMap<String, Vec<ListItem>>,
) -> Result<String, String> {
    resolve_list_reporters(&value.resolve_vars(env), lists)?.eval_text()
}

/// Creates a new value block parked on open canvas - for a sidebar drop, or
/// the "create" half of dragging an existing block out onto canvas.
/// `origin_block_id` is set only when `value` is a `Param` reporter dragged
/// straight out of its declaring block's header (see blockstitchSetup.ts's
/// `createFloatingValue`) - lets a floating param render with its real
/// declared shape instead of a guess.
pub(crate) fn create_floating_value(
    state: &SharedState,
    app: &AppHandle,
    x: i32,
    y: i32,
    value: Value,
    origin_block_id: Option<String>,
) -> Result<String, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    push_undo(&mut s);
    let id = s
        .current_macro
        .as_mut()
        .ok_or("No macro selected")?
        .add_floating_value(x, y, value, origin_block_id);
    auto_save(&s);
    emit_state_updated(&app, &s);
    Ok(id)
}

/// Repositions a floating value block dropped on open canvas. No
/// `push_undo` - pure repositioning, same as `move_strand`.
pub(crate) fn move_floating_value(
    state: &SharedState,
    app: &AppHandle,
    floating_id: String,
    x: i32,
    y: i32,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if let Some(mac) = &mut s.current_macro
        && mac.move_floating_value(&floating_id, x, y)
    {
        auto_save(&s);
    }
    emit_state_updated(&app, &s);
    Ok(())
}

/// Deletes a floating value block outright (dropped on the sidebar trash) -
/// mirrors `remove_strand`.
pub(crate) fn remove_floating_value(
    state: &SharedState,
    app: &AppHandle,
    floating_id: String,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    push_undo(&mut s);
    if let Some(mac) = &mut s.current_macro {
        mac.remove_floating_value(&floating_id);
        auto_save(&s);
    }
    emit_state_updated(&app, &s);
    Ok(())
}

/// Creates a freestanding note parked on open canvas - the "Add Comment"
/// canvas-context-menu item. Mirrors `create_floating_value`.
pub(crate) fn create_comment(
    state: &SharedState,
    app: &AppHandle,
    x: i32,
    y: i32,
    text: String,
) -> Result<String, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    push_undo(&mut s);
    let id = s
        .current_macro
        .as_mut()
        .ok_or("No macro selected")?
        .add_comment(x, y, text, None);
    auto_save(&s);
    emit_state_updated(&app, &s);
    Ok(id)
}

/// Creates a note pinned to `instruction_id` - the "Add Comment" block/header
/// context-menu item. `dx`/`dy` are an offset from that instruction's
/// on-screen position, not an absolute canvas coordinate (see `Comment`'s
/// doc comment); the frontend picks a default that clears the block.
pub(crate) fn create_attached_comment(
    state: &SharedState,
    app: &AppHandle,
    instruction_id: String,
    dx: i32,
    dy: i32,
    text: String,
) -> Result<String, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    push_undo(&mut s);
    let id = s
        .current_macro
        .as_mut()
        .ok_or("No macro selected")?
        .add_comment(dx, dy, text, Some(instruction_id));
    auto_save(&s);
    emit_state_updated(&app, &s);
    Ok(id)
}

/// Repositions a note - `x`/`y` are the same value the frontend already
/// tracks for it (an absolute canvas position if freestanding, an offset
/// from its attached instruction if not), plus the pointer's drag delta; see
/// `Comment`'s doc comment. No `push_undo` - pure repositioning, same as
/// `move_floating_value`.
pub(crate) fn move_comment(
    state: &SharedState,
    app: &AppHandle,
    comment_id: String,
    x: i32,
    y: i32,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if let Some(mac) = &mut s.current_macro
        && mac.move_comment(&comment_id, x, y)
    {
        auto_save(&s);
    }
    emit_state_updated(&app, &s);
    Ok(())
}

/// Deletes a note outright (its own "×" button) - mirrors `remove_floating_value`.
pub(crate) fn remove_comment(
    state: &SharedState,
    app: &AppHandle,
    comment_id: String,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    push_undo(&mut s);
    if let Some(mac) = &mut s.current_macro {
        mac.remove_comment(&comment_id);
        auto_save(&s);
    }
    emit_state_updated(&app, &s);
    Ok(())
}

/// Edits a note's text - coalesces keystrokes into one undo group, same as
/// `edit_instruction`'s freeform-text instructions.
pub(crate) fn edit_comment_text(
    state: &SharedState,
    app: &AppHandle,
    comment_id: String,
    text: String,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    push_undo_for(
        &mut s,
        Some(EditSession::Comment {
            comment_id: comment_id.clone(),
        }),
    );
    if let Some(mac) = &mut s.current_macro
        && mac.set_comment_text(&comment_id, text)
    {
        auto_save(&s);
    }
    emit_state_updated(&app, &s);
    Ok(())
}

/// Toggles a note's collapsed state - view state, not content, so no
/// `push_undo` (same reasoning as `move_comment`).
pub(crate) fn set_comment_collapsed(
    state: &SharedState,
    app: &AppHandle,
    comment_id: String,
    collapsed: bool,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if let Some(mac) = &mut s.current_macro
        && mac.set_comment_collapsed(&comment_id, collapsed)
    {
        auto_save(&s);
    }
    emit_state_updated(&app, &s);
    Ok(())
}

pub(crate) fn remove_instruction(
    state: &SharedState,
    app: &AppHandle,
    strand_id: String,
    path: Vec<PathStep>,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    // Work on a copy: a path addressing nothing costs no undo step.
    let mut graph = match &s.current_macro {
        Some(mac) => mac.graph.clone(),
        None => {
            emit_state_updated(&app, &s);
            return Ok(());
        }
    };
    if graph.remove_instruction(&strand_id, &path) {
        push_undo(&mut s);
        if let Some(mac) = &mut s.current_macro {
            mac.graph = graph;
        }
        s.invalid_field_buffers.clear();
        auto_save(&s);
    }
    emit_state_updated(&app, &s);
    Ok(())
}

/// Deletes the instruction at `path`, splitting anything below it (in the
/// same body list) off into a new top-level strand at `(x, y)` - atomic
/// with the removal, so it's one undo step. Returns the new strand's id, or
/// `None` if nothing was split off.
pub(crate) fn delete_instruction(
    state: &SharedState,
    app: &AppHandle,
    strand_id: String,
    path: Vec<PathStep>,
    x: i32,
    y: i32,
) -> Result<Option<String>, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    let mut graph = match &s.current_macro {
        Some(mac) => mac.graph.clone(),
        None => {
            emit_state_updated(&app, &s);
            return Ok(None);
        }
    };
    // Out of range isn't worth an error: the block just isn't there.
    let Ok(new_id) = graph.delete_instruction(&strand_id, &path, x, y) else {
        emit_state_updated(&app, &s);
        return Ok(None);
    };
    push_undo(&mut s);
    if let Some(mac) = &mut s.current_macro {
        mac.graph = graph;
    }
    s.invalid_field_buffers.clear();
    auto_save(&s);
    emit_state_updated(&app, &s);
    Ok(new_id)
}

pub(crate) fn reorder_instruction(
    state: &SharedState,
    app: &AppHandle,
    strand_id: String,
    path: Vec<PathStep>,
    direction: i32,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    // Reordering can legitimately do nothing (either end of the list, or a
    // swap past a header), so only checkpoint if it moved.
    let mut graph = match &s.current_macro {
        Some(mac) => mac.graph.clone(),
        None => {
            emit_state_updated(&app, &s);
            return Ok(());
        }
    };
    if graph.reorder_instruction(&strand_id, &path, direction) {
        push_undo(&mut s);
        if let Some(mac) = &mut s.current_macro {
            mac.graph = graph;
        }
        s.invalid_field_buffers.clear();
        auto_save(&s);
    }
    emit_state_updated(&app, &s);
    Ok(())
}

pub(crate) fn clear_instructions(
    state: &SharedState,
    app: &AppHandle,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if !s.confirm_clear_instructions {
        s.confirm_clear_instructions = true;
        s.clear_confirm_remaining_secs = CLEAR_CONFIRM_TIMEOUT_SECS as u8;
        s.clear_confirm_generation = s.clear_confirm_generation.wrapping_add(1);
        let timeout_gen = s.clear_confirm_generation;
        emit_state_updated(&app, &s);
        drop(s);

        let state_clone = Arc::clone(&*state);
        let app_clone = app.clone();
        crate::async_runtime::spawn(async move {
            for remaining in (1..=CLEAR_CONFIRM_TIMEOUT_SECS).rev() {
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                if let Ok(mut s) = state_clone.lock() {
                    if s.clear_confirm_generation != timeout_gen {
                        break;
                    }
                    s.clear_confirm_remaining_secs = remaining as u8 - 1;
                    if remaining == 1 {
                        s.confirm_clear_instructions = false;
                    }
                    emit_state_updated(&app_clone, &s);
                }
            }
        });
    } else {
        push_undo(&mut s);
        if let Some(mac) = &mut s.current_macro {
            // Clearing wipes every strand, including "When Ran" blocks -
            // "start this macro over from scratch".
            mac.clear_strands();
            s.invalid_field_buffers.clear();
            auto_save(&s);
            s.confirm_clear_instructions = false;
            s.clear_confirm_remaining_secs = 0;
        }
        emit_state_updated(&app, &s);
    }
    Ok(())
}

/// Swaps the canvas for the neighbouring history entry; `step` is
/// [`History::undo`] or [`History::redo`], which are otherwise identical.
fn apply_history_step(
    state: &SharedState,
    app: &AppHandle,
    step: fn(&mut crate::state::History<MacroGraph>, MacroGraph) -> Option<MacroGraph>,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if let Some(current) = s
        .current_macro
        .as_ref()
        .map(|mac| mac.graph.clone())
        && let Some(restored_graph) = step(&mut s.history, current)
    {
        if let Some(mac) = &mut s.current_macro {
            mac.graph = restored_graph;
            mac.ensure_id();
        }
        sync_variable_values(&mut s);
        sync_list_values(&mut s);
        s.invalid_field_buffers.clear();
        auto_save(&s);
    }
    emit_state_updated(app, &s);
    Ok(())
}

pub(crate) fn undo(state: &SharedState, app: &AppHandle) -> Result<(), String> {
    apply_history_step(state, app, crate::state::History::undo)
}

pub(crate) fn redo(state: &SharedState, app: &AppHandle) -> Result<(), String> {
    apply_history_step(state, app, crate::state::History::redo)
}

// ─── Strands (canvas) ──────────────────────────────────────────────────────

/// Creates a new detached strand. `x`/`y` default to an auto-picked spot
/// next to the farthest-right strand; an explicit position and initial
/// `instruction` are passed for a palette-block drop, as one atomic call.
pub(crate) fn add_strand(
    state: &SharedState,
    app: &AppHandle,
    x: Option<i32>,
    y: Option<i32>,
    instruction: Option<InstructionDto>,
) -> Result<String, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    let ins = match instruction {
        Some(dto) => vec![dto_to_instruction(&dto).ok_or("Unknown instruction type")?],
        None => vec![],
    };
    push_undo(&mut s);
    let mac = s.current_macro.as_mut().ok_or("No macro selected")?;
    let (default_x, default_y) = mac.next_strand_position();
    let new_id = mac.add_strand(x.unwrap_or(default_x), y.unwrap_or(default_y), ins);
    auto_save(&s);
    emit_state_updated(&app, &s);
    Ok(new_id)
}

pub(crate) fn remove_strand(
    state: &SharedState,
    app: &AppHandle,
    strand_id: String,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    push_undo(&mut s);
    if let Some(mac) = &mut s.current_macro {
        mac.remove_strand(&strand_id);
        drop_strand_buffers(&mut s.invalid_field_buffers, &strand_id);
        auto_save(&s);
    }
    emit_state_updated(&app, &s);
    Ok(())
}

/// Repositions a strand on the canvas - used while dragging a stack that
/// ends up dropped on empty space rather than snapped onto another strand.
pub(crate) fn move_strand(
    state: &SharedState,
    app: &AppHandle,
    strand_id: String,
    x: i32,
    y: i32,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if let Some(mac) = &mut s.current_macro
        && mac.move_strand(&strand_id, x, y)
    {
        auto_save(&s);
    }
    emit_state_updated(&app, &s);
    Ok(())
}

/// Detaches the instructions at and after `path` (within the same body list)
/// into a new top-level strand at `(x, y)`, returning its id. This is how
/// the frontend "picks up" a block: split first, then drop as a stray strand
/// or re-merge elsewhere.
pub(crate) fn split_strand(
    state: &SharedState,
    app: &AppHandle,
    strand_id: String,
    path: Vec<PathStep>,
    x: i32,
    y: i32,
) -> Result<String, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    push_undo(&mut s);
    let new_id = s
        .current_macro
        .as_mut()
        .ok_or("No macro selected")?
        .split_strand(&strand_id, &path, x, y)?;
    s.invalid_field_buffers.clear();
    auto_save(&s);
    emit_state_updated(&app, &s);
    Ok(new_id)
}

/// Splices `dragged_id`'s instructions into `target_id` at `path` and
/// deletes the (now empty) dragged strand - how two stacks snap together.
/// A "When Ran" strand can only be a merge target, never the dragged side.
pub(crate) fn merge_strand(
    state: &SharedState,
    app: &AppHandle,
    dragged_id: String,
    target_id: String,
    path: Vec<PathStep>,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    // Merge validates before it moves anything, so a rejected drop leaves
    // both strands and the undo stack untouched.
    let mut graph = s
        .current_macro
        .as_ref()
        .map(|mac| mac.graph.clone())
        .ok_or("No macro selected")?;
    graph.merge_strand(&dragged_id, &target_id, &path)?;
    push_undo(&mut s);
    if let Some(mac) = &mut s.current_macro {
        mac.graph = graph;
    }
    drop_strand_buffers(&mut s.invalid_field_buffers, &dragged_id);
    auto_save(&s);
    emit_state_updated(&app, &s);
    Ok(())
}

/// Moves the tail at and after `path` in `strand_id` into `target_id` at
/// `target_path` atomically - the Qt canvas's attach-on-drop for a partial
/// stack drag, without a split-then-merge round trip (the Qt bridge drops
/// backend replies, so it can't chain the new split id into a merge).
pub(crate) fn merge_tail(
    state: &SharedState,
    app: &AppHandle,
    strand_id: String,
    path: Vec<PathStep>,
    target_id: String,
    target_path: Vec<PathStep>,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    let mut graph = s
        .current_macro
        .as_ref()
        .map(|mac| mac.graph.clone())
        .ok_or("No macro selected")?;
    graph.merge_tail(&strand_id, &path, &target_id, &target_path)?;
    push_undo(&mut s);
    if let Some(mac) = &mut s.current_macro {
        mac.graph = graph;
    }
    s.invalid_field_buffers.clear();
    auto_save(&s);
    emit_state_updated(&app, &s);
    Ok(())
}

/// Creates a new detached strand at `(x, y)` holding `instructions`
/// verbatim - how "Paste" drops previously-copied blocks onto the canvas.
pub(crate) fn paste_instructions(
    state: &SharedState,
    app: &AppHandle,
    x: i32,
    y: i32,
    instructions: Vec<InstructionDto>,
) -> Result<String, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    let ins: Vec<Instruction> = instructions
        .iter()
        .map(dto_to_instruction)
        .collect::<Option<Vec<_>>>()
        .ok_or("Unknown instruction type")?;
    push_undo(&mut s);
    let new_id = s
        .current_macro
        .as_mut()
        .ok_or("No macro selected")?
        .add_strand(x, y, ins);
    auto_save(&s);
    emit_state_updated(&app, &s);
    Ok(new_id)
}

/// Sets the strand that freshly-recorded input is appended to. Not part of
/// the undo/redo stacks, so it survives undo/redo untouched; no-ops if
/// `strand_id` doesn't exist.
pub(crate) fn set_recording_target(
    state: &SharedState,
    app: &AppHandle,
    strand_id: String,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if let Some(mac) = &mut s.current_macro {
        if mac.strand(&strand_id).is_some() {
            mac.recording_target = Some(strand_id);
            auto_save(&s);
        }
    }
    emit_state_updated(&app, &s);
    Ok(())
}

// ─── Key capture ───────────────────────────────────────────────────────────

pub(crate) fn start_key_capture(
    state: &SharedState,
    app: &AppHandle,
    strand_id: String,
    path: Vec<PathStep>,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.key_capture = Some(KeyCaptureTarget::Strand(strand_id, path));
    emit_state_updated(&app, &s);
    Ok(())
}

/// Same capture flow, but for a key field with no backing strand/instruction
/// (the sidebar's Key prefab) - the result lands in `pending_standalone_key`
/// instead of being written into a strand.
pub(crate) fn start_standalone_key_capture(
    state: &SharedState,
    app: &AppHandle,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.key_capture = Some(KeyCaptureTarget::Standalone);
    emit_state_updated(&app, &s);
    Ok(())
}

pub(crate) fn key_capture_event(
    state: &SharedState,
    app: &AppHandle,
    code: String,
    key: String,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    let Some(target) = s.key_capture.take() else {
        return Ok(());
    };

    let captured_key = blockwork_core::key_mapping::web_code_to_macro_key(&code)
        .or_else(|| blockwork_core::key_mapping::web_key_to_macro_key(&key));

    if let Some(mk) = captured_key {
        match target {
            KeyCaptureTarget::Strand(strand_id, path) => {
                let captured = s.current_macro.as_mut().is_some_and(|mac| {
                    match mac.instruction_at_mut(&strand_id, &path) {
                        Some(Instruction {
                            kind: InstructionKind::Token(InputToken::Key(key, _)),
                            ..
                        }) => {
                            *key = mk;
                            true
                        }
                        _ => false,
                    }
                });
                if captured {
                    auto_save(&s);
                }
            }
            KeyCaptureTarget::Standalone => {
                s.pending_standalone_key = Some(
                    blockwork_core::input::key_to_string(&mk)
                        .unwrap_or("Unknown")
                        .to_string(),
                );
            }
        }
    }
    emit_state_updated(&app, &s);
    Ok(())
}

/// Consumes `pending_standalone_key` once the frontend has copied it, so a
/// stale value can't leak into the next capture.
pub(crate) fn clear_standalone_key_capture(
    state: &SharedState,
    app: &AppHandle,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.pending_standalone_key = None;
    emit_state_updated(&app, &s);
    Ok(())
}

// ─── Execution ─────────────────────────────────────────────────────────────

pub(crate) fn run_macro(
    state: &SharedState,
    app: &AppHandle,
) -> Result<(), String> {
    let (mac, emulator, is_looping, loop_mode, speed_multiplier, variables, lists) = {
        let s = state.lock().map_err(|e| e.to_string())?;
        let mac = s.current_macro.clone();
        let emulator = s.emulator.as_ref().map(Arc::clone);
        let is_looping = Arc::clone(&s.is_looping);
        let loop_mode = s.loop_mode_enabled;
        let speed_multiplier =
            mac.as_ref().map_or(1.0, |m| m.speed_multiplier) * s.global_speed_multiplier;
        let variables = Arc::clone(&s.variable_values);
        let lists = Arc::clone(&s.list_values);
        (
            mac,
            emulator,
            is_looping,
            loop_mode,
            speed_multiplier,
            variables,
            lists,
        )
    };

    if let (Some(mac), Some(emulator)) = (mac, emulator) {
        if loop_mode {
            if let Err(e) = loop_control::start_loop(&is_looping) {
                warn!("Failed to start loop: {e}");
                return Ok(());
            }
            let mac_name = mac.name.clone();
            let loop_task = macros_thread::into_loop_task(
                mac,
                Arc::clone(&emulator),
                Arc::clone(&is_looping),
                speed_multiplier,
                variables,
                lists,
                Arc::clone(&*state),
                app.clone(),
            );
            let mut s = state.lock().map_err(|e| e.to_string())?;
            if let Err(e) = macros_thread::spawn_macro_thread(
                &mut s.thread_pool,
                format!("loop_{}", mac_name),
                loop_task,
            ) {
                warn!("Failed to spawn loop thread: {e}");
                let _ = loop_control::stop_loop(&is_looping);
            }
        } else {
            let _ = loop_control::set_loop_state(&is_looping, true);
            let mac_name = mac.name.clone();
            let single_run_task = macros_thread::into_single_run_task(
                mac,
                Arc::clone(&emulator),
                Arc::clone(&is_looping),
                speed_multiplier,
                variables,
                lists,
                Arc::clone(&*state),
                app.clone(),
            );
            let mut s = state.lock().map_err(|e| e.to_string())?;
            if let Err(e) = macros_thread::spawn_macro_thread(
                &mut s.thread_pool,
                format!("run_{}", mac_name),
                single_run_task,
            ) {
                warn!("Failed to spawn run thread: {e}");
                let _ = loop_control::stop_loop(&is_looping);
            }
        }
    }

    let s = state.lock().map_err(|e| e.to_string())?;
    emit_state_updated(&app, &s);
    Ok(())
}

pub(crate) fn toggle_loop_mode(
    state: &SharedState,
    app: &AppHandle,
    enabled: bool,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.loop_mode_enabled = enabled;
    config::update_settings(|settings| settings.loop_mode_enabled = Some(enabled));
    emit_state_updated(&app, &s);
    Ok(())
}

pub(crate) fn set_global_speed_multiplier(
    state: &SharedState,
    app: &AppHandle,
    multiplier: f64,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    let clamped = multiplier.clamp(
        *SPEED_MULTIPLIER_RANGE.start(),
        *SPEED_MULTIPLIER_RANGE.end(),
    );
    s.global_speed_multiplier = clamped;
    config::update_settings(|settings| settings.global_speed_multiplier = Some(clamped));
    emit_state_updated(&app, &s);
    Ok(())
}

// ─── Recording ─────────────────────────────────────────────────────────────

/// Seeds the recorder's tracked cursor position from the real, current
/// cursor position so the first captured move has a baseline - needed for
/// absolute-move recording, which adds each relative delta the backend
/// reports onto a known starting point. Best-effort: if the backend can't
/// report a position, absolute recording waits for a later seed.
fn seed_recording_mouse_pos(s: &crate::state::AppState) {
    let Some(emulator) = s.emulator.as_ref() else {
        return;
    };
    let Ok(mut emulator) = emulator.lock() else {
        return;
    };
    // Absolute recording needs an exact origin, which on Wayland means placing
    // the cursor rather than asking where it is.
    let pos = if s.record_mouse_relative || !s.record_mouse_movement {
        emulator.cursor_pos()
    } else {
        emulator.anchor_cursor()
    };
    if let Some((x, y)) = pos {
        recording::set_last_mouse_pos(x as f64, y as f64);
    }
}

pub(crate) fn start_recording(
    state: &SharedState,
    app: &AppHandle,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if s.current_macro.is_none() {
        return Ok(());
    }
    s.recording_countdown_generation = s.recording_countdown_generation.wrapping_add(1);
    let countdown_gen = s.recording_countdown_generation;
    s.recording_phase = RecordingPhase::Countdown(3);
    emit_state_updated(&app, &s);
    drop(s);

    let state_clone = Arc::clone(&*state);
    let app_clone = app.clone();
    crate::async_runtime::spawn(async move {
        for n in (0u8..3).rev() {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            let mut s = match state_clone.lock() {
                Ok(g) => g,
                Err(_) => return,
            };
            if s.recording_countdown_generation != countdown_gen {
                return;
            }
            if n == 0 {
                s.recording_phase = RecordingPhase::Active;
                recording::reset_timing();
                seed_recording_mouse_pos(&s);
                recording::RECORDING_ACTIVE.store(true, Ordering::Relaxed);
                emit_state_updated(&app_clone, &s);
                return;
            }
            s.recording_phase = RecordingPhase::Countdown(n);
            emit_state_updated(&app_clone, &s);
        }
    });
    Ok(())
}

pub(crate) fn stop_recording(
    state: &SharedState,
    app: &AppHandle,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    stop_recording_impl(&mut s);
    emit_state_updated(&app, &s);
    Ok(())
}

/// Returns the macro to auto-save (an owned clone), if recorded instructions
/// actually got appended - saving itself happens after the state lock is
/// released, see `stop_recording_internal`.
fn stop_recording_impl(s: &mut crate::state::AppState) -> Option<Macro> {
    recording::RECORDING_ACTIVE.store(false, Ordering::Relaxed);
    s.recording_phase = RecordingPhase::Idle;
    // Cancel any in-progress countdown
    s.recording_countdown_generation = s.recording_countdown_generation.wrapping_add(1);

    let instructions: Vec<Instruction> = recording::get_recording_queue()
        .lock()
        .unwrap()
        .drain(..)
        .collect();
    if instructions.is_empty() {
        return None;
    }
    push_undo(s);
    let mac = s.current_macro.as_mut()?;
    mac.recording_target_mut().instructions.extend(instructions);
    Some(mac.clone())
}

/// Called from the QueueSignal background task when the OS-level hook signals stop.
pub(crate) fn stop_recording_internal(state: &SharedState, app: &AppHandle) {
    let (to_save, dto) = {
        let Ok(mut s) = state.lock() else { return };
        let to_save = stop_recording_impl(&mut s);
        (to_save, build_state_dto(&s))
    };
    // The disk write (and the emit below) run after the state lock is
    // released. IPC commands go through this same lock (`StartRecordingImmediate`
    // needs it for `reset_timing()`), so holding it across a save made a
    // closely-following next attempt's start signal land inconsistently.
    if let Some(mac) = to_save {
        if let Err(e) = mac.save() {
            warn!("Failed to auto-save macro: {e}");
        } else {
            config::set_selected_macro_id(Some(&mac.id));
        }
    }
    app.emit_state(&dto);
}

pub(crate) async fn toggle_record_mouse_relative(
    state: &SharedState,
    app: &AppHandle,
    relative: bool,
) -> Result<(), String> {
    if !relative && !blockwork_core::macros::backend::absolute_mouse_position_source_available() {
        return Err("Absolute mouse recording isn't available in this session.".to_string());
    }
    if !relative && !blockwork_core::macros::backend::absolute_mouse_position_available() {
        request_absolute_mouse_support(state, app).await?;
    }
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.record_mouse_relative = relative;
    recording::RECORD_MOUSE_RELATIVE.store(relative, Ordering::Relaxed);
    config::update_settings(|settings| settings.record_mouse_relative = Some(relative));
    emit_state_updated(&app, &s);
    Ok(())
}

pub(crate) fn toggle_record_mouse_movement(
    state: &SharedState,
    app: &AppHandle,
    enabled: bool,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.record_mouse_movement = enabled;
    recording::RECORD_MOUSE_MOVEMENT.store(enabled, Ordering::Relaxed);
    config::update_settings(|settings| settings.record_mouse_movement = Some(enabled));
    emit_state_updated(&app, &s);
    Ok(())
}

// ─── Navigation ────────────────────────────────────────────────────────────

pub(crate) fn open_settings(
    state: &SharedState,
    app: &AppHandle,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.page = Page::Settings;
    s.hotkey_bindings = config::load_hotkey_bindings();
    s.combo_capture = None;
    s.pending_macro_hotkey = None;
    emit_state_updated(&app, &s);
    Ok(())
}

pub(crate) fn close_settings(
    state: &SharedState,
    app: &AppHandle,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.page = Page::Main;
    s.combo_capture = None;
    s.pending_macro_hotkey = None;
    emit_state_updated(&app, &s);
    Ok(())
}


// ─── Hotkey bindings ───────────────────────────────────────────────────────

pub(crate) fn start_combo_capture(
    state: &SharedState,
    app: &AppHandle,
    action: HotkeyActionDto,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.combo_capture = Some(ComboCapture::Named(dto_to_hotkey_action(&action)));
    emit_state_updated(&app, &s);
    Ok(())
}

pub(crate) fn start_pending_combo_capture(
    state: &SharedState,
    app: &AppHandle,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.combo_capture = Some(ComboCapture::Pending);
    emit_state_updated(&app, &s);
    Ok(())
}

pub(crate) fn combo_capture_event(
    state: &SharedState,
    app: &AppHandle,
    code: String,
    modifiers: u8,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    let key_name = match blockwork_core::key_mapping::web_code_to_rdev_name(&code) {
        Some(n) => n,
        None => return Ok(()),
    };
    let combo = KeyCombo {
        modifiers,
        key: key_name,
    };

    match s.combo_capture.take() {
        Some(ComboCapture::Named(action)) => {
            // StopRecording must never require a combo: modifier keys held
            // while it fires would themselves have already been captured as
            // macro steps before the trigger key arrives.
            let combo = if matches!(action, HotkeyAction::StopRecording) {
                KeyCombo {
                    modifiers: 0,
                    ..combo
                }
            } else {
                combo
            };
            if let Some(existing) = s.hotkey_bindings.iter_mut().find(|b| b.action == action) {
                existing.combo = combo;
            } else {
                s.hotkey_bindings.push(HotkeyBinding { action, combo });
            }
            save_hotkey_bindings_impl(&mut s);
        }
        Some(ComboCapture::Pending) => {
            let entry = s.pending_macro_hotkey.get_or_insert((None, None));
            entry.1 = Some(combo);
        }
        None => {}
    }
    emit_state_updated(&app, &s);
    Ok(())
}

pub(crate) fn cancel_combo_capture(
    state: &SharedState,
    app: &AppHandle,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.combo_capture = None;
    emit_state_updated(&app, &s);
    Ok(())
}

pub(crate) fn set_pending_macro_idx(
    state: &SharedState,
    app: &AppHandle,
    index: Option<usize>,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    let entry = s.pending_macro_hotkey.get_or_insert((None, None));
    entry.0 = index;
    emit_state_updated(&app, &s);
    Ok(())
}

pub(crate) fn add_macro_hotkey(
    state: &SharedState,
    app: &AppHandle,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if let Some((Some(idx), Some(combo))) = s.pending_macro_hotkey.take() {
        let macros = config::get_macros_from_config();
        if let Some(mac) = macros.get(idx) {
            let binding = HotkeyBinding {
                action: HotkeyAction::RunSpecificMacro(mac.id.clone()),
                combo,
            };
            s.hotkey_bindings.push(binding);
            save_hotkey_bindings_impl(&mut s);
        }
    }
    emit_state_updated(&app, &s);
    Ok(())
}

pub(crate) fn remove_hotkey_binding(
    state: &SharedState,
    app: &AppHandle,
    index: usize,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if index < s.hotkey_bindings.len() {
        s.hotkey_bindings.remove(index);
        save_hotkey_bindings_impl(&mut s);
    }
    emit_state_updated(&app, &s);
    Ok(())
}

pub(crate) fn clear_named_hotkey(
    state: &SharedState,
    app: &AppHandle,
    action: HotkeyActionDto,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    let action = dto_to_hotkey_action(&action);
    s.hotkey_bindings.retain(|b| b.action != action);
    save_hotkey_bindings_impl(&mut s);
    emit_state_updated(&app, &s);
    Ok(())
}

pub(crate) fn reset_hotkey_to_default(
    state: &SharedState,
    app: &AppHandle,
    action: HotkeyActionDto,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    let action = dto_to_hotkey_action(&action);
    if let Some(default_combo) = config::default_combo_for_action(&action) {
        if let Some(existing) = s.hotkey_bindings.iter_mut().find(|b| b.action == action) {
            existing.combo = default_combo;
        } else {
            s.hotkey_bindings.push(HotkeyBinding {
                action,
                combo: default_combo,
            });
        }
        save_hotkey_bindings_impl(&mut s);
    }
    emit_state_updated(&app, &s);
    Ok(())
}

fn save_hotkey_bindings_impl(s: &mut crate::state::AppState) {
    config::save_hotkey_bindings(&s.hotkey_bindings);
    recording::update_hotkey_table(s.hotkey_bindings.clone());
}

// ─── IPC server ────────────────────────────────────────────────────────────

pub(crate) fn set_ipc_port_text(
    state: &SharedState,
    app: &AppHandle,
    text: String,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    match text.trim().parse::<u16>() {
        Ok(port) => {
            s.ipc_port_invalid = false;
            config::update_settings(|settings| settings.ipc_port = Some(port));
        }
        Err(_) => {
            s.ipc_port_invalid = true;
        }
    }
    s.ipc_port_text = text;
    emit_state_updated(&app, &s);
    Ok(())
}

pub(crate) async fn start_ipc_server(
    state: &SharedState,
    app: &AppHandle,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if s.ipc_server.is_none() {
        if let Ok(port) = s.ipc_port_text.trim().parse::<u16>() {
            let (tx, rx) = tokio::sync::watch::channel(false);
            s.ipc_server = Some(crate::async_runtime::spawn(
                blockwork_core::ipc::run_server(port, rx),
            ));
            s.ipc_shutdown_tx = Some(tx);
            s.ipc_active_port = Some(port);
        }
    }
    emit_state_updated(&app, &s);
    Ok(())
}

pub(crate) fn stop_ipc_server(
    state: &SharedState,
    app: &AppHandle,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if let Some(tx) = s.ipc_shutdown_tx.take() {
        let _ = tx.send(true);
    }
    if let Some(handle) = s.ipc_server.take() {
        handle.abort();
    }
    s.ipc_active_port = None;
    emit_state_updated(&app, &s);
    Ok(())
}

pub(crate) fn set_ipc_auto_start(
    state: &SharedState,
    app: &AppHandle,
    enabled: bool,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.ipc_auto_start = enabled;
    config::update_settings(|settings| settings.ipc_auto_start = Some(enabled));
    emit_state_updated(&app, &s);
    Ok(())
}

// ─── System tray ────────────────────────────────────────────────────────────

pub(crate) fn set_close_to_tray(
    state: &SharedState,
    app: &AppHandle,
    enabled: bool,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.close_to_tray = enabled;
    config::update_settings(|settings| settings.close_to_tray = Some(enabled));
    app.send(crate::Event::CloseToTray(enabled));
    emit_state_updated(app, &s);
    Ok(())
}

// ─── Updates (Windows/macOS) ───────────────────────────────────────────────

pub(crate) fn check_for_updates(
    state: &SharedState,
    app: &AppHandle,
) -> Result<(), String> {
    {
        let mut s = state.lock().map_err(|e| e.to_string())?;
        s.update_check_state = UpdateCheckState::Checking;
        emit_state_updated(&app, &s);
    }
    let state_clone = Arc::clone(&*state);
    let app_clone = app.clone();
    crate::async_runtime::spawn(async move {
        check_for_updates_internal(&state_clone, &app_clone).await;
    });
    Ok(())
}

pub(crate) async fn check_for_updates_internal(
    state: &SharedState,
    app: &AppHandle,
) {
    #[cfg(any(windows, target_os = "macos"))]
    {
        let version = env!("CARGO_PKG_VERSION").to_string();
        let result = tokio::task::spawn_blocking(move || {
            blockwork_core::updater::check_for_update(&version)
        })
        .await
        .unwrap_or_else(|e| Err(e.to_string()));
        if let Ok(mut s) = state.lock() {
            s.update_check_state = match result {
                Ok(Some(info)) => UpdateCheckState::UpdateAvailable(info.version),
                Ok(None) => UpdateCheckState::UpToDate,
                Err(e) => UpdateCheckState::Error(e),
            };
            emit_state_updated(app, &s);
        }
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    if let Ok(mut s) = state.lock() {
        s.update_check_state =
            UpdateCheckState::Error("Updates are only supported on Windows and macOS".to_string());
        emit_state_updated(app, &s);
    }
}

pub(crate) fn apply_update(
    state: &SharedState,
    app: &AppHandle,
) -> Result<(), String> {
    {
        let mut s = state.lock().map_err(|e| e.to_string())?;
        s.update_check_state = UpdateCheckState::Applying;
        emit_state_updated(&app, &s);
    }
    #[cfg(any(windows, target_os = "macos"))]
    {
        let state_clone = Arc::clone(&*state);
        let app_clone = app.clone();
        crate::async_runtime::spawn(async move {
            let version = env!("CARGO_PKG_VERSION").to_string();
            let result = tokio::task::spawn_blocking(move || {
                blockwork_core::updater::apply_update(&version)
            })
            .await
            .unwrap_or_else(|e| Err(e.to_string()));
            match result {
                Ok(_) => app_clone.send(crate::Event::Quit),
                Err(e) => {
                    if let Ok(mut s) = state_clone.lock() {
                        s.update_check_state = UpdateCheckState::Error(e);
                        emit_state_updated(&app_clone, &s);
                    }
                }
            }
        });
    }
    Ok(())
}

// ─── Hotkey action handler (called from QueueSignal task) ──────────────────

pub(crate) fn handle_hotkey_action(
    state: &SharedState,
    app: &AppHandle,
    action: HotkeyAction,
) {
    match &action {
        HotkeyAction::RunMacro | HotkeyAction::RunSpecificMacro(_) => {
            let s = match state.lock() {
                Ok(g) => g,
                Err(_) => return,
            };
            if let Some(emulator) = s.emulator.as_ref() {
                let emulator = Arc::clone(emulator);
                let is_looping = Arc::clone(&s.is_looping);
                let loop_mode = s.loop_mode_enabled;
                let global_speed_multiplier = s.global_speed_multiplier;
                let shared_state = Arc::clone(state);
                let mac = match &action {
                    HotkeyAction::RunMacro => s.current_macro.clone(),
                    HotkeyAction::RunSpecificMacro(id) => {
                        let macs = s.macros_list.clone();
                        macs.iter().find(|m| &m.id == id).cloned()
                    }
                    _ => None,
                };
                drop(s);
                if let Some(mac) = mac {
                    let speed_multiplier = mac.speed_multiplier * global_speed_multiplier;
                    // Seeded from this macro's own persisted values, not
                    // `AppState::variable_values` (tracks the *selected* macro).
                    let variables: VariableStore = Arc::new(Mutex::new(
                        mac.variables
                            .iter()
                            .map(|v| (v.name.clone(), v.value.clone()))
                            .collect(),
                    ));
                    let lists: ListStore = Arc::new(Mutex::new(
                        mac.lists
                            .iter()
                            .map(|list| (list.name.clone(), list.items.clone()))
                            .collect(),
                    ));
                    run_macro_task(
                        mac,
                        emulator,
                        is_looping,
                        loop_mode,
                        speed_multiplier,
                        variables,
                        lists,
                        shared_state,
                        app.clone(),
                    );
                }
            }
        }
        HotkeyAction::StopLoop => {
            // Clears every in-flight run's stop flag, not just loop mode's -
            // a single run has its own stop flag that `is_looping` doesn't reach.
            let cleared = blockwork_core::macros::run_registry::stop_all();
            tracing::info!(cleared, "StopLoop hotkey handled");
            if let Ok(s) = state.lock() {
                if let Ok(mut lk) = s.is_looping.lock() {
                    *lk = false;
                }
                emit_state_updated(app, &s);
            }
        }
        HotkeyAction::NextMacro => {
            let macros = config::get_macros_from_config();
            if !macros.is_empty() {
                if let Ok(mut s) = state.lock() {
                    let current_id = config::get_selected_macro_id();
                    let current_idx = current_id
                        .and_then(|id| macros.iter().position(|m| m.id == id))
                        .unwrap_or(0);
                    let next = (current_idx + 1) % macros.len();
                    s.macro_selected = Some(next);
                    s.current_macro = macros.get(next).cloned();
                    if let Some(mac) = &s.current_macro {
                        config::set_selected_macro_id(Some(&mac.id));
                    }
                    sync_variable_values(&mut s);
                    sync_list_values(&mut s);
                    emit_state_updated(app, &s);
                }
            }
        }
        HotkeyAction::PrevMacro => {
            let macros = config::get_macros_from_config();
            if !macros.is_empty() {
                if let Ok(mut s) = state.lock() {
                    let current_id = config::get_selected_macro_id();
                    let current_idx = current_id
                        .and_then(|id| macros.iter().position(|m| m.id == id))
                        .unwrap_or(0);
                    let prev = if current_idx == 0 {
                        macros.len() - 1
                    } else {
                        current_idx - 1
                    };
                    s.macro_selected = Some(prev);
                    s.current_macro = macros.get(prev).cloned();
                    if let Some(mac) = &s.current_macro {
                        config::set_selected_macro_id(Some(&mac.id));
                    }
                    sync_variable_values(&mut s);
                    sync_list_values(&mut s);
                    emit_state_updated(app, &s);
                }
            }
        }
        HotkeyAction::ToggleLoop => {
            if let Ok(mut s) = state.lock() {
                s.loop_mode_enabled = !s.loop_mode_enabled;
                let enabled = s.loop_mode_enabled;
                config::update_settings(|settings| settings.loop_mode_enabled = Some(enabled));
                emit_state_updated(app, &s);
            }
        }
        HotkeyAction::StartRecordingImmediate => {
            if let Ok(mut s) = state.lock() {
                if s.current_macro.is_some() {
                    s.recording_countdown_generation =
                        s.recording_countdown_generation.wrapping_add(1);
                    s.recording_phase = RecordingPhase::Active;
                    recording::reset_timing();
                    seed_recording_mouse_pos(&s);
                    recording::RECORDING_ACTIVE.store(true, Ordering::Relaxed);
                    emit_state_updated(app, &s);
                }
            }
        }
        HotkeyAction::StopRecording => {
            // Handled directly in recording::start_grab_thread while active;
            // reached here only if pressed while idle - no-op.
        }
        HotkeyAction::Undo => {
            let _ = undo(state, app);
        }
        HotkeyAction::Redo => {
            let _ = redo(state, app);
        }
    }
}

fn run_macro_task(
    mac: Macro,
    emulator: Arc<std::sync::Mutex<dyn blockwork_core::macros::backend::InputBackend>>,
    is_looping: Arc<std::sync::Mutex<bool>>,
    loop_mode: bool,
    speed_multiplier: f64,
    variables: VariableStore,
    lists: ListStore,
    state: SharedState,
    app: AppHandle,
) {
    // Kept alive past the move into the task so the pre-run focus/modifier
    // cleanup can play its releases through the very backend the run will use.
    #[cfg(windows)]
    let prep_backend = Arc::clone(&emulator);

    if loop_mode {
        if let Ok(mut st) = is_looping.lock() {
            *st = true;
        }
        let loop_flag = Arc::clone(&is_looping);
        // `into_loop_task` already loops until `loop_flag` clears and
        // persists final variable values - no need to hand-roll it here.
        let task = macros_thread::into_loop_task(
            mac,
            emulator,
            loop_flag,
            speed_multiplier,
            variables,
            lists,
            state,
            app,
        );
        tokio::task::spawn_blocking(move || {
            // Registered before the run starts so "Stop Loop" can reach it
            // even during the focus switch below.
            let playback_guard = blockwork_core::macros::run_registry::begin_run();
            #[cfg(windows)]
            blockwork_core::macros::backend::windows::prepare_for_macro_execution(&prep_backend);
            task();
            blockwork_core::macros::run_registry::end_run(&playback_guard);
        });
    } else {
        if let Ok(mut st) = is_looping.lock() {
            *st = true;
        }
        let stop_flag = Arc::clone(&is_looping);
        let task = macros_thread::into_single_run_task(
            mac,
            emulator,
            stop_flag,
            speed_multiplier,
            variables,
            lists,
            state,
            app,
        );
        tokio::task::spawn_blocking(move || {
            // Same pre-registration as the loop branch above.
            let playback_guard = blockwork_core::macros::run_registry::begin_run();
            #[cfg(windows)]
            blockwork_core::macros::backend::windows::prepare_for_macro_execution(&prep_backend);
            task();
            blockwork_core::macros::run_registry::end_run(&playback_guard);
        });
    }
}

// ─── Open App picker ────────────────────────────────────────────────────────

/// Lists installed applications for the "Open App" instruction's picker
/// popup - see `installed_apps`. `async` so scanning `.desktop`/icon-theme
/// files (Linux) or walking the Start Menu (Windows) doesn't block the main
/// thread; stateless, so unlike almost every other command here it doesn't
/// take `State<SharedState>` at all.
pub(crate) async fn list_installed_apps() -> Vec<crate::state::AppEntryDto> {
    crate::installed_apps::list_apps()
        .into_iter()
        .map(|a| crate::state::AppEntryDto {
            name: a.name,
            command: a.command,
            icon: a.icon,
        })
        .collect()
}

#[cfg(test)]
mod value_location_tests {
    use super::*;
    use blockstitch_core::editor::apply_value_kind;
    use blockwork_core::input::types::Coordinate;
    use blockwork_core::input::value::Op;
    use blockwork_core::macros::{
        BlockShape, FieldId, FloatingValue, InputValueType, MacroGraph, VariableDef,
    };

    /// A flat, non-nested `InstrPath` - the shape every location was
    /// addressed by before nested `If`/`IfElse` bodies existed.
    fn top(index: usize) -> Vec<PathStep> {
        vec![PathStep { index, slot: None }]
    }

    fn test_macro() -> Macro {
        Macro {
            id: "m".into(),
            name: "Test".into(),
            description: "".into(),
            graph: MacroGraph {
                strands: vec![Strand {
                    id: "s1".into(),
                    x: 0,
                    y: 0,
                    instructions: vec![
                        Instruction::new(InstructionKind::WhenRan),
                        Instruction::new(InstructionKind::Wait(Value::number(1000.0))),
                    ],
                }],
                floating_values: vec![FloatingValue {
                    id: "f1".into(),
                    x: 10,
                    y: 20,
                    value: Value::number(5.0),
                    origin_block_id: None,
                }],
                ..MacroGraph::new()
            },
            recording_target: None,
            speed_multiplier: 1.0,
            settings: blockwork_core::macros::MacroSettings::default(),
        }
    }

    #[test]
    fn resolves_field_location() {
        let mut mac = test_macro();
        let loc = ValueLocation::Field {
            strand_id: "s1".into(),
            index: top(1),
            field_id: FieldId::WaitDuration.to_string(),
            path: vec![],
        };
        assert_eq!(
            mac.value_at_mut(&loc),
            Some(&mut Value::number(1000.0))
        );
    }

    #[test]
    fn resolves_floating_location() {
        let mut mac = test_macro();
        let loc = ValueLocation::Floating {
            floating_id: "f1".into(),
            path: vec![],
        };
        assert_eq!(
            mac.value_at_mut(&loc),
            Some(&mut Value::number(5.0))
        );
    }

    #[test]
    fn missing_location_resolves_to_none() {
        let mut mac = test_macro();
        let bad_field = ValueLocation::Field {
            strand_id: "nope".into(),
            index: top(0),
            field_id: FieldId::WaitDuration.to_string(),
            path: vec![],
        };
        assert_eq!(mac.value_at_mut(&bad_field), None);
        let bad_floating = ValueLocation::Floating {
            floating_id: "nope".into(),
            path: vec![],
        };
        assert_eq!(mac.value_at_mut(&bad_floating), None);
    }

    #[test]
    fn apply_value_kind_tucks_leaf_away_as_saved() {
        let mut node = Value::number(5.0);
        apply_value_kind(&mut node, "Add", &HashMap::new()).unwrap();
        assert_eq!(
            node,
            Value::Op {
                op: Op::Add,
                args: vec![Value::number(0.0), Value::number(0.0)],
                saved: Box::new(Value::number(5.0))
            }
        );
    }

    #[test]
    fn apply_value_kind_collapses_operator_to_number_best_effort() {
        let mut node = Value::Op {
            op: Op::Add,
            args: vec![Value::number(2.0), Value::number(3.0)],
            saved: Box::new(Value::number(0.0)),
        };
        apply_value_kind(&mut node, "Number", &HashMap::new()).unwrap();
        assert_eq!(node, Value::number(5.0));
    }

    #[test]
    fn apply_value_kind_random_tucks_leaf_away_as_saved() {
        let mut node = Value::number(5.0);
        apply_value_kind(&mut node, "Random", &HashMap::new()).unwrap();
        assert_eq!(
            node,
            Value::Op {
                op: Op::Random,
                args: vec![Value::number(0.0), Value::number(0.0)],
                saved: Box::new(Value::number(5.0))
            }
        );
    }

    #[test]
    fn apply_value_kind_rejects_unknown_kind() {
        let mut node = Value::number(0.0);
        assert!(apply_value_kind(&mut node, "Bogus", &HashMap::new()).is_err());
    }

    #[test]
    fn apply_value_kind_var_becomes_a_plain_leaf() {
        let mut node = Value::number(5.0);
        apply_value_kind(&mut node, "Var:x", &HashMap::new()).unwrap();
        assert_eq!(
            node,
            Value::Var {
                name: "x".to_string()
            }
        );
    }

    #[test]
    fn apply_value_kind_number_best_effort_carries_over_variable_value() {
        let mut node = Value::Var {
            name: "x".to_string(),
        };
        let env = HashMap::from([("x".to_string(), Evaluated::Number(7.0))]);
        apply_value_kind(&mut node, "Number", &env).unwrap();
        assert_eq!(node, Value::number(7.0));
    }

    #[test]
    fn apply_value_kind_join_tucks_leaf_away_as_saved() {
        let mut node = Value::number(5.0);
        apply_value_kind(&mut node, "Join", &HashMap::new()).unwrap();
        assert_eq!(
            node,
            Value::Op {
                op: Op::Join,
                args: vec![
                    Value::Text {
                        value: String::new()
                    },
                    Value::Text {
                        value: String::new()
                    }
                ],
                saved: Box::new(Value::number(5.0)),
            }
        );
    }

    #[test]
    fn apply_value_kind_join3_grows_existing_join_args() {
        let mut node = Value::Op {
            op: Op::Join,
            args: vec![
                Value::Text { value: "a".into() },
                Value::Text { value: "b".into() },
            ],
            saved: Box::new(Value::number(9.0)),
        };
        apply_value_kind(&mut node, "Join3", &HashMap::new()).unwrap();
        assert_eq!(
            node,
            Value::Op {
                op: Op::Join,
                args: vec![
                    Value::Text { value: "a".into() },
                    Value::Text { value: "b".into() },
                    Value::Text {
                        value: String::new()
                    },
                ],
                saved: Box::new(Value::number(9.0)),
            }
        );
    }

    #[test]
    fn apply_value_kind_join_shrinks_existing_join3_args() {
        let mut node = Value::Op {
            op: Op::Join,
            args: vec![
                Value::Text { value: "a".into() },
                Value::Text { value: "b".into() },
                Value::Text { value: "c".into() },
            ],
            saved: Box::new(Value::number(0.0)),
        };
        apply_value_kind(&mut node, "Join", &HashMap::new()).unwrap();
        assert_eq!(
            node,
            Value::Op {
                op: Op::Join,
                args: vec![
                    Value::Text { value: "a".into() },
                    Value::Text { value: "b".into() }
                ],
                saved: Box::new(Value::number(0.0)),
            }
        );
    }

    #[test]
    fn apply_value_kind_new_line_tucks_leaf_away_as_saved_with_no_args() {
        let mut node = Value::number(5.0);
        apply_value_kind(&mut node, "NewLine", &HashMap::new()).unwrap();
        assert_eq!(
            node,
            Value::Op {
                op: Op::NewLine,
                args: vec![],
                saved: Box::new(Value::number(5.0))
            }
        );
    }

    #[test]
    fn apply_value_kind_tab_tucks_leaf_away_as_saved_with_no_args() {
        let mut node = Value::number(5.0);
        apply_value_kind(&mut node, "Tab", &HashMap::new()).unwrap();
        assert_eq!(
            node,
            Value::Op {
                op: Op::Tab,
                args: vec![],
                saved: Box::new(Value::number(5.0))
            }
        );
    }

    #[test]
    fn apply_value_kind_swapping_join_to_new_line_drops_args() {
        let mut node = Value::Op {
            op: Op::Join,
            args: vec![
                Value::Text { value: "a".into() },
                Value::Text { value: "b".into() },
            ],
            saved: Box::new(Value::number(9.0)),
        };
        apply_value_kind(&mut node, "NewLine", &HashMap::new()).unwrap();
        assert_eq!(
            node,
            Value::Op {
                op: Op::NewLine,
                args: vec![],
                saved: Box::new(Value::number(9.0))
            }
        );
    }

    #[test]
    fn apply_value_kind_round_tucks_leaf_away_as_saved_with_one_arg() {
        let mut node = Value::number(5.0);
        apply_value_kind(&mut node, "Round", &HashMap::new()).unwrap();
        assert_eq!(
            node,
            Value::Op {
                op: Op::Round,
                args: vec![Value::number(0.0)],
                saved: Box::new(Value::number(5.0))
            }
        );
    }

    #[test]
    fn apply_value_kind_math_tucks_leaf_away_with_abs_selector() {
        let mut node = Value::number(5.0);
        apply_value_kind(&mut node, "Math", &HashMap::new()).unwrap();
        assert_eq!(
            node,
            Value::Op {
                op: Op::Math,
                args: vec![
                    Value::Text {
                        value: "Abs".into()
                    },
                    Value::number(0.0)
                ],
                saved: Box::new(Value::number(5.0)),
            }
        );
    }

    #[test]
    fn apply_value_kind_case_defaults_second_arg_to_upper() {
        let mut node = Value::number(5.0);
        apply_value_kind(&mut node, "Case", &HashMap::new()).unwrap();
        assert_eq!(
            node,
            Value::Op {
                op: Op::Case,
                args: vec![
                    Value::Text {
                        value: String::new()
                    },
                    Value::Text {
                        value: "Upper".into()
                    }
                ],
                saved: Box::new(Value::number(5.0)),
            }
        );
    }

    #[test]
    fn apply_value_kind_letter_of_grows_from_round_with_mixed_type_default() {
        // Round (arity 1) -> LetterOf (arity 2): the surviving first arg
        // keeps its value untouched, and the newly-added second slot gets
        // LetterOf's own default type (text), not Round's (number).
        let mut node = Value::Op {
            op: Op::Round,
            args: vec![Value::number(7.0)],
            saved: Box::new(Value::number(0.0)),
        };
        apply_value_kind(&mut node, "LetterOf", &HashMap::new()).unwrap();
        assert_eq!(
            node,
            Value::Op {
                op: Op::LetterOf,
                args: vec![
                    Value::number(7.0),
                    Value::Text {
                        value: String::new()
                    }
                ],
                saved: Box::new(Value::number(0.0)),
            }
        );
    }

    #[test]
    fn prune_value_buffers_drops_only_descendants_of_changed_path() {
        let mut buffers: HashMap<ValueLocation, String> = HashMap::new();
        let field = |path: Vec<u8>| ValueLocation::Field {
            strand_id: "s1".into(),
            index: top(1),
            field_id: FieldId::WaitDuration.to_string(),
            path,
        };
        buffers.insert(field(vec![0]), "kept-sibling-subtree-root".into());
        buffers.insert(field(vec![1]), "dropped-descendant".into());
        buffers.insert(field(vec![1, 0]), "dropped-nested-descendant".into());
        buffers.insert(
            ValueLocation::Field {
                strand_id: "s2".into(),
                index: top(1),
                field_id: FieldId::WaitDuration.to_string(),
                path: vec![1],
            },
            "kept-different-strand".into(),
        );

        prune_value_buffers(&mut buffers, &field(vec![1]));

        assert_eq!(buffers.len(), 2);
        assert!(buffers.contains_key(&field(vec![0])));
        assert!(buffers.contains_key(&ValueLocation::Field {
            strand_id: "s2".into(),
            index: top(1),
            field_id: FieldId::WaitDuration.to_string(),
            path: vec![1]
        }));
    }

    #[test]
    fn location_requires_integer_only_for_pixel_fields() {
        assert!(FieldId::MoveMouseX.requires_integer());
        assert!(FieldId::MoveMouseY.requires_integer());
        assert!(FieldId::ScrollAmount.requires_integer());
        assert!(!FieldId::WaitDuration.requires_integer());
        // And the graph looks it up through the instruction a location addresses.
        let mac = test_macro();
        assert!(!mac.location_requires_integer(&ValueLocation::Field {
            strand_id: "s1".into(),
            index: top(1),
            field_id: FieldId::WaitDuration.to_string(),
            path: vec![],
        }));
        assert!(!mac.location_requires_integer(&ValueLocation::Floating {
            floating_id: "f1".into(),
            path: vec![]
        }));
    }

    #[test]
    fn floating_values_round_trip_through_json() {
        let mac = test_macro();
        let json = serde_json::to_string(&mac).unwrap();
        let back: Macro = serde_json::from_str(&json).unwrap();
        assert_eq!(back.floating_values, mac.floating_values);
    }

    #[test]
    fn macro_without_floating_values_key_loads_with_empty_vec() {
        let json = r#"{"id":"m1","name":"Old","description":"","strands":[
            {"id":"s1","x":0,"y":0,"instructions":["WhenRan"]}
        ]}"#;
        let mac: Macro = serde_json::from_str(json).unwrap();
        assert!(mac.floating_values.is_empty());
    }

    #[test]
    fn legacy_bare_number_floating_value_migrates() {
        let json = r#"{"id":"f1","x":10,"y":20,"value":5.0}"#;
        let fv: FloatingValue = serde_json::from_str(json).unwrap();
        assert_eq!(fv.value, Value::number(5.0));
    }

    #[test]
    fn move_mouse_field_still_resolves_after_rework() {
        let mut mac = Macro {
            id: "m".into(),
            name: "T".into(),
            description: "".into(),
            graph: MacroGraph {
                strands: vec![Strand {
                    id: "s1".into(),
                    x: 0,
                    y: 0,
                    instructions: vec![Instruction::new(InstructionKind::Token(
                        blockwork_core::input::types::InputToken::MoveMouse(
                            Value::number(1.0),
                            Value::number(2.0),
                            Coordinate::Rel,
                        ),
                    ))],
                }],
                ..MacroGraph::new()
            },
            recording_target: None,
            speed_multiplier: 1.0,
            settings: blockwork_core::macros::MacroSettings::default(),
        };
        let loc = ValueLocation::Field {
            strand_id: "s1".into(),
            index: top(0),
            field_id: FieldId::MoveMouseY.to_string(),
            path: vec![],
        };
        assert_eq!(
            mac.value_at_mut(&loc),
            Some(&mut Value::number(2.0))
        );
    }

    #[test]
    fn preview_value_stringifies_numeric_result() {
        let dto = Value::Op {
            op: Op::Add,
            args: vec![
                Value::Number { value: 2.0 },
                Value::Number { value: 3.0 },
            ],
            saved: Box::new(Value::Number { value: 0.0 }),
        };
        assert_eq!(
            preview_value_with_env(&dto, &HashMap::new()),
            Ok("5".to_string())
        );
    }

    #[test]
    fn preview_value_joins_text_args() {
        let dto = Value::Op {
            op: Op::Join,
            args: vec![
                Value::Text {
                    value: "foo".into(),
                },
                Value::Text {
                    value: "bar".into(),
                },
            ],
            saved: Box::new(Value::Number { value: 0.0 }),
        };
        assert_eq!(
            preview_value_with_env(&dto, &HashMap::new()),
            Ok("foobar".to_string())
        );
    }

    #[test]
    fn preview_value_surfaces_eval_errors() {
        let dto = Value::Op {
            op: Op::Div,
            args: vec![
                Value::Number { value: 1.0 },
                Value::Number { value: 0.0 },
            ],
            saved: Box::new(Value::Number { value: 0.0 }),
        };
        assert!(preview_value_with_env(&dto, &HashMap::new()).is_err());
    }

    #[test]
    fn create_variable_adds_variable_starting_at_zero() {
        let mut mac = Macro::new("Test".into(), "".into(), vec![]);
        let name = mac.create_variable("score").unwrap();
        assert_eq!(name, "score");
        assert_eq!(
            mac.variables,
            vec![VariableDef {
                name: "score".into(),
                value: Evaluated::Number(0.0)
            }]
        );
    }

    #[test]
    fn create_variable_trims_whitespace() {
        let mut mac = Macro::new("Test".into(), "".into(), vec![]);
        let name = mac.create_variable("  score  ").unwrap();
        assert_eq!(name, "score");
    }

    #[test]
    fn create_variable_rejects_empty_name() {
        let mut mac = Macro::new("Test".into(), "".into(), vec![]);
        assert!(mac.create_variable("   ").is_err());
    }

    #[test]
    fn create_variable_rejects_duplicate_name() {
        let mut mac = Macro::new("Test".into(), "".into(), vec![]);
        mac.create_variable("score").unwrap();
        assert!(mac.create_variable("score").is_err());
        assert_eq!(mac.variables.len(), 1);
    }

    #[test]
    fn rename_variable_renames_and_updates_references() {
        let mut mac = Macro::new(
            "Test".into(),
            "".into(),
            vec![Instruction::new(InstructionKind::SetVariable(
                "score".to_string(),
                Value::number(1.0),
            ))],
        );
        mac.create_variable("score").unwrap();
        let name = mac.rename_variable("score", "points").unwrap();
        assert_eq!(name, "points");
        assert_eq!(mac.variables[0].name, "points");
        assert_eq!(
            mac.strands[0].instructions[1],
            Instruction::new(InstructionKind::SetVariable(
                "points".to_string(),
                Value::number(1.0)
            ))
        );
    }

    #[test]
    fn rename_variable_trims_whitespace() {
        let mut mac = Macro::new("Test".into(), "".into(), vec![]);
        mac.create_variable("score").unwrap();
        let name = mac.rename_variable("score", "  points  ").unwrap();
        assert_eq!(name, "points");
    }

    #[test]
    fn rename_variable_rejects_empty_name() {
        let mut mac = Macro::new("Test".into(), "".into(), vec![]);
        mac.create_variable("score").unwrap();
        assert!(mac.rename_variable("score", "   ").is_err());
    }

    #[test]
    fn rename_variable_rejects_duplicate_name() {
        let mut mac = Macro::new("Test".into(), "".into(), vec![]);
        mac.create_variable("score").unwrap();
        mac.create_variable("points").unwrap();
        assert!(mac.rename_variable("score", "points").is_err());
    }

    #[test]
    fn rename_variable_allows_renaming_to_its_own_current_name() {
        let mut mac = Macro::new("Test".into(), "".into(), vec![]);
        mac.create_variable("score").unwrap();
        assert_eq!(
            mac.rename_variable("score", "score").unwrap(),
            "score"
        );
        assert_eq!(mac.variables.len(), 1);
    }

    #[test]
    fn remove_variable_removes_declaration_but_leaves_references() {
        let mut mac = Macro::new(
            "Test".into(),
            "".into(),
            vec![Instruction::new(InstructionKind::Token(InputToken::Text(
                Value::Var {
                    name: "score".to_string(),
                },
            )))],
        );
        mac.create_variable("score").unwrap();
        mac.remove_variable("score");
        assert!(mac.variables.is_empty());
        assert_eq!(
            mac.strands[0].instructions[1],
            Instruction::new(InstructionKind::Token(InputToken::Text(Value::Var {
                name: "score".to_string()
            })))
        );
    }

    // ─── Custom blocks ("My Blocks") ────────────────────────────────────────

    fn label(text: &str) -> BlockPiece {
        BlockPiece::Label {
            id: format!("id-{text}"),
            text: text.to_string(),
        }
    }
    fn input(id: &str, name: &str) -> BlockPiece {
        BlockPiece::Input {
            id: id.to_string(),
            name: name.to_string(),
            value_type: Default::default(),
        }
    }

    #[test]
    fn validate_pieces_accepts_a_well_formed_prototype() {
        assert!(BlockDef::validate_pieces(&[label("double"), input("i1", "n")]).is_ok());
    }

    #[test]
    fn validate_pieces_rejects_all_blank_labels() {
        assert!(BlockDef::validate_pieces(&[label("  "), input("i1", "n")]).is_err());
    }

    #[test]
    fn validate_pieces_rejects_blank_input_name() {
        assert!(BlockDef::validate_pieces(&[label("double"), input("i1", "  ")]).is_err());
    }

    #[test]
    fn validate_pieces_rejects_duplicate_input_names() {
        assert!(
            BlockDef::validate_pieces(&[label("add"), input("i1", "n"), input("i2", "n")]).is_err()
        );
    }

    #[test]
    fn create_block_appends_def_and_empty_header_strand() {
        let mut mac = Macro::new("Test".into(), "".into(), vec![]);
        let id = mac.create_block(
            vec![label("double"), input("i1", "n")],
            BlockShape::ReturnsValue,
            "#4C97FF".into(),
            100,
            0,
        );
        assert_eq!(mac.block_defs.len(), 1);
        assert_eq!(mac.block_defs[0].id, id);
        assert_eq!(mac.block_defs[0].shape, BlockShape::ReturnsValue);
        let header_strand = mac.strands.iter().find(|s| {
            s.instructions == vec![Instruction::new(InstructionKind::BlockHeader(id.clone()))]
        });
        assert!(header_strand.is_some());
    }

    #[test]
    fn reconcile_block_call_args_preserves_value_across_rename_by_id() {
        let mut mac = Macro::new("Test".into(), "".into(), vec![]);
        let id = mac.create_block(
            vec![input("i1", "a")],
            BlockShape::Normal,
            "#4C97FF".into(),
            0,
            0,
        );
        // A CallBlock call site with one arg bound to the "a" slot.
        mac.strands.push(Strand {
            id: "caller".into(),
            x: 0,
            y: 0,
            instructions: vec![Instruction::new(InstructionKind::CallBlock {
                block_id: id.clone(),
                args: vec![Value::number(5.0)],
                branches: vec![],
            })],
        });
        let old_pieces = mac.block_defs[0].pieces.clone();
        let new_pieces = vec![input("i1", "b")]; // same id, renamed
        mac.reconcile_block_call_args(&id, &old_pieces, &new_pieces);
        let caller = mac.strands.iter().find(|s| s.id == "caller").unwrap();
        let InstructionKind::CallBlock { args, .. } = &caller.instructions[0].kind else {
            panic!("expected CallBlock")
        };
        assert_eq!(args, &vec![Value::number(5.0)]);
    }

    #[test]
    fn reconcile_block_call_args_drops_removed_input_and_keeps_survivor() {
        let mut mac = Macro::new("Test".into(), "".into(), vec![]);
        let id = mac.create_block(
            vec![input("i1", "a"), input("i2", "b")],
            BlockShape::Normal,
            "#4C97FF".into(),
            0,
            0,
        );
        mac.strands.push(Strand {
            id: "caller".into(),
            x: 0,
            y: 0,
            instructions: vec![Instruction::new(InstructionKind::CallBlock {
                block_id: id.clone(),
                args: vec![Value::number(1.0), Value::number(2.0)],
                branches: vec![],
            })],
        });
        let old_pieces = mac.block_defs[0].pieces.clone();
        let new_pieces = vec![input("i2", "b")]; // "a" (i1) removed
        mac.reconcile_block_call_args(&id, &old_pieces, &new_pieces);
        let caller = mac.strands.iter().find(|s| s.id == "caller").unwrap();
        let InstructionKind::CallBlock { args, .. } = &caller.instructions[0].kind else {
            panic!("expected CallBlock")
        };
        assert_eq!(args, &vec![Value::number(2.0)]);
    }

    #[test]
    fn reconcile_block_call_args_defaults_a_newly_added_input_to_zero() {
        let mut mac = Macro::new("Test".into(), "".into(), vec![]);
        let id = mac.create_block(
            vec![input("i1", "a")],
            BlockShape::Normal,
            "#4C97FF".into(),
            0,
            0,
        );
        mac.strands.push(Strand {
            id: "caller".into(),
            x: 0,
            y: 0,
            instructions: vec![Instruction::new(InstructionKind::CallBlock {
                block_id: id.clone(),
                args: vec![Value::number(5.0)],
                branches: vec![],
            })],
        });
        let old_pieces = mac.block_defs[0].pieces.clone();
        let new_pieces = vec![input("i1", "a"), input("i2", "b")]; // "b" newly added
        mac.reconcile_block_call_args(&id, &old_pieces, &new_pieces);
        let caller = mac.strands.iter().find(|s| s.id == "caller").unwrap();
        let InstructionKind::CallBlock { args, .. } = &caller.instructions[0].kind else {
            panic!("expected CallBlock")
        };
        assert_eq!(args, &vec![Value::number(5.0), Value::number(0.0)]);
    }

    #[test]
    fn reconcile_block_call_args_defaults_a_newly_added_bool_input_to_blank_bool() {
        let mut mac = Macro::new("Test".into(), "".into(), vec![]);
        let id = mac.create_block(
            vec![input("i1", "a")],
            BlockShape::Normal,
            "#4C97FF".into(),
            0,
            0,
        );
        mac.strands.push(Strand {
            id: "caller".into(),
            x: 0,
            y: 0,
            instructions: vec![Instruction::new(InstructionKind::CallBlock {
                block_id: id.clone(),
                args: vec![Value::number(5.0)],
                branches: vec![],
            })],
        });
        let old_pieces = mac.block_defs[0].pieces.clone();
        let new_pieces = vec![
            input("i1", "a"),
            BlockPiece::Input {
                id: "i2".into(),
                name: "b".into(),
                value_type: InputValueType::Bool,
            },
        ];
        mac.reconcile_block_call_args(&id, &old_pieces, &new_pieces);
        let caller = mac.strands.iter().find(|s| s.id == "caller").unwrap();
        let InstructionKind::CallBlock { args, .. } = &caller.instructions[0].kind else {
            panic!("expected CallBlock")
        };
        assert_eq!(args, &vec![Value::number(5.0), Value::Bool]);
    }

    #[test]
    fn remove_block_scrubs_call_block_and_call_references() {
        let mut mac = Macro::new("Test".into(), "".into(), vec![]);
        let id = mac.create_block(
            vec![input("i1", "n")],
            BlockShape::ReturnsValue,
            "#4C97FF".into(),
            0,
            0,
        );
        mac.strands.push(Strand {
            id: "caller".into(),
            x: 0,
            y: 0,
            instructions: vec![Instruction::new(InstructionKind::SetVariable(
                "x".to_string(),
                Value::Call {
                    block_id: id.clone(),
                    args: vec![],
                    branches: vec![],
                    saved: Box::new(Value::number(0.0)),
                },
            ))],
        });
        mac.remove_block(&id);
        assert!(mac.block_defs.is_empty());
        assert!(!mac.strands.iter().any(|s| matches!(
            s.instructions.first().map(|i| &i.kind),
            Some(InstructionKind::BlockHeader(_))
        )));
        let caller = mac.strands.iter().find(|s| s.id == "caller").unwrap();
        assert_eq!(
            caller.instructions[0],
            Instruction::new(InstructionKind::SetVariable(
                "x".to_string(),
                Value::number(0.0)
            ))
        );
    }

    /// An unresolved `Call` node must degrade to an ordinary `Err`, never panic.
    #[test]
    fn preview_value_with_env_errors_on_unresolved_call() {
        let dto = Value::Call {
            block_id: "missing".into(),
            args: vec![],
            branches: vec![],
            saved: Box::new(Value::Number { value: 0.0 }),
        };
        assert!(preview_value_with_env(&dto, &HashMap::new()).is_err());
    }

    #[test]
    fn apply_value_kind_number_best_effort_defaults_to_zero_for_unresolved_call() {
        let mut node = Value::Call {
            block_id: "missing".into(),
            args: vec![],
            branches: vec![],
            saved: Box::new(Value::number(0.0)),
        };
        apply_value_kind(&mut node, "Number", &HashMap::new()).unwrap();
        assert_eq!(node, Value::number(0.0));
    }
}
