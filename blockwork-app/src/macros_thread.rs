use crate::AppHandle;
use crate::state::{SharedState, build_state_dto};
use blockstitch_core::graph::ListStore;
use blockwork_core::macros::Macro;
use blockwork_core::macros::backend::InputBackend;
use blockwork_core::macros::run_registry;
use blockwork_core::macros::runner::{RunOptions, VariableStore};
use blockwork_core::macros::thread_pool::ThreadPool;
use std::sync::{Arc, Mutex};
use std::thread;
use tracing::warn;

/// Writes the run's final variable values back into the (still-selected)
/// macro and saves it to disk. Called once per run/loop finish rather than
/// per-instruction, and a no-op if the macro was switched away mid-run.
pub(crate) fn persist_variables(
    shared_state: &SharedState,
    app: &AppHandle,
    macro_id: &str,
    variables: &VariableStore,
    lists: &ListStore,
) {
    let (mac_to_save, dto) = {
        let Ok(mut s) = shared_state.lock() else {
            return;
        };
        let Some(mac) = s.current_macro.as_mut() else {
            return;
        };
        if mac.id != macro_id {
            return;
        }
        if let Ok(values) = variables.lock() {
            mac.sync_variables_from(&values);
        }
        if let Ok(values) = lists.lock() {
            mac.sync_lists_from(&values);
        }
        (mac.clone(), build_state_dto(&s))
    };
    // Saved after the state lock is released: this runs right when an IPC
    // client is likely to fire the next start/run command, which needs this
    // same lock, so holding it through a disk write made timing inconsistent.
    if let Err(e) = mac_to_save.save() {
        warn!("Failed to persist variable values: {e}");
    }
    app.emit_state(&dto);
}

/// One manual or hotkey Run of a macro.
pub(crate) struct RunRequest {
    pub(crate) mac: Macro,
    pub(crate) emulator: Arc<Mutex<dyn InputBackend>>,
    /// The app's `is_looping` flag, cleared when the run ends. The run stops
    /// on a flag of its own (see below).
    pub(crate) is_looping: Arc<Mutex<bool>>,
    pub(crate) speed_multiplier: f64,
    pub(crate) variables: VariableStore,
    pub(crate) lists: ListStore,
    /// Just this strand, from its top block; `None` runs the whole macro.
    pub(crate) strand: Option<String>,
}

pub(crate) fn into_single_run_task(
    request: RunRequest,
    shared_state: SharedState,
    app: AppHandle,
) -> impl FnOnce() + Send + 'static {
    let RunRequest {
        mac,
        emulator,
        is_looping: stop_flag,
        speed_multiplier,
        variables,
        lists,
        strand,
    } = request;
    move || {
        println!("Running macro: {}", mac.name);
        let macro_id = mac.id.clone();
        let source = blockwork_core::run_log::Source::for_macro(&mac.id, &mac.name);
        blockwork_core::run_log::run_event(
            &source,
            blockwork_core::run_log::RunEvent::Started,
            "Run started",
        );
        // A stop flag of this run's own, not the shared `is_looping` `stop_flag`
        // refers to - that one gets cleared by whichever run finishes first,
        // which would spuriously cut a concurrently-started run short.
        let run_flag = run_registry::begin_macro_run(&mac.id);
        let done = crate::scheduled_run::limit_run(&source, &run_flag, mac.settings.max_run_secs);
        let options = RunOptions {
            stop_flag: Some(Arc::clone(&run_flag)),
            speed_multiplier,
            lists: Some(Arc::clone(&lists)),
            ..RunOptions::new(emulator, Arc::clone(&variables))
        };
        match strand {
            // Run strand: just that one, from its top block.
            Some(id) => {
                mac.run_one_strand(&id, options);
            }
            None => mac.run_with(options),
        }
        done.store(true, std::sync::atomic::Ordering::Relaxed);
        let stopped = !run_flag.lock().map(|r| *r).unwrap_or(true);
        blockwork_core::run_log::run_event(
            &source,
            blockwork_core::run_log::RunEvent::Ended,
            if stopped {
                "Run stopped"
            } else {
                "Run finished"
            },
        );
        run_registry::end_run(&run_flag);
        persist_variables(&shared_state, &app, &macro_id, &variables, &lists);
        if let Ok(mut stopped) = stop_flag.lock() {
            *stopped = false;
        }
        println!("Macro complete.");
    }
}

pub(crate) fn spawn_macro_thread<F>(
    thread_pool: &mut ThreadPool,
    name: String,
    task: F,
) -> Result<(), String>
where
    F: FnOnce() + Send + 'static,
{
    let thread_num = thread_pool.workers.len();
    let thread_name = format!("macro_thread_{}: {}", thread_num, name);

    match thread::Builder::new().name(thread_name).spawn(task) {
        Ok(handle) => {
            thread_pool.add_worker(handle);
            thread_pool.cleanup_completed_threads();
            Ok(())
        }
        Err(err) => {
            let error_msg = format!("Failed to spawn thread '{}': {}", name, err);
            warn!("{}", error_msg);
            Err(error_msg)
        }
    }
}
