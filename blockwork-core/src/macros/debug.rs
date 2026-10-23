//! Breakpoints, pause and single-step for running macros. Every strand calls
//! [`checkpoint`] before each block; it returns straight away unless the
//! macro is paused or the block has a breakpoint.

use crate::input::value::Evaluated;
use crate::macros::runner::VariableStore;
use blockstitch_core::graph::{ListItem, ListStore};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

/// A strand waiting at a block, with what its variables held then.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PausedStrand {
    pub macro_id: String,
    pub block_id: String,
    /// (name, value) for macro variables, then custom block inputs.
    pub variables: Vec<(String, String)>,
    pub lists: Vec<(String, Vec<String>)>,
}

#[derive(Default)]
struct MacroDebug {
    paused: bool,
    /// Bumped by each step; a waiting strand runs one block when it changes.
    step: u64,
}

#[derive(Default)]
struct Debug {
    macros: HashMap<String, MacroDebug>,
    breakpoints: HashSet<(String, String)>,
    waiting: Vec<(u64, PausedStrand)>,
    next_key: u64,
}

static STATE: Mutex<Option<Debug>> = Mutex::new(None);
static WAKE: Condvar = Condvar::new();
/// Set while anything could make a strand stop, so checkpoints stay cheap otherwise.
static ARMED: AtomicBool = AtomicBool::new(false);
static REVISION: AtomicU64 = AtomicU64::new(0);

/// How often a waiting strand checks its stop flag.
const STOP_POLL: Duration = Duration::from_millis(50);

fn lock() -> MutexGuard<'static, Option<Debug>> {
    STATE.lock().unwrap_or_else(|e| e.into_inner())
}

fn changed(debug: &Debug) {
    let armed = !debug.breakpoints.is_empty() || debug.macros.values().any(|m| m.paused);
    ARMED.store(armed, Ordering::Relaxed);
    REVISION.fetch_add(1, Ordering::Relaxed);
    WAKE.notify_all();
}

fn with<R>(f: impl FnOnce(&mut Debug) -> R) -> R {
    let mut guard = lock();
    let debug = guard.get_or_insert_with(Debug::default);
    let result = f(debug);
    changed(debug);
    result
}

/// Bumped whenever breakpoints, pauses or waiting strands change.
pub fn revision() -> u64 {
    REVISION.load(Ordering::Relaxed)
}

pub fn set_breakpoint(macro_id: &str, block_id: &str, on: bool) {
    with(|d| {
        let key = (macro_id.to_string(), block_id.to_string());
        if on {
            d.breakpoints.insert(key);
        } else {
            d.breakpoints.remove(&key);
        }
    });
}

pub fn clear_breakpoints(macro_id: &str) {
    with(|d| d.breakpoints.retain(|(m, _)| m != macro_id));
}

/// Block ids with a breakpoint in `macro_id`.
pub fn breakpoints(macro_id: &str) -> Vec<String> {
    let guard = lock();
    let Some(d) = guard.as_ref() else { return Vec::new() };
    let mut ids: Vec<String> = d.breakpoints.iter().filter(|(m, _)| m == macro_id).map(|(_, b)| b.clone()).collect();
    ids.sort();
    ids
}

/// Every strand of `macro_id` stops at its next block.
pub fn pause(macro_id: &str) {
    with(|d| d.macros.entry(macro_id.to_string()).or_default().paused = true);
}

pub fn resume(macro_id: &str) {
    with(|d| {
        d.macros.remove(macro_id);
    });
}

/// Lets each waiting strand of `macro_id` run one block, then stop again.
pub fn step(macro_id: &str) {
    with(|d| {
        let m = d.macros.entry(macro_id.to_string()).or_default();
        m.paused = true;
        m.step += 1;
    });
}

pub fn is_paused(macro_id: &str) -> bool {
    lock().as_ref().is_some_and(|d| d.macros.get(macro_id).is_some_and(|m| m.paused))
}

/// Strands waiting right now, oldest first.
pub fn paused_strands() -> Vec<PausedStrand> {
    lock().as_ref().map(|d| d.waiting.iter().map(|(_, s)| s.clone()).collect()).unwrap_or_default()
}

/// The last strand of `macro_id` ended, so a pause left behind would only
/// catch the next run.
pub(crate) fn run_ended(macro_id: &str) {
    let mut guard = lock();
    if let Some(d) = guard.as_mut()
        && d.macros.remove(macro_id).is_some()
    {
        changed(d);
    }
}

fn display(value: &Evaluated) -> String {
    let text = match value {
        Evaluated::Number(n) if n.fract() == 0.0 && n.abs() < 1e15 => format!("{n:.0}"),
        Evaluated::Number(n) => n.to_string(),
        Evaluated::Text(t) => t.clone(),
        Evaluated::Bool(b) => b.to_string(),
    };
    crate::secrets::redact(&text)
}

fn snapshot(
    macro_id: &str,
    block_id: &str,
    variables: &VariableStore,
    lists: &ListStore,
    params: &HashMap<String, Evaluated>,
) -> PausedStrand {
    let mut vars: Vec<(String, String)> = variables
        .lock()
        .map(|v| v.iter().map(|(k, v)| (k.clone(), display(v))).collect())
        .unwrap_or_default();
    vars.sort();
    let mut inputs: Vec<(String, String)> = params.iter().map(|(k, v)| (k.clone(), display(v))).collect();
    inputs.sort();
    vars.extend(inputs);
    let mut list_values: Vec<(String, Vec<String>)> = lists
        .lock()
        .map(|l| {
            l.iter()
                .map(|(name, items)| {
                    let items = items
                        .iter()
                        .map(|item| match item {
                            ListItem::Number(n) => display(&Evaluated::Number(*n)),
                            ListItem::Text(t) => crate::secrets::redact(t),
                        })
                        .collect();
                    (name.clone(), items)
                })
                .collect()
        })
        .unwrap_or_default();
    list_values.sort();
    PausedStrand { macro_id: macro_id.to_string(), block_id: block_id.to_string(), variables: vars, lists: list_values }
}

/// Called before each block. Waits while the macro is paused or the block
/// has a breakpoint, until resumed, stepped or `stopped`. Returns how long
/// it waited, so the strand's wait pacing can skip the pause.
pub(crate) fn checkpoint(
    macro_id: &str,
    block_id: &str,
    stopped: &dyn Fn() -> bool,
    variables: &VariableStore,
    lists: &ListStore,
    params: &HashMap<String, Evaluated>,
) -> Duration {
    if !ARMED.load(Ordering::Relaxed) || macro_id.is_empty() {
        return Duration::ZERO;
    }
    let mut guard = lock();
    let Some(d) = guard.as_mut() else { return Duration::ZERO };
    let hit = d.breakpoints.contains(&(macro_id.to_string(), block_id.to_string()));
    let m = d.macros.entry(macro_id.to_string()).or_default();
    if hit {
        m.paused = true;
    }
    if !m.paused {
        return Duration::ZERO;
    }
    let my_step = m.step;
    drop(guard);
    // Read outside the debug lock: the variable store has its own.
    let paused = snapshot(macro_id, block_id, variables, lists, params);

    let started = Instant::now();
    let mut guard = lock();
    let Some(d) = guard.as_mut() else { return Duration::ZERO };
    let key = d.next_key;
    d.next_key += 1;
    d.waiting.push((key, paused));
    changed(d);
    loop {
        let Some(d) = guard.as_ref() else { break };
        let released = d.macros.get(macro_id).is_none_or(|m| !m.paused || m.step != my_step);
        if released || stopped() {
            break;
        }
        guard = WAKE.wait_timeout(guard, STOP_POLL).unwrap_or_else(|e| e.into_inner()).0;
    }
    if let Some(d) = guard.as_mut() {
        d.waiting.retain(|(k, _)| *k != key);
        changed(d);
    }
    started.elapsed()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn stores() -> (VariableStore, ListStore) {
        let vars: VariableStore = Arc::new(Mutex::new(HashMap::from([("n".to_string(), Evaluated::Number(3.0))])));
        (vars, Arc::new(Mutex::new(HashMap::new())))
    }

    fn wait_for(what: impl Fn() -> bool) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while !what() {
            assert!(Instant::now() < deadline, "timed out");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    // One test: the state is process-wide.
    #[test]
    fn breakpoints_pause_and_step_strands() {
        let id = "debug-test";
        let (vars, lists) = stores();
        let none = HashMap::new();
        let never = || false;
        assert_eq!(checkpoint(id, "a", &never, &vars, &lists, &none), Duration::ZERO);

        set_breakpoint(id, "b", true);
        assert_eq!(breakpoints(id), vec!["b".to_string()]);
        let passed = Arc::new(AtomicU64::new(0));
        let worker = {
            let (vars, lists, passed) = (Arc::clone(&vars), Arc::clone(&lists), Arc::clone(&passed));
            std::thread::spawn(move || {
                for block in ["a", "b", "c", "d"] {
                    checkpoint(id, block, &|| false, &vars, &lists, &HashMap::new());
                    passed.fetch_add(1, Ordering::SeqCst);
                }
            })
        };
        // Runs "a", then waits at the breakpoint before "b".
        wait_for(|| paused_strands().iter().any(|s| s.macro_id == id));
        let strand = paused_strands().into_iter().find(|s| s.macro_id == id).unwrap();
        assert_eq!(strand.block_id, "b");
        assert_eq!(strand.variables, vec![("n".to_string(), "3".to_string())]);
        assert_eq!(passed.load(Ordering::SeqCst), 1);
        assert!(is_paused(id));

        // A step runs "b" and stops before "c".
        step(id);
        wait_for(|| paused_strands().iter().any(|s| s.macro_id == id && s.block_id == "c"));
        assert_eq!(passed.load(Ordering::SeqCst), 2);

        resume(id);
        worker.join().unwrap();
        assert_eq!(passed.load(Ordering::SeqCst), 4);

        // A stop releases a waiting strand.
        pause(id);
        let stopped = || true;
        checkpoint(id, "a", &stopped, &vars, &lists, &none);
        run_ended(id);
        assert!(!is_paused(id));
        clear_breakpoints(id);
        assert!(breakpoints(id).is_empty());
    }
}
