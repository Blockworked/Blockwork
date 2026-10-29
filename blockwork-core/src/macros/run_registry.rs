use std::sync::{Arc, Mutex, OnceLock};

/// Called under the registry lock at run boundaries, in registration order.
/// A host can durably record interrupted runs. The handler must not call
/// back into this module or start/stop macros.
pub type Observer = dyn Fn(&[String]) + Send + Sync;
static OBSERVER: std::sync::RwLock<Option<Arc<Observer>>> = std::sync::RwLock::new(None);

pub fn set_observer(observer: Arc<Observer>) {
    *OBSERVER.write().unwrap_or_else(|e| e.into_inner()) = Some(observer);
}

fn changed(runs: &[(Arc<Mutex<bool>>, String)]) {
    if let Some(observer) = OBSERVER.read().unwrap_or_else(|e| e.into_inner()).as_ref() {
        let ids: Vec<String> = runs.iter().map(|(_, id)| id.clone()).collect();
        observer(&ids);
    }
}

/// Stop flags for every macro run currently executing, so "Stop Macro" can
/// abort *any* run, not just loop-mode ones. Loop runs register their shared
/// `is_looping` flag; single runs register one of their own (see `begin_run`).
/// Each entry is a run's stop flag and the id of its macro ("" when unknown).
static ACTIVE_RUNS: OnceLock<Mutex<Vec<(Arc<Mutex<bool>>, String)>>> = OnceLock::new();

fn active_runs() -> &'static Mutex<Vec<(Arc<Mutex<bool>>, String)>> {
    ACTIVE_RUNS.get_or_init(|| Mutex::new(Vec::new()))
}

/// Registers an existing stop flag (the app's shared `is_looping`) for the duration
/// of a run.
pub fn register(flag: &Arc<Mutex<bool>>) {
    register_for(flag, "");
}

fn register_for(flag: &Arc<Mutex<bool>>, macro_id: &str) {
    if let Ok(mut runs) = active_runs().lock() {
        runs.push((Arc::clone(flag), macro_id.to_string()));
        changed(&runs);
    }
}

/// Creates a fresh registered stop flag for one run. Single runs get their own
/// rather than sharing `is_looping`, which is cleared whenever *any* run
/// finishes and would otherwise cut a concurrently-started run short.
pub fn begin_run() -> Arc<Mutex<bool>> {
    begin_macro_run("")
}

/// [`begin_run`] for a run of the macro `macro_id`, so it can be found by
/// [`is_running`] and [`stop_macro`].
pub fn begin_macro_run(macro_id: &str) -> Arc<Mutex<bool>> {
    let flag = Arc::new(Mutex::new(true));
    register_for(&flag, macro_id);
    flag
}

/// Whether some run of `macro_id` hasn't ended yet.
pub fn is_running(macro_id: &str) -> bool {
    !macro_id.is_empty()
        && active_runs().lock().is_ok_and(|runs| runs.iter().any(|(_, id)| id == macro_id))
}

/// Ids of the macros with a run in flight.
pub fn running_ids() -> Vec<String> {
    let mut ids: Vec<String> = Vec::new();
    if let Ok(runs) = active_runs().lock() {
        for (_, id) in runs.iter() {
            if !id.is_empty() && !ids.contains(id) {
                ids.push(id.clone());
            }
        }
    }
    ids
}

/// How many runs are in flight.
pub fn count() -> usize {
    active_runs().lock().map_or(0, |runs| runs.len())
}

/// Clears the stop flag of every run of `macro_id`, returning how many.
pub fn stop_macro(macro_id: &str) -> usize {
    let mut cleared = 0;
    if let Ok(runs) = active_runs().lock() {
        for (flag, _) in runs.iter().filter(|(_, id)| !macro_id.is_empty() && id == macro_id) {
            if let Ok(mut running) = flag.lock() {
                *running = false;
                cleared += 1;
            }
        }
    }
    cleared
}

pub fn end_run(flag: &Arc<Mutex<bool>>) {
    let mut finished = None;
    if let Ok(mut runs) = active_runs().lock() {
        // Only drops one registration, so concurrent runs sharing a flag
        // do not deregister each other.
        if let Some(pos) = runs.iter().position(|(f, _)| Arc::ptr_eq(f, flag)) {
            let (_, id) = runs.remove(pos);
            if !id.is_empty() && !runs.iter().any(|(_, running)| running == &id) { finished = Some(id); }
            changed(&runs);
        }
    }
    // A host can ask for a trigger grant before any strand/trace exists.
    // Denial/cancellation must still end that run's temporary answers.
    if let Some(id) = finished { crate::permissions::run_ended(&id); }
}

/// Clears every in-flight run's stop flag, returning how many were cleared.
/// `run_block` checks these between instructions and during `Wait`s so runs
/// unwind promptly.
pub fn stop_all() -> usize {
    let mut cleared = 0;
    if let Ok(runs) = active_runs().lock() {
        for (flag, _) in runs.iter() {
            if let Ok(mut running) = flag.lock() {
                *running = false;
                cleared += 1;
            }
        }
    }
    cleared
}

#[cfg(test)]
mod tests {
    use super::*;

    // One test for the whole lifecycle: the registry is process-global, so
    // separate tests would race each other under the default test harness.
    #[test]
    fn stop_all_reaches_registered_runs_and_spares_ended_ones() {
        let running = begin_run();
        let finished = begin_run();
        end_run(&finished);
        if let Ok(mut f) = finished.lock() {
            *f = true;
        }

        stop_all();

        assert!(!*running.lock().unwrap(), "an in-flight run should be stopped");
        assert!(*finished.lock().unwrap(), "a finished run should be deregistered");

        end_run(&running);

        let a = begin_macro_run("macro-a");
        let b = begin_macro_run("macro-b");
        assert!(is_running("macro-a") && !is_running("macro-c") && !is_running(""));
        assert_eq!(stop_macro("macro-a"), 1);
        assert!(!*a.lock().unwrap() && *b.lock().unwrap());
        end_run(&a);
        end_run(&b);
        assert!(!is_running("macro-a"));
        assert!(active_runs().lock().unwrap().is_empty());
    }
}
