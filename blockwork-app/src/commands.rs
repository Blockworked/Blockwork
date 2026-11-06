//! Every command the frontend can send. `dispatch.rs` maps command names onto
//! these; each area of the app has a submodule, re-exported here.

use crate::AppHandle;
use crate::macros_thread;
use crate::state::{
    AppState, ComboCapture, EditSession, FragmentDto, HotkeyActionDto, InstructionDto,
    KeyCaptureTarget, ListItemDto, Page, PathStep, RecordingPhase, SharedState, StateDto,
    UpdateCheckState, ValueLocation, build_state_dto, dto_to_fragment, dto_to_hotkey_action,
    dto_to_instruction, emit_state_updated, fragment_to_dto,
};
use blockstitch_core::editor::{
    Created, EditCommand, EditError, EditOutcome, ValueDestination, apply_buffer_effects,
    drop_strand_buffers, prune_value_buffers, retain_live_buffers,
};
use blockstitch_core::graph::{ListItem, ListStore, resolve_list_reporters};
use blockwork_core::config;
use blockwork_core::hotkey_types::{HotkeyAction, HotkeyBinding, KeyCombo};
use blockwork_core::input::types::InputToken;
use blockwork_core::input::value::{Evaluated, Value};
use blockwork_core::macros::runner::VariableStore;
#[cfg(test)]
use blockwork_core::macros::{BlockPiece, Strand};
use blockwork_core::macros::{
    EDIT_CONTEXT, Instruction, InstructionKind, Macro, clamp_speed, loop_control,
};
use blockwork_core::recording;
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use tracing::warn;

/// Checkpoints the canvas before a structural edit, and ends the current
/// text-edit group so the next keystroke starts a fresh step.
pub(crate) fn push_undo(s: &mut AppState) {
    push_undo_for(s, None);
}

/// [`push_undo`] for an edit that coalesces with the keystrokes already in
/// progress at `session` - only the first one checkpoints.
fn push_undo_for(s: &mut AppState, session: Option<EditSession>) {
    let snapshot = s.current_macro.as_ref().map(Macro::undo_snapshot);
    match (snapshot, session) {
        (Some(snapshot), Some(session)) => {
            s.history.push_for_session(snapshot, session);
        }
        (Some(snapshot), None) => s.history.push(snapshot),
        (None, _) => s.history.end_session(),
    }
}

/// Runs one typed edit on the open macro: a rejected or no-op edit costs no
/// undo step and no save; buffers follow the outcome either way.
fn run_edit(
    s: &mut AppState,
    command: EditCommand<InstructionKind>,
) -> Result<EditOutcome, EditError> {
    let mac = s
        .current_macro
        .as_ref()
        .ok_or(EditError::Missing("No macro selected"))?;
    let prepared = mac.prepare_edit(command, &EDIT_CONTEXT)?;
    if prepared.changes() {
        push_undo_for(s, prepared.coalesce().cloned());
    }
    let outcome = match &mut s.current_macro {
        Some(mac) => mac.commit_edit(prepared),
        None => EditOutcome::default(),
    };
    apply_buffer_effects(&mut s.invalid_field_buffers, &outcome.buffers);
    if outcome.changed {
        auto_save(s);
    }
    Ok(outcome)
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
    s.absolute_mouse_position_available =
        result.is_ok() && blockwork_core::macros::backend::absolute_mouse_position_available();
    emit_state_updated(app, &s);
    result
}

fn refresh_macro_list(s: &mut crate::state::AppState) {
    let macros = config::get_macros_from_config();
    s.macro_strs = macros.iter().map(|m| m.name.clone()).collect();
    // Keep current_macro in sync if it was modified
    if let Some(ref cur) = s.current_macro.clone()
        && let Some(updated) = macros.iter().find(|m| m.id == cur.id)
    {
        s.current_macro = Some(updated.clone());
    }
    // Update macro_selected index (name may have changed)
    if let Some(ref cur) = s.current_macro {
        s.macro_selected = macros.iter().position(|m| m.id == cur.id);
    }
    s.macros_list = macros;
}

/// Resyncs the live variable store to `current_macro`'s declared variables -
/// called after `current_macro` is (re)assigned, so stale entries don't linger.
pub(crate) fn sync_variable_values(s: &mut crate::state::AppState) {
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

pub(crate) fn sync_list_values(s: &mut crate::state::AppState) {
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

pub(crate) fn auto_save(s: &mut crate::state::AppState) {
    if let Some(mac) = &mut s.current_macro {
        mac.collect_named_signals();
        // Written on the save thread, so the edit's state goes out first.
        if let Err(e) = mac.save_soon() {
            warn!("Failed to auto-save macro: {e}");
        } else if config::get_selected_macro_id().as_deref() != Some(mac.id.as_str()) {
            config::set_selected_macro_id(Some(&mac.id));
        }
    }
    sync_open_macro_into_list(s);
}

/// Copies the open macro over its `macros_list` entry. Watchers and fired
/// runs read that list, so a stale entry runs (and saves back) old blocks.
pub(crate) fn sync_open_macro_into_list(s: &mut crate::state::AppState) {
    let Some(mac) = &s.current_macro else { return };
    if let Some(entry) = s.macros_list.iter_mut().find(|m| m.id == mac.id) {
        *entry = mac.clone();
    }
}

// ─── Read ──────────────────────────────────────────────────────────────────

pub(crate) fn get_state(state: &SharedState) -> Result<StateDto, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    Ok(build_state_dto(&s))
}

mod blocks;
mod data;
mod debugging;
mod editing;
mod files;
mod hotkey_actions;
mod library;
mod running;
mod settings;
mod strands;
pub(crate) use blocks::*;
pub(crate) use data::*;
pub(crate) use debugging::*;
pub(crate) use editing::*;
pub(crate) use files::*;
pub(crate) use hotkey_actions::*;
pub(crate) use library::*;
pub(crate) use running::*;
pub(crate) use settings::*;
pub(crate) use strands::*;

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
mod value_location_tests;

#[cfg(test)]
mod import_review_tests;

/// Commits a reporter body edit as one undoable document change.
pub(crate) async fn edit_reporter_branch(
    state: &SharedState,
    app: &AppHandle,
    cmd: &str,
    args: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    if matches!(cmd, "add_instruction" | "edit_instruction") {
        let dto: InstructionDto =
            serde_json::from_value(args["instruction"].clone()).map_err(|e| e.to_string())?;
        let instruction = dto_to_instruction(&dto).ok_or("Unknown instruction type")?;
        if matches!(
            instruction.kind,
            InstructionKind::Token(InputToken::MoveMouse(
                _,
                _,
                blockwork_core::input::types::Coordinate::Abs
            ))
        ) {
            request_absolute_mouse_support(state, app).await?;
        }
    }
    let mut s = state.lock().map_err(|e| e.to_string())?;
    let mut graph = s
        .current_macro
        .as_ref()
        .ok_or("No macro selected")?
        .graph
        .clone();
    let result = blockwork_core::frontend::edit_reporter_branch(&mut graph, cmd, args)?;
    if cmd != "copy_instructions" {
        push_undo(&mut s);
        s.current_macro.as_mut().ok_or("No macro selected")?.graph = graph;
        s.invalid_field_buffers.clear();
        auto_save(&mut s);
        emit_state_updated(app, &s);
    }
    Ok(result)
}
