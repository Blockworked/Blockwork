//! Macro library: selecting, creating, duplicating and deleting macros, and each macro's settings.

use super::*;
use blockwork_core::config::MacroFiles;

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
    emit_state_updated(app, &s);
    Ok(())
}

pub(crate) fn new_macro(
    state: &SharedState,
    app: &AppHandle,
    new_macro: Macro,
    permissions: Option<Vec<String>>,
) -> Result<(), String> {
    add_and_select_with_permissions(state, app, new_macro, permissions)
}

/// Adds a starter macro from `templates` and opens it.
pub(crate) fn new_macro_from_template(
    state: &SharedState,
    app: &AppHandle,
    template: &str,
) -> Result<(), String> {
    add_and_select(state, app, crate::templates::build(template)?)
}

pub(super) fn add_and_select(
    state: &SharedState,
    app: &AppHandle,
    new_macro: Macro,
) -> Result<(), String> {
    add_and_select_with_permissions(state, app, new_macro, None)
}

fn add_and_select_with_permissions(
    state: &SharedState,
    app: &AppHandle,
    new_macro: Macro,
    permissions: Option<Vec<String>>,
) -> Result<(), String> {
    let new_id = new_macro.id.clone();
    if let Err(e) = new_macro.add() {
        return Err(format!("Failed to create macro: {e}"));
    }
    if let Some(allowed) = permissions {
        let allowed = allowed
            .into_iter()
            .filter(|kind| {
                blockwork_core::permissions::GUARDED
                    .iter()
                    .any(|(known, _)| known == kind)
            })
            .collect();
        blockwork_core::permissions::restrict(&new_id, allowed);
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
    emit_state_updated(app, &s);
    Ok(())
}

/// Opens the macro with id `id`.
pub(crate) fn select_macro_by_id(
    state: &SharedState,
    app: &AppHandle,
    id: &str,
) -> Result<(), String> {
    let index = {
        let s = state.lock().map_err(|e| e.to_string())?;
        s.macros_list
            .iter()
            .position(|m| m.id == id)
            .ok_or("Macro not found")?
    };
    select_macro(state, app, index)
}

/// The freshest copy of macro `id`: the open one holds unsaved edits.
pub(super) fn find_macro(s: &crate::state::AppState, id: &str) -> Result<Macro, String> {
    s.current_macro
        .as_ref()
        .filter(|m| m.id == id)
        .or_else(|| s.macros_list.iter().find(|m| m.id == id))
        .cloned()
        .ok_or_else(|| "Macro not found".to_string())
}

/// Saves a copy of macro `id` next to it and opens the copy.
pub(crate) fn duplicate_macro(
    state: &SharedState,
    app: &AppHandle,
    id: &str,
) -> Result<(), String> {
    let copy = {
        let s = state.lock().map_err(|e| e.to_string())?;
        find_macro(&s, id)?.duplicate()
    };
    blockwork_core::permissions::copy(id, &copy.id);
    add_and_select(state, app, copy)
}

/// Moves macro `id` into `folder` (empty for the top level).
pub(crate) fn set_macro_folder(
    state: &SharedState,
    app: &AppHandle,
    id: &str,
    folder: &str,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    let mut mac = find_macro(&s, id)?;
    mac.settings.folder = blockwork_core::macros::clean_folder(folder);
    mac.save()?;
    if let Some(open) = s.current_macro.as_mut().filter(|m| m.id == id) {
        open.settings.folder = mac.settings.folder.clone();
    }
    refresh_macro_list(&mut s);
    emit_state_updated(app, &s);
    Ok(())
}

/// Deletes macro `id`, closing it if it's open. The UI confirms first.
pub(crate) fn delete_macro(state: &SharedState, app: &AppHandle, id: &str) -> Result<(), String> {
    let is_open = state
        .lock()
        .map_err(|e| e.to_string())?
        .current_macro
        .as_ref()
        .is_some_and(|m| m.id == id);
    if is_open {
        return remove_macro(state, app);
    }
    let mut s = state.lock().map_err(|e| e.to_string())?;
    let mac = find_macro(&s, id)?;
    blockwork_core::macros::debug::clear_breakpoints(id);
    mac.remove()?;
    refresh_macro_list(&mut s);
    emit_state_updated(app, &s);
    Ok(())
}

/// Deletes the current macro. The frontend confirms with the user via a
/// confirmation dialog before ever calling this, so it deletes
/// unconditionally.
pub(crate) fn remove_macro(state: &SharedState, app: &AppHandle) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if let Some(mac) = s.current_macro.take()
        && let Err(e) = mac.remove()
    {
        warn!("Failed to remove macro: {e}");
    }
    s.macro_selected = None;
    s.invalid_field_buffers.clear();
    refresh_macro_list(&mut s);
    config::set_selected_macro_id(None);
    emit_state_updated(app, &s);
    Ok(())
}

pub(crate) fn set_title(state: &SharedState, app: &AppHandle, title: String) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if let Some(mac) = &mut s.current_macro {
        mac.name = title;
        auto_save(&mut s);
        // Rebuild list so dropdown reflects the new name
        let macros = config::get_macros_from_config();
        s.macro_strs = macros.iter().map(|m| m.name.clone()).collect();
        s.macros_list = macros;
    }
    emit_state_updated(app, &s);
    Ok(())
}

pub(crate) fn set_macro_speed_multiplier(
    state: &SharedState,
    app: &AppHandle,
    multiplier: f64,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if let Some(mac) = &mut s.current_macro {
        mac.speed_multiplier = clamp_speed(multiplier);
        auto_save(&mut s);
    }
    emit_state_updated(app, &s);
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
        auto_save(&mut s);
        refresh_macro_list(&mut s);
    }
    emit_state_updated(app, &s);
    Ok(())
}

/// Sets whether the open macro runs schedules it missed - see
/// `MacroSettings::catch_up_missed`.
pub(crate) fn set_macro_catch_up(
    state: &SharedState,
    app: &AppHandle,
    enabled: bool,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if let Some(mac) = &mut s.current_macro {
        mac.settings.catch_up_missed = enabled;
        auto_save(&mut s);
        refresh_macro_list(&mut s);
    }
    emit_state_updated(app, &s);
    Ok(())
}

/// Makes the open macro ask before risky abilities, or trusts it fully.
pub(crate) fn set_macro_restricted(
    state: &SharedState,
    app: &AppHandle,
    restricted: bool,
) -> Result<(), String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    let id = s
        .current_macro
        .as_ref()
        .map(|m| m.id.clone())
        .ok_or("No macro selected")?;
    if restricted {
        if blockwork_core::permissions::grants(&id).is_none() {
            blockwork_core::permissions::restrict(&id, Vec::new());
        }
    } else {
        blockwork_core::permissions::trust(&id);
    }
    emit_state_updated(app, &s);
    Ok(())
}

/// Lets the open, restricted macro use `kind` without asking, or not.
pub(crate) fn set_macro_permission(
    state: &SharedState,
    app: &AppHandle,
    kind: &str,
    allowed: bool,
) -> Result<(), String> {
    if !blockwork_core::permissions::GUARDED
        .iter()
        .any(|(k, _)| *k == kind)
    {
        return Err(format!("unknown permission {kind}"));
    }
    let s = state.lock().map_err(|e| e.to_string())?;
    let id = s
        .current_macro
        .as_ref()
        .map(|m| m.id.clone())
        .ok_or("No macro selected")?;
    blockwork_core::permissions::set_allowed(&id, kind, allowed);
    emit_state_updated(app, &s);
    Ok(())
}

/// Sets when triggers and hotkeys may start the selected macro. Empty
/// times mean all day; one empty time is an error.
pub(crate) fn set_macro_conditions(
    state: &SharedState,
    app: &AppHandle,
    only_in_window: String,
    power: blockwork_core::macros::conditions::PowerCondition,
    active_from: &str,
    active_until: &str,
    active_days: u8,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if let Some(mac) = &mut s.current_macro {
        mac.settings.set_conditions(
            &only_in_window,
            power,
            active_from,
            active_until,
            active_days,
        )?;
        auto_save(&mut s);
        refresh_macro_list(&mut s);
    }
    emit_state_updated(app, &s);
    Ok(())
}

/// Sets how long a run of the selected macro may last; 0 means no limit.
pub(crate) fn set_macro_max_run_secs(
    state: &SharedState,
    app: &AppHandle,
    seconds: u32,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if let Some(mac) = &mut s.current_macro {
        mac.settings.max_run_secs = seconds;
        auto_save(&mut s);
        refresh_macro_list(&mut s);
    }
    emit_state_updated(app, &s);
    Ok(())
}

/// Creates an empty global for every macro's name picker.
pub(crate) fn create_global(
    state: &SharedState,
    app: &AppHandle,
    name: &str,
) -> Result<(), String> {
    blockwork_core::globals::create(name)?;
    let s = state.lock().map_err(|e| e.to_string())?;
    emit_state_updated(app, &s);
    Ok(())
}

pub(crate) fn delete_global(
    state: &SharedState,
    app: &AppHandle,
    name: &str,
) -> Result<(), String> {
    blockwork_core::globals::remove(name);
    let s = state.lock().map_err(|e| e.to_string())?;
    emit_state_updated(app, &s);
    Ok(())
}

/// Sets how many iterations any one loop of the selected macro may run; 0
/// means no limit.
pub(crate) fn set_macro_max_loop_iterations(
    state: &SharedState,
    app: &AppHandle,
    iterations: u32,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if let Some(mac) = &mut s.current_macro {
        mac.settings.max_loop_iterations = iterations;
        auto_save(&mut s);
        refresh_macro_list(&mut s);
    }
    emit_state_updated(app, &s);
    Ok(())
}

/// Kill switch: stops every run, including trigger-fired ones, and pauses
/// the trigger watchers until `resume_triggers`.
pub(crate) fn emergency_stop(state: &SharedState, app: &AppHandle) {
    let cleared = blockwork_core::macros::run_registry::stop_all();
    tracing::info!(cleared, "Emergency stop");
    blockwork_core::run_log::error(
        &blockwork_core::run_log::Source::app("Blockwork"),
        format!("Emergency stop: stopped {cleared} run(s) and paused triggers"),
    );
    if let Ok(mut s) = state.lock() {
        s.triggers_paused = true;
        if let Ok(mut lk) = s.is_looping.lock() {
            *lk = false;
        }
        emit_state_updated(app, &s);
    }
}

/// Re-arms the trigger watchers after an emergency stop.
pub(crate) fn resume_triggers(state: &SharedState, app: &AppHandle) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.triggers_paused = false;
    blockwork_core::run_log::info(
        &blockwork_core::run_log::Source::app("Blockwork"),
        "Triggers resumed",
    );
    emit_state_updated(app, &s);
    Ok(())
}

pub(crate) fn clear_run_log(state: &SharedState, app: &AppHandle) -> Result<(), String> {
    blockwork_core::run_log::clear();
    let s = state.lock().map_err(|e| e.to_string())?;
    emit_state_updated(app, &s);
    Ok(())
}
