//! Background service that fires `WhenWindow` strands when a matching window
//! gains or loses focus. Same shape as `watch::clipboard`: poll, compare with
//! the last poll, fire the strand body on a change.

use super::{ReportOnce, StrandKey, Target, fires_on_change, variable_env};
use crate::AppHandle;
use crate::scheduled_run;
use crate::state::SharedState;
use blockwork_core::macros::InstructionKind;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

const POLL_INTERVAL: Duration = Duration::from_millis(500);

struct Watched {
    target: Target,
    pattern: String,
    focused: bool,
}

pub(crate) fn start(shared_state: SharedState, app: AppHandle) {
    let _ = std::thread::Builder::new()
        .name("window-watch".into())
        .spawn(move || run(shared_state, app));
}

fn run(shared_state: SharedState, app: AppHandle) {
    // (macro_id, strand_id) -> whether the active window matched last poll.
    let mut last_match: HashMap<StrandKey, bool> = HashMap::new();
    let mut errors = ReportOnce::default();
    loop {
        std::thread::sleep(POLL_INTERVAL);

        let (watched, emulator, speed_multiplier) = {
            let Ok(s) = shared_state.lock() else { continue };
            let Some(emulator) = s.emulator.as_ref().map(Arc::clone) else {
                continue;
            };
            let mut watched = Vec::new();
            for mac in scheduled_run::live_macros(&s) {
                let env = variable_env(mac);
                for strand in &mac.strands {
                    let Some(InstructionKind::WhenWindow { pattern, focused }) =
                        strand.instructions.first().map(|i| &i.kind)
                    else {
                        continue;
                    };
                    watched.push(Watched {
                        target: Target::new(mac, strand),
                        pattern: pattern.resolve_vars(&env).eval_text().unwrap_or_default(),
                        focused: *focused,
                    });
                }
            }
            (watched, emulator, s.global_speed_multiplier)
        };
        if watched.is_empty() {
            last_match.clear();
            continue;
        }

        // Says once why window triggers can't work here.
        let Some(window) = errors.check("Window trigger", blockwork_core::window::active()) else {
            continue;
        };

        let mut next = HashMap::with_capacity(watched.len());
        for w in watched {
            let matched = window.matches(&w.pattern, blockwork_core::window::MatchBy::Any);
            if fires_on_change(last_match.get(&w.target.key).copied(), matched, w.focused) {
                w.target
                    .fire(&emulator, speed_multiplier, &shared_state, &app);
            }
            next.insert(w.target.key, matched);
        }
        last_match = next;
    }
}
