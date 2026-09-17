//! Fires `WhenTime` strands. Dedups once per date.

use crate::scheduled_run;
use crate::state::SharedState;
use chrono::{Local, NaiveDate};
use blockwork_core::macros::InstructionKind;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use crate::AppHandle;

/// Poll interval.
const POLL_INTERVAL: Duration = Duration::from_secs(1);

/// Spawns the watcher thread.
pub(crate) fn start(shared_state: SharedState, app: AppHandle) {
    let _ = std::thread::Builder::new().name("time-watch".into()).spawn(move || run(shared_state, app));
}

fn run(shared_state: SharedState, app: AppHandle) {
    // (macro_id, strand_id) -> last fired date.
    let mut last_fired: HashMap<(String, String), NaiveDate> = HashMap::new();

    loop {
        std::thread::sleep(POLL_INTERVAL);

        let now = Local::now();
        let today = now.date_naive();

        let (macros, emulator, speed_multiplier, selected_id) = {
            let Ok(s) = shared_state.lock() else { continue };
            let Some(emulator) = s.emulator.as_ref().map(Arc::clone) else { continue };
            let selected_id = s.macro_selected.and_then(|i| s.macros_list.get(i)).map(|m| m.id.clone());
            (s.macros_list.clone(), emulator, s.global_speed_multiplier, selected_id)
        };

        let mut next_last_fired = HashMap::with_capacity(last_fired.len());
        for mac in &macros {
            if !mac.settings.always_listen && selected_id.as_deref() != Some(mac.id.as_str()) {
                continue;
            }
            for strand in &mac.strands {
                let Some(InstructionKind::WhenTime(schedule)) = strand.instructions.first().map(|i| &i.kind) else { continue };

                let key = (mac.id.clone(), strand.id.clone());
                let already_fired_today = last_fired.get(&key) == Some(&today);

                if !already_fired_today && schedule.matches(&now) {
                    scheduled_run::fire(mac.id.clone(), strand.instructions[1..].to_vec(), Arc::clone(&emulator), speed_multiplier, &mac.variables, Arc::clone(&shared_state), app.clone());
                    next_last_fired.insert(key, today);
                } else if already_fired_today {
                    next_last_fired.insert(key, today);
                }
            }
        }
        last_fired = next_last_fired;
    }
}
