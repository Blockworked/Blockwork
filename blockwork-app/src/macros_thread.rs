use crate::state::{build_state_dto, SharedState};
use blockwork_core::macros::backend::InputBackend;
use blockwork_core::macros::run_registry;
use blockwork_core::macros::runner::VariableStore;
use blockstitch_core::graph::ListStore;
use blockwork_core::macros::thread_pool::ThreadPool;
use blockwork_core::macros::Macro;
use std::sync::{Arc, Mutex};
use std::thread;
use crate::AppHandle;
use tracing::warn;

/// Writes the run's final variable values back into the (still-selected)
/// macro and saves it to disk. Called once per run/loop finish rather than
/// per-instruction, and a no-op if the macro was switched away mid-run.
fn persist_variables(shared_state: &SharedState, app: &AppHandle, macro_id: &str, variables: &VariableStore, lists: &ListStore) {
    let (mac_to_save, dto) = {
        let Ok(mut s) = shared_state.lock() else { return };
        let Some(mac) = s.current_macro.as_mut() else { return };
        if mac.id != macro_id {
            return;
        }
        if let Ok(values) = variables.lock() {
            mac.sync_variables_from(&values);
        }
        if let Ok(values) = lists.lock() { mac.sync_lists_from(&values); }
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

pub(crate) fn into_loop_task(
    mac: Macro,
    emulator: Arc<Mutex<dyn InputBackend>>,
    loop_flag: Arc<Mutex<bool>>,
    speed_multiplier: f64,
    variables: VariableStore,
    lists: ListStore,
    shared_state: SharedState,
    app: AppHandle,
) -> impl FnOnce() + Send + 'static {
    move || {
        println!("Starting macro loop: {}", mac.name);
        let macro_id = mac.id.clone();
        run_registry::register(&loop_flag);
        loop {
            if let Ok(should_continue) = loop_flag.lock() {
                if !*should_continue {
                    break;
                }
            } else {
                warn!("Failed to lock loop flag, stopping loop");
                break;
            }

            mac.clone().run_with_lists(Arc::clone(&emulator), Some(Arc::clone(&loop_flag)), speed_multiplier, Arc::clone(&variables), Arc::clone(&lists));

            //todo better solution std::thread::sleep(std::time::Duration::from_millis(1));
        }
        run_registry::end_run(&loop_flag);
        persist_variables(&shared_state, &app, &macro_id, &variables, &lists);
        println!("Macro loop stopped.");
    }
}

pub(crate) fn into_single_run_task(
    mac: Macro,
    emulator: Arc<Mutex<dyn InputBackend>>,
    stop_flag: Arc<Mutex<bool>>,
    speed_multiplier: f64,
    variables: VariableStore,
    lists: ListStore,
    shared_state: SharedState,
    app: AppHandle,
) -> impl FnOnce() + Send + 'static {
    move || {
        println!("Running macro: {}", mac.name);
        let macro_id = mac.id.clone();
        // A stop flag of this run's own, not the shared `is_looping` `stop_flag`
        // refers to - that one gets cleared by whichever run finishes first,
        // which would spuriously cut a concurrently-started run short.
        let run_flag = run_registry::begin_run();
        mac.run_with_lists(emulator, Some(Arc::clone(&run_flag)), speed_multiplier, Arc::clone(&variables), Arc::clone(&lists));
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
