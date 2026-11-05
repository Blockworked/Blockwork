//! The debugger and saved secrets.

use super::*;

pub(super) fn open_macro_id(state: &SharedState) -> Result<String, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    s.current_macro.as_ref().map(|m| m.id.clone()).ok_or_else(|| "No macro selected".to_string())
}

/// Turns the breakpoint on block `block_id` of the open macro on or off.
pub(crate) fn toggle_breakpoint(state: &SharedState, block_id: &str) -> Result<(), String> {
    use blockwork_core::macros::debug;
    let id = open_macro_id(state)?;
    let on = !debug::breakpoints(&id).iter().any(|b| b == block_id);
    debug::set_breakpoint(&id, block_id, on);
    Ok(())
}

pub(crate) fn clear_breakpoints(state: &SharedState) -> Result<(), String> {
    blockwork_core::macros::debug::clear_breakpoints(&open_macro_id(state)?);
    Ok(())
}

/// `pause`, `resume`, `step` or `stop` for the open macro's runs.
pub(crate) fn debug_action(state: &SharedState, action: &str) -> Result<(), String> {
    use blockwork_core::macros::{debug, run_registry};
    let id = open_macro_id(state)?;
    match action {
        "pause" => debug::pause(&id),
        "resume" => debug::resume(&id),
        "step" => debug::step(&id),
        "stop" => {
            run_registry::stop_macro(&id);
            debug::resume(&id);
        }
        other => return Err(format!("unknown debugger action {other}")),
    }
    Ok(())
}

// ─── Secrets ───────────────────────────────────────────────────────────────

/// Saves a secret to the keyring. Blocking: the keyring may show an unlock prompt.
pub(crate) fn set_secret(state: &SharedState, app: &AppHandle, name: &str, value: &str) -> Result<(), String> {
    blockwork_core::secrets::set(name, value)?;
    let s = state.lock().map_err(|e| e.to_string())?;
    emit_state_updated(app, &s);
    Ok(())
}

pub(crate) fn delete_secret(state: &SharedState, app: &AppHandle, name: &str) -> Result<(), String> {
    blockwork_core::secrets::delete(name)?;
    let s = state.lock().map_err(|e| e.to_string())?;
    emit_state_updated(app, &s);
    Ok(())
}
