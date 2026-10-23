//! Background service that fires `WhenWindow` strands when a matching window
//! gains or loses focus. Same shape as `clipboard_watch`: poll, compare with
//! the last poll, fire the strand body on a change.

use crate::AppHandle;
use crate::scheduled_run;
use crate::state::SharedState;
use blockwork_core::macros::{Instruction, InstructionKind, VariableDef};
use blockstitch_core::graph::ListDef;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

const POLL_INTERVAL: Duration = Duration::from_millis(500);

struct Watched {
    key: (String, String),
    pattern: String,
    focused: bool,
    body: Vec<Instruction>,
    variables: Vec<VariableDef>,
    lists: Vec<ListDef>,
}

pub(crate) fn start(shared_state: SharedState, app: AppHandle) {
    let _ = std::thread::Builder::new()
        .name("window-watch".into())
        .spawn(move || run(shared_state, app));
}

fn run(shared_state: SharedState, app: AppHandle) {
    // (macro_id, strand_id) -> whether the active window matched last poll.
    let mut last_match: HashMap<(String, String), bool> = HashMap::new();
    let mut last_error: Option<String> = None;
    loop {
        std::thread::sleep(POLL_INTERVAL);

        let (watched, emulator, speed_multiplier) = {
            let Ok(s) = shared_state.lock() else { continue };
            let Some(emulator) = s.emulator.as_ref().map(Arc::clone) else { continue };
            let mut watched = Vec::new();
            for mac in scheduled_run::live_macros(&s) {
                let env: HashMap<_, _> = mac.variables.iter().map(|v| (v.name.clone(), v.value.clone())).collect();
                for strand in &mac.strands {
                    let Some(InstructionKind::WhenWindow { pattern, focused }) = strand.instructions.first().map(|i| &i.kind) else {
                        continue;
                    };
                    watched.push(Watched {
                        key: (mac.id.clone(), strand.id.clone()),
                        pattern: pattern.resolve_vars(&env).eval_text().unwrap_or_default(),
                        focused: *focused,
                        body: strand.instructions[1..].to_vec(),
                        variables: mac.variables.clone(),
                        lists: mac.lists.clone(),
                    });
                }
            }
            (watched, emulator, s.global_speed_multiplier)
        };
        if watched.is_empty() {
            last_match.clear();
            continue;
        }

        let window = match blockwork_core::window::active() {
            Ok(window) => {
                last_error = None;
                window
            }
            Err(e) => {
                // Say once why window triggers can't work here.
                if last_error.as_ref() != Some(&e) {
                    blockwork_core::run_log::error(&blockwork_core::run_log::Source::app("Window trigger"), e.clone());
                    last_error = Some(e);
                }
                continue;
            }
        };

        let mut next = HashMap::with_capacity(watched.len());
        for w in watched {
            let matched = window.matches(&w.pattern, blockwork_core::window::MatchBy::Any);
            // The first poll only records a baseline.
            if let Some(&previous) = last_match.get(&w.key)
                && previous != matched
                && matched == w.focused
            {
                scheduled_run::fire(w.key.0.clone(), w.body, Arc::clone(&emulator), speed_multiplier, &w.variables, &w.lists, Arc::clone(&shared_state), app.clone());
            }
            next.insert(w.key, matched);
        }
        last_match = next;
    }
}
