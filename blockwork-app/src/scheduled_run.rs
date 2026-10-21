//! Shared "fire a strand outside the normal Run/Loop system" plumbing, used
//! by every background watcher (`battery_watch`, `time_watch`, etc.) that fires a
//! strand's body on its own trigger rather than the user pressing Run.

use crate::state::{build_state_dto, SharedState};
use blockwork_core::input::value::Evaluated;
use blockwork_core::macros::backend::InputBackend;
use blockwork_core::macros::runner::{run_fired_strand, VariableStore};
use blockwork_core::run_log;
use blockstitch_core::graph::ListStore;
use blockwork_core::macros::{run_registry, Instruction, VariableDef};
use blockstitch_core::graph::ListDef;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
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
    fire_with_input(macro_id, instructions, emulator, speed_multiplier, variable_defs, list_defs, shared_state, app, None)
}

/// `fire`, where `input` is a variable the trigger filled for this run only
/// (a webhook's request body): it's put back to the given value before the
/// run's variables are saved, so the input never lands in the macro file.
#[allow(clippy::too_many_arguments)]
pub(crate) fn fire_with_input(
    macro_id: String,
    instructions: Vec<Instruction>,
    emulator: Arc<Mutex<dyn InputBackend>>,
    speed_multiplier: f64,
    variable_defs: &[VariableDef],
    list_defs: &[ListDef],
    shared_state: SharedState,
    app: AppHandle,
    input: Option<(String, Evaluated)>,
) {
    let variables: VariableStore = Arc::new(Mutex::new(variable_defs.iter().map(|v| (v.name.clone(), v.value.clone())).collect()));
    let lists: ListStore = Arc::new(Mutex::new(list_defs.iter().map(|list| (list.name.clone(), list.items.clone())).collect()));
    std::thread::spawn(move || {
        // Looked up here, off the watcher's thread, which may hold the lock.
        let (source, max_secs, max_loops, block_table, settings) = {
            let Ok(s) = shared_state.lock() else { return };
            if s.triggers_paused {
                return;
            }
            // Prefer the open copy, which holds unsaved block edits.
            let mac = s.current_macro.as_ref().filter(|m| m.id == macro_id).or_else(|| s.macros_list.iter().find(|m| m.id == macro_id));
            (
                run_log::Source::for_macro(&macro_id, mac.map(|m| m.name.as_str()).unwrap_or_default()),
                mac.map_or(0, |m| m.settings.max_run_secs),
                mac.map_or(0, |m| m.settings.max_loop_iterations),
                mac.map(|m| m.block_table()).unwrap_or_default(),
                mac.map(|m| m.settings.clone()).unwrap_or_default(),
            )
        };
        // Conditions may ask the window system, so checked unlocked.
        if settings.blocked_by().is_some() {
            return;
        }
        run_log::run_event(&source, run_log::RunEvent::Fired, "Trigger fired");
        let run_flag = run_registry::begin_macro_run(&macro_id);
        let done = limit_run(&source, &run_flag, max_secs);
        run_fired_strand(source, instructions, emulator, Some(Arc::clone(&run_flag)), speed_multiplier, Arc::clone(&variables), Arc::clone(&lists), block_table, max_loops);
        done.store(true, Ordering::Relaxed);
        run_registry::end_run(&run_flag);
        if let Some((name, value)) = input
            && let Ok(mut vars) = variables.lock()
        {
            vars.insert(name, value);
        }
        persist_fired_variables(&shared_state, &app, &macro_id, &variables, &lists);
    });
}

/// Stops the run behind `flag` once it has lasted `secs` seconds (0 means
/// never). Set the returned flag when the run ends.
pub(crate) fn limit_run(source: &run_log::Source, flag: &Arc<Mutex<bool>>, secs: u32) -> Arc<AtomicBool> {
    let done = Arc::new(AtomicBool::new(false));
    if secs == 0 {
        return done;
    }
    let (source, flag, finished) = (source.clone(), Arc::clone(flag), Arc::clone(&done));
    std::thread::spawn(move || {
        const TICK: Duration = Duration::from_millis(100);
        let mut deadline = Instant::now() + Duration::from_secs(secs.into());
        while Instant::now() < deadline {
            if finished.load(Ordering::Relaxed) {
                return;
            }
            std::thread::sleep(TICK);
            // Time paused in the debugger doesn't count.
            if blockwork_core::macros::debug::is_paused(&source.macro_id) {
                deadline += TICK;
            }
        }
        if !finished.load(Ordering::Relaxed)
            && let Ok(mut running) = flag.lock()
            && *running
        {
            *running = false;
            run_log::error(&source, format!("Stopped after its {secs}s time limit"));
        }
    });
    done
}

/// Macros whose trigger strands are live: the open one (edits land there
/// first) plus any with `always_listen` set.
pub(crate) fn live_macros(s: &crate::state::AppState) -> Vec<&blockwork_core::macros::Macro> {
    let selected = s.current_macro.as_ref().map(|m| m.id.as_str());
    s.current_macro
        .iter()
        .chain(s.macros_list.iter().filter(|m| Some(m.id.as_str()) != selected && m.settings.always_listen))
        .collect()
}

/// Like `macros_thread::persist_variables`, but looks the macro up by id
/// across the whole macro list instead of only the currently-selected one -
/// a watcher-fired macro is very often not the one open in the editor.
pub(crate) fn persist_fired_variables(shared_state: &SharedState, app: &AppHandle, macro_id: &str, variables: &VariableStore, lists: &ListStore) {
    let (mac_to_save, dto) = {
        let Ok(mut s) = shared_state.lock() else { return };
        // Merge values into the open copy rather than replacing it, so edits
        // made since the last save (or during the run) survive.
        let open = s.current_macro.as_mut().filter(|m| m.id == macro_id);
        let mac = match open {
            Some(mac) => mac,
            None => match s.macros_list.iter_mut().find(|m| m.id == macro_id) {
                Some(mac) => mac,
                None => return,
            },
        };
        if let Ok(values) = variables.lock() {
            mac.sync_variables_from(&values);
        }
        if let Ok(values) = lists.lock() {
            mac.sync_lists_from(&values);
        }
        let mac_to_save = mac.clone();
        if let Some(entry) = s.macros_list.iter_mut().find(|m| m.id == macro_id) {
            *entry = mac_to_save.clone();
        }
        (mac_to_save, build_state_dto(&s))
    };
    if let Err(e) = mac_to_save.save() {
        warn!("Failed to persist watcher-fired macro's variables: {e}");
    }
    app.emit_state(&dto);
}
