//! Starting a macro by name rather than with the editor's Run button: the
//! `Run macro` block and the `blockwork run` command line.

use crate::state::{AppState, SharedState};
use crate::{AppHandle, macros_thread, scheduled_run};
use blockwork_core::macros::Run;
use blockwork_core::macros::calls::MacroHost;
use blockwork_core::macros::run_registry;
use blockwork_core::macros::runner::RunOptions;
use blockwork_core::run_log;
use serde::Serialize;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

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
pub(crate) fn start(
    state: &SharedState,
    app: &AppHandle,
    id: &str,
    reason: &str,
) -> Result<Arc<AtomicBool>, String> {
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
    let saved = if is_open { Saved::Open } else { Saved::Library };
    start_macro(&mut s, state, app, mac, saved, reason)
}

/// Where a run's variables are kept once it ends.
#[derive(Clone, Copy, PartialEq)]
enum Saved {
    /// The open macro: its live stores, saved like an editor run.
    Open,
    /// A macro in the library, saved back to it.
    Library,
    /// A macro from a file that isn't in the library: nowhere.
    Nowhere,
}

fn start_macro(
    s: &mut AppState,
    state: &SharedState,
    app: &AppHandle,
    mac: blockwork_core::macros::Macro,
    saved: Saved,
    reason: &str,
) -> Result<Arc<AtomicBool>, String> {
    let is_open = saved == Saved::Open;
    let emulator = s
        .emulator
        .clone()
        .ok_or("input emulation isn't available")?;
    let speed = mac.speed_multiplier * s.global_speed_multiplier;
    let (variables, lists, dicts) = if is_open {
        (
            Arc::clone(&s.variable_values),
            Arc::clone(&s.list_values),
            Arc::clone(&s.dict_values),
        )
    } else {
        (mac.variable_store(), mac.list_store(), mac.dict_store())
    };
    let thread_name = format!("called_{}", mac.name);
    let done = Arc::new(AtomicBool::new(false));
    // Registered before the thread starts, so the macro already counts as
    // running when this returns (`blockwork run --wait` polls for that).
    let run_flag = run_registry::begin_macro_run(&mac.id);
    let task = {
        let (done, state, app, reason) = (
            Arc::clone(&done),
            Arc::clone(state),
            app.clone(),
            reason.to_string(),
        );
        let run_flag = Arc::clone(&run_flag);
        move || {
            let id = mac.id.clone();
            let source = run_log::Source::for_macro(&mac.id, &mac.name);
            run_log::run_event(&source, run_log::RunEvent::Started, reason);
            let limit = scheduled_run::limit_run(&source, &run_flag, mac.settings.max_run_secs);
            mac.run_with(RunOptions {
                stop_flag: Some(Arc::clone(&run_flag)),
                speed_multiplier: speed,
                lists: Some(Arc::clone(&lists)),
                dicts: Some(Arc::clone(&dicts)),
                ..RunOptions::new(emulator, Arc::clone(&variables))
            });
            limit.store(true, Ordering::Relaxed);
            let stopped = !run_flag.lock().map(|r| *r).unwrap_or(true);
            run_log::run_event(
                &source,
                run_log::RunEvent::Ended,
                if stopped {
                    "Run stopped"
                } else {
                    "Run finished"
                },
            );
            run_registry::end_run(&run_flag);
            match saved {
                Saved::Open => {
                    macros_thread::persist_variables(&state, &app, &id, &variables, &lists, &dicts)
                }
                Saved::Library => scheduled_run::persist_fired_variables(
                    &state, &app, &id, &variables, &lists, &dicts,
                ),
                Saved::Nowhere => {}
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
        .map(|m| MacroInfo {
            name: m.name.clone(),
            running: run_registry::is_running(&m.id),
        })
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

/// Runs the first macro in a macro or bundle file without adding it to the
/// library. Returns the run's id, for `macro_running`.
pub(crate) fn run_file(state: &SharedState, app: &AppHandle, path: &str) -> Result<String, String> {
    let file = blockwork_core::config::persistence::read_import_file(std::path::Path::new(path))?;
    let mut mac = file
        .macros
        .into_iter()
        .next()
        .ok_or("This file holds no macros")?;
    // A fresh id, so the run never touches a library macro with the same one.
    mac.id = uuid::Uuid::new_v4().to_string();
    let id = mac.id.clone();
    let mut s = state.lock().map_err(|e| e.to_string())?;
    start_macro(
        &mut s,
        state,
        app,
        mac,
        Saved::Nowhere,
        "Started from a file on the command line",
    )?;
    Ok(id)
}

/// Whether the macro with this id is running, for `blockwork run file --wait`.
pub(crate) fn running(id: &str) -> bool {
    run_registry::is_running(id)
}

#[cfg(test)]
mod tests {
    use blockwork_core::macros::Macro;

    #[test]
    fn run_file_reads_the_macro_and_keeps_it_out_of_the_library() {
        let dir = std::env::temp_dir().join(format!("blockwork-runfile-{}", uuid::Uuid::new_v4()));
        let backend = crate::Backend::for_tests(dir.clone());
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("hello.macro");
        let mac = Macro::new("Hello".into(), "".into(), vec![]);
        blockwork_core::config::write_macro_file(&path, &mac).unwrap();
        let not_macro = dir.join("notes.txt");
        std::fs::write(&not_macro, "hi").unwrap();

        assert!(
            super::run_file(&backend.state, &backend.app, &not_macro.to_string_lossy()).is_err()
        );
        // The test backend has no input emulator, so the run stops there,
        // after the file was read.
        let err =
            super::run_file(&backend.state, &backend.app, &path.to_string_lossy()).unwrap_err();
        assert!(err.contains("input emulation"), "{err}");
        assert!(backend.state.lock().unwrap().macros_list.is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }
}
