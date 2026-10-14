//! Background service that fires `WhenFileChanged` strands when their file
//! is created, modified or deleted. Polls the file's modified time and size,
//! like `clipboard_watch` polls the clipboard.

use crate::AppHandle;
use crate::scheduled_run;
use crate::state::SharedState;
use blockwork_core::macros::InstructionKind;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

const POLL_INTERVAL: Duration = Duration::from_secs(1);

type Stamp = Option<(SystemTime, u64)>;

pub(crate) fn start(shared_state: SharedState, app: AppHandle) {
    let _ = std::thread::Builder::new()
        .name("file-watch".into())
        .spawn(move || run(shared_state, app));
}

fn run(shared_state: SharedState, app: AppHandle) {
    // (macro_id, strand_id, path) -> the file's stamp at the last poll. The
    // path is part of the key so editing it starts a fresh baseline.
    let mut last_seen: HashMap<(String, String, String), Stamp> = HashMap::new();
    loop {
        std::thread::sleep(POLL_INTERVAL);

        let (watched, emulator, speed_multiplier) = {
            let Ok(s) = shared_state.lock() else { continue };
            let Some(emulator) = s.emulator.as_ref().map(Arc::clone) else { continue };
            let mut watched = Vec::new();
            for mac in scheduled_run::live_macros(&s) {
                let env: HashMap<_, _> = mac.variables.iter().map(|v| (v.name.clone(), v.value.clone())).collect();
                for strand in &mac.strands {
                    let Some(InstructionKind::WhenFileChanged(path)) = strand.instructions.first().map(|i| &i.kind) else {
                        continue;
                    };
                    let Ok(path) = path.resolve_vars(&env).eval_text() else { continue };
                    if path.trim().is_empty() {
                        continue;
                    }
                    watched.push(((mac.id.clone(), strand.id.clone(), path), strand.instructions[1..].to_vec(), mac.variables.clone(), mac.lists.clone()));
                }
            }
            (watched, emulator, s.global_speed_multiplier)
        };

        let mut next = HashMap::with_capacity(watched.len());
        for (key, body, variables, lists) in watched {
            let stamp = blockwork_core::files::stamp(&key.2);
            // The first poll only records a baseline.
            if let Some(previous) = last_seen.get(&key)
                && *previous != stamp
            {
                scheduled_run::fire(key.0.clone(), body, Arc::clone(&emulator), speed_multiplier, &variables, &lists, Arc::clone(&shared_state), app.clone());
            }
            next.insert(key, stamp);
        }
        last_seen = next;
    }
}
