//! Fires `WhenTime` strands (once per date) and `WhenCron` strands (once
//! per matching minute).

use crate::scheduled_run;
use crate::state::SharedState;
use crate::AppHandle;
use blockwork_core::input::cron::Cron;
use blockwork_core::macros::InstructionKind;
use blockwork_core::run_log;
use chrono::{Local, NaiveDateTime, Timelike};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

/// Poll interval.
const POLL_INTERVAL: Duration = Duration::from_secs(1);

/// Spawns the watcher thread.
pub(crate) fn start(shared_state: SharedState, app: AppHandle) {
    let _ = std::thread::Builder::new().name("time-watch".into()).spawn(move || run(shared_state, app));
}

fn run(shared_state: SharedState, app: AppHandle) {
    // (macro_id, strand_id) -> the minute it last fired. `WhenTime` only
    // compares the date part.
    let mut last_fired: HashMap<(String, String), NaiveDateTime> = HashMap::new();
    // Cron expressions already reported as broken.
    let mut reported: HashMap<(String, String), String> = HashMap::new();

    loop {
        std::thread::sleep(POLL_INTERVAL);

        let now = Local::now();
        let Some(minute) = now.naive_local().with_second(0).and_then(|t| t.with_nanosecond(0)) else { continue };

        let (macros, emulator, speed_multiplier) = {
            let Ok(s) = shared_state.lock() else { continue };
            let Some(emulator) = s.emulator.as_ref().map(Arc::clone) else { continue };
            let macros: Vec<_> = scheduled_run::live_macros(&s).into_iter().cloned().collect();
            (macros, emulator, s.global_speed_multiplier)
        };

        let mut next_last_fired = HashMap::with_capacity(last_fired.len());
        for mac in &macros {
            let env: HashMap<_, _> = mac.variables.iter().map(|v| (v.name.clone(), v.value.clone())).collect();
            for strand in &mac.strands {
                let key = (mac.id.clone(), strand.id.clone());
                let previous = last_fired.get(&key).copied();
                let due = match strand.instructions.first().map(|i| &i.kind) {
                    Some(InstructionKind::WhenTime(schedule)) => {
                        previous.map(|t| t.date()) != Some(minute.date()) && schedule.matches(&now)
                    }
                    Some(InstructionKind::WhenCron(expression)) => {
                        let text = expression.resolve_vars(&env).eval_text().unwrap_or_default();
                        match text.parse::<Cron>() {
                            Ok(cron) => {
                                reported.remove(&key);
                                previous != Some(minute) && cron.matches(&now)
                            }
                            Err(e) => {
                                if reported.get(&key) != Some(&text) {
                                    run_log::error(&run_log::Source::for_macro(&mac.id, &mac.name), format!("Schedule: {e}"));
                                    reported.insert(key.clone(), text);
                                }
                                false
                            }
                        }
                    }
                    _ => continue,
                };
                if due {
                    scheduled_run::fire(mac.id.clone(), strand.instructions[1..].to_vec(), Arc::clone(&emulator), speed_multiplier, &mac.variables, &mac.lists, Arc::clone(&shared_state), app.clone());
                    next_last_fired.insert(key, minute);
                } else if let Some(previous) = previous {
                    next_last_fired.insert(key, previous);
                }
            }
        }
        last_fired = next_last_fired;
    }
}
