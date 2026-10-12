use std::sync::{Arc, Mutex, OnceLock};

/// Stop flags for every macro run currently executing, so "Stop Loop" can
/// abort *any* run, not just loop-mode ones. Loop runs register their shared
/// `is_looping` flag; single runs register one of their own (see `begin_run`).
static ACTIVE_RUNS: OnceLock<Mutex<Vec<Arc<Mutex<bool>>>>> = OnceLock::new();

fn active_runs() -> &'static Mutex<Vec<Arc<Mutex<bool>>>> {
    ACTIVE_RUNS.get_or_init(|| Mutex::new(Vec::new()))
}

/// Registers an existing stop flag (the app's shared `is_looping`) for the duration
/// of a run.
pub fn register(flag: &Arc<Mutex<bool>>) {
    if let Ok(mut runs) = active_runs().lock() {
        runs.push(Arc::clone(flag));
    }
}

/// Creates a fresh registered stop flag for one run. Single runs get their own
/// rather than sharing `is_looping`, which is cleared whenever *any* run
/// finishes and would otherwise cut a concurrently-started run short.
pub fn begin_run() -> Arc<Mutex<bool>> {
    let flag = Arc::new(Mutex::new(true));
    register(&flag);
    flag
}

pub fn end_run(flag: &Arc<Mutex<bool>>) {
    if let Ok(mut runs) = active_runs().lock() {
        // Only drops one registration, so two concurrent runs sharing a flag
        // (the app's shared `is_looping`) don't deregister each other.
        if let Some(pos) = runs.iter().position(|f| Arc::ptr_eq(f, flag)) {
            runs.remove(pos);
        }
    }
}

/// Clears every in-flight run's stop flag, returning how many were cleared.
/// `run_block` checks these between instructions and during `Wait`s so runs
/// unwind promptly.
pub fn stop_all() -> usize {
    let mut cleared = 0;
    if let Ok(runs) = active_runs().lock() {
        for flag in runs.iter() {
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
        assert!(active_runs().lock().unwrap().is_empty());
    }
}
