//! Starting a macro by name rather than with the editor's Run button: the
//! `Run macro` block and the `blockwork run` command line.

use crate::state::{AppState, SharedState};
use crate::{macros_thread, scheduled_run, AppHandle};
use blockwork_core::macros::calls::MacroHost;
use blockwork_core::macros::run_registry;
use blockwork_core::run_log;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// Serves `Run macro` blocks from the app's macro list.
pub(crate) struct Host {
    pub(crate) state: SharedState,
    pub(crate) app: AppHandle,
}

impl MacroHost for Host {
    fn find(&self, name: &str) -> Option<String> {
        find_id(&*self.state.lock().ok()?, name)
    }

    fn start(&self, id: &str) -> Result<Arc<AtomicBool>, String> {
        start(&self.state, &self.app, id, "Started by a Run macro block")
    }
}

/// The id of the macro called `name`: an exact name, then any case, then an id.
pub(crate) fn find_id(s: &AppState, name: &str) -> Option<String> {
    let name = name.trim();
    let macros = || s.current_macro.iter().chain(s.macros_list.iter());
    macros()
        .find(|m| m.name == name)
        .or_else(|| macros().find(|m| m.name.eq_ignore_ascii_case(name)))
        .or_else(|| macros().find(|m| m.id == name))
        .map(|m| m.id.clone())
}

/// Runs macro `id` on its own thread. The flag is set once the run ends.
pub(crate) fn start(state: &SharedState, app: &AppHandle, id: &str, reason: &str) -> Result<Arc<AtomicBool>, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    // The open copy holds unsaved edits and the live variable store.
    let is_open = s.current_macro.as_ref().is_some_and(|m| m.id == id);
    let mac = s
        .current_macro
        .iter()
        .chain(s.macros_list.iter())
        .find(|m| m.id == id)
        .cloned()
        .ok_or("macro not found")?;
    let emulator = s.emulator.clone().ok_or("input emulation isn't available")?;
    let speed = mac.speed_multiplier * s.global_speed_multiplier;
    let (variables, lists) = if is_open {
        (Arc::clone(&s.variable_values), Arc::clone(&s.list_values))
    } else {
        (
            Arc::new(Mutex::new(mac.variables.iter().map(|v| (v.name.clone(), v.value.clone())).collect::<HashMap<_, _>>())),
            Arc::new(Mutex::new(mac.lists.iter().map(|l| (l.name.clone(), l.items.clone())).collect::<HashMap<_, _>>())),
        )
    };
    let thread_name = format!("called_{}", mac.name);
    let done = Arc::new(AtomicBool::new(false));
    // Registered before the thread starts, so the macro already counts as
    // running when this returns (`blockwork run --wait` polls for that).
    let run_flag = run_registry::begin_macro_run(&mac.id);
    let task = {
        let (done, state, app, reason) = (Arc::clone(&done), Arc::clone(state), app.clone(), reason.to_string());
        let run_flag = Arc::clone(&run_flag);
        move || {
            let id = mac.id.clone();
            let source = run_log::Source::for_macro(&mac.id, &mac.name);
            run_log::run_event(&source, run_log::RunEvent::Started, reason);
            let limit = scheduled_run::limit_run(&source, &run_flag, mac.settings.max_run_secs);
            mac.run_with_lists(emulator, Some(Arc::clone(&run_flag)), speed, Arc::clone(&variables), Arc::clone(&lists));
            limit.store(true, Ordering::Relaxed);
            let stopped = !run_flag.lock().map(|r| *r).unwrap_or(true);
            run_log::run_event(&source, run_log::RunEvent::Ended, if stopped { "Run stopped" } else { "Run finished" });
            run_registry::end_run(&run_flag);
            if is_open {
                macros_thread::persist_variables(&state, &app, &id, &variables, &lists);
            } else {
                scheduled_run::persist_fired_variables(&state, &app, &id, &variables, &lists);
            }
            done.store(true, Ordering::Relaxed);
        }
    };
    if let Err(e) = macros_thread::spawn_macro_thread(&mut s.thread_pool, thread_name, task) {
        run_registry::end_run(&run_flag);
        return Err(e);
    }
    Ok(done)
}

#[derive(Serialize)]
pub(crate) struct MacroInfo {
    name: String,
    running: bool,
}

pub(crate) fn list(state: &SharedState) -> Result<Vec<MacroInfo>, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    Ok(s.macros_list
        .iter()
        .map(|m| MacroInfo { name: m.name.clone(), running: run_registry::is_running(&m.id) })
        .collect())
}

fn id_for(state: &SharedState, name: &str) -> Result<String, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    find_id(&s, name).ok_or_else(|| format!("no macro named \"{}\"", name.trim()))
}

pub(crate) fn run_named(state: &SharedState, app: &AppHandle, name: &str) -> Result<(), String> {
    let id = id_for(state, name)?;
    start(state, app, &id, "Started from the command line").map(|_| ())
}

pub(crate) fn stop_named(state: &SharedState, name: &str) -> Result<usize, String> {
    let id = id_for(state, name)?;
    Ok(run_registry::stop_macro(&id))
}
