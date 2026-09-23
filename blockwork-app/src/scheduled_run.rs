//! Shared "fire a strand outside the normal Run/Loop system" plumbing, used
//! by every background watcher (`battery_watch`, `time_watch`) that fires a
//! strand's body on its own trigger rather than the user pressing Run.

use crate::state::{build_state_dto, SharedState};
use blockwork_core::macros::backend::InputBackend;
use blockwork_core::macros::runner::{run_instructions_with_lists, VariableStore};
use blockstitch_core::graph::ListStore;
use blockwork_core::macros::{run_registry, Instruction, VariableDef};
use blockstitch_core::graph::ListDef;
use std::sync::{Arc, Mutex};
use crate::AppHandle;
use tracing::warn;

/// Runs one watcher-triggered strand's body on its own thread - the
/// watcher's own equivalent of `macros_thread::into_single_run_task`, for a
/// strand fired by a background condition rather than the Run button.
/// Variables start fresh from the macro's own declared defaults (there's no
/// "live" store for a macro that isn't open/running) and are written back to
/// that macro's saved variables once the fired strand finishes, same as a
/// normal run does.
pub(crate) fn fire(
    macro_id: String,
    instructions: Vec<Instruction>,
    emulator: Arc<Mutex<dyn InputBackend>>,
    speed_multiplier: f64,
    variable_defs: &[VariableDef],
    list_defs: &[ListDef],
    shared_state: SharedState,
    app: AppHandle,
) {
    let variables: VariableStore = Arc::new(Mutex::new(variable_defs.iter().map(|v| (v.name.clone(), v.value.clone())).collect()));
    let lists: ListStore = Arc::new(Mutex::new(list_defs.iter().map(|list| (list.name.clone(), list.items.clone())).collect()));
    std::thread::spawn(move || {
        let run_flag = run_registry::begin_run();
        run_instructions_with_lists(instructions, emulator, Some(Arc::clone(&run_flag)), speed_multiplier, Arc::clone(&variables), Arc::clone(&lists));
        run_registry::end_run(&run_flag);
        persist_fired_variables(&shared_state, &app, &macro_id, &variables, &lists);
    });
}

/// Like `macros_thread::persist_variables`, but looks the macro up by id
/// across the whole macro list instead of only the currently-selected one -
/// a watcher-fired macro is very often not the one open in the editor.
fn persist_fired_variables(shared_state: &SharedState, app: &AppHandle, macro_id: &str, variables: &VariableStore, lists: &ListStore) {
    let (mac_to_save, dto) = {
        let Ok(mut s) = shared_state.lock() else { return };
        let Some(mac) = s.macros_list.iter_mut().find(|m| m.id == macro_id) else { return };
        if let Ok(values) = variables.lock() {
            mac.sync_variables_from(&values);
        }
        if let Ok(values) = lists.lock() {
            mac.sync_lists_from(&values);
        }
        let mac_to_save = mac.clone();
        if s.current_macro.as_ref().is_some_and(|c| c.id == macro_id) {
            s.current_macro = Some(mac_to_save.clone());
        }
        (mac_to_save, build_state_dto(&s))
    };
    if let Err(e) = mac_to_save.save() {
        warn!("Failed to persist watcher-fired macro's variables: {e}");
    }
    app.emit_state(&dto);
}
