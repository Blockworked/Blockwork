//! Fires `WhenApp`, `WhenIdle` and `WhenScreenLock` strands. Polls like
//! `window_watch`: compare each condition with the last poll and fire the
//! strand when it becomes what the header waits for.

use crate::scheduled_run;
use crate::state::SharedState;
use crate::AppHandle;
use blockwork_core::macros::{Instruction, InstructionKind, VariableDef};
use blockwork_core::run_log;
use blockwork_core::system;
use blockstitch_core::graph::ListDef;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

const POLL_INTERVAL: Duration = Duration::from_secs(1);

enum Condition {
    App { pattern: String, started: bool },
    Idle { secs: f64 },
    Locked(bool),
}

struct Watched {
    key: (String, String),
    condition: Condition,
    body: Vec<Instruction>,
    variables: Vec<VariableDef>,
    lists: Vec<ListDef>,
}

pub(crate) fn start(shared_state: SharedState, app: AppHandle) {
    let _ = std::thread::Builder::new()
        .name("system-watch".into())
        .spawn(move || run(shared_state, app));
}

/// Logs each distinct failure once, so a missing service doesn't flood the log.
#[derive(Default)]
struct Errors(HashMap<&'static str, String>);

impl Errors {
    fn check<T>(&mut self, what: &'static str, result: Result<T, String>) -> Option<T> {
        match result {
            Ok(value) => {
                self.0.remove(what);
                Some(value)
            }
            Err(e) => {
                if self.0.get(what) != Some(&e) {
                    run_log::error(&run_log::Source::app(what), e.clone());
                    self.0.insert(what, e);
                }
                None
            }
        }
    }
}

fn run(shared_state: SharedState, app: AppHandle) {
    // (macro_id, strand_id) -> whether the condition held last poll.
    let mut last: HashMap<(String, String), bool> = HashMap::new();
    let mut errors = Errors::default();
    loop {
        std::thread::sleep(POLL_INTERVAL);

        let (watched, emulator, speed_multiplier) = {
            let Ok(s) = shared_state.lock() else { continue };
            let Some(emulator) = s.emulator.as_ref().map(Arc::clone) else { continue };
            let mut watched = Vec::new();
            for mac in scheduled_run::live_macros(&s) {
                let env: HashMap<_, _> = mac.variables.iter().map(|v| (v.name.clone(), v.value.clone())).collect();
                for strand in &mac.strands {
                    let condition = match strand.instructions.first().map(|i| &i.kind) {
                        Some(InstructionKind::WhenApp { pattern, started }) => Condition::App {
                            pattern: pattern.resolve_vars(&env).eval_text().unwrap_or_default(),
                            started: *started,
                        },
                        Some(InstructionKind::WhenIdle(minutes)) => match minutes.resolve_vars(&env).eval_number() {
                            Ok(m) if m > 0.0 => Condition::Idle { secs: m * 60.0 },
                            _ => continue,
                        },
                        Some(InstructionKind::WhenScreenLock { locked }) => Condition::Locked(*locked),
                        _ => continue,
                    };
                    watched.push(Watched {
                        key: (mac.id.clone(), strand.id.clone()),
                        condition,
                        body: strand.instructions[1..].to_vec(),
                        variables: mac.variables.clone(),
                        lists: mac.lists.clone(),
                    });
                }
            }
            (watched, emulator, s.global_speed_multiplier)
        };
        if watched.is_empty() {
            last.clear();
            continue;
        }

        // Only ask the system for what some trigger needs.
        let wants = |f: fn(&Condition) -> bool| watched.iter().any(|w| f(&w.condition));
        let apps = if wants(|c| matches!(c, Condition::App { .. })) { errors.check("App trigger", system::running_apps()) } else { None };
        let idle = if wants(|c| matches!(c, Condition::Idle { .. })) { errors.check("Idle trigger", system::idle_time()) } else { None };
        let locked = if wants(|c| matches!(c, Condition::Locked(_))) { errors.check("Screen lock trigger", system::screen_locked()) } else { None };

        let mut next = HashMap::with_capacity(watched.len());
        for w in watched {
            let (now, fire_on) = match &w.condition {
                Condition::App { pattern, started } => match &apps {
                    Some(apps) => (system::app_matches(apps, pattern), *started),
                    None => continue,
                },
                Condition::Idle { secs } => match idle {
                    Some(idle) => (idle.as_secs_f64() >= *secs, true),
                    None => continue,
                },
                Condition::Locked(want) => match locked {
                    Some(locked) => (locked, *want),
                    None => continue,
                },
            };
            // The first poll only records a baseline.
            if let Some(&previous) = last.get(&w.key)
                && previous != now
                && now == fire_on
            {
                scheduled_run::fire(w.key.0.clone(), w.body, Arc::clone(&emulator), speed_multiplier, &w.variables, &w.lists, Arc::clone(&shared_state), app.clone());
            }
            next.insert(w.key, now);
        }
        last = next;
    }
}
