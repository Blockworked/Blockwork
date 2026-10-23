//! Background service that fires `WhenClipboardChanged` strands on their own,
//! independent of Run/Loop - see `battery_watch` for why (same mechanism,
//! same reasoning, just a clipboard-content condition instead of a battery
//! one). Polls the clipboard on a timer and fires a strand's body directly
//! (skipping its header) the moment its contents differ from the last poll.
//!
//! Only the currently selected macro's strands are watched, plus any macro
//! whose `MacroSettings::always_listen` is set - see `battery_watch`'s module
//! doc for the same scoping rule.
//!
//! Unlike the battery watcher's threshold-crossing hysteresis, there's no
//! condition to recover past here - any difference from the previous poll's
//! fingerprint fires. A strand's very first observation only records a
//! baseline rather than firing, so opening a macro that's always had *some*
//! clipboard content doesn't immediately fire just because this watcher has
//! never looked before.

use crate::scheduled_run;
use crate::state::SharedState;
use blockwork_core::macros::InstructionKind;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use crate::AppHandle;

/// How often the watcher re-reads the clipboard and re-checks every macro's
/// strands. Fast enough to feel immediate after a copy, still cheap - unlike
/// the battery watcher's 5s poll, a clipboard read is trivial and this is a
/// copy-triggered workflow where responsiveness matters.
const POLL_INTERVAL: Duration = Duration::from_millis(500);

/// Cheap snapshot of "what's on the clipboard right now", compared tick to
/// tick to decide whether it changed. Only as detailed as `WhenClipboardChanged`
/// needs - an actual content diff, not just presence/absence.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Fingerprint {
    text: Option<String>,
    has_image: bool,
    has_file_list: bool,
}

fn snapshot() -> Fingerprint {
    Fingerprint {
        text: blockwork_core::clipboard::get_text().ok(),
        has_image: blockwork_core::clipboard::has_image(),
        has_file_list: blockwork_core::clipboard::has_file_list(),
    }
}

/// Spawns the watcher thread. Runs for the lifetime of the app; there's no
/// handle to stop it since it only ever does anything when a macro actually
/// declares a `WhenClipboardChanged` block.
pub(crate) fn start(shared_state: SharedState, app: AppHandle) {
    let _ = std::thread::Builder::new().name("clipboard-watch".into()).spawn(move || run(shared_state, app));
}

fn run(shared_state: SharedState, app: AppHandle) {
    // (macro_id, strand_id) -> the clipboard fingerprint last observed for
    // this strand. Rebuilt fresh each tick from whatever strands currently
    // exist, carrying over the prior value by key - so a deleted
    // strand/macro just quietly drops out instead of leaking.
    let mut last_seen: HashMap<(String, String), Fingerprint> = HashMap::new();

    loop {
        std::thread::sleep(POLL_INTERVAL);

        let current = snapshot();

        let (macros, emulator, speed_multiplier, selected_id) = {
            let Ok(s) = shared_state.lock() else { continue };
            let Some(emulator) = s.emulator.as_ref().map(Arc::clone) else { continue };
            let selected_id = s.macro_selected.and_then(|i| s.macros_list.get(i)).map(|m| m.id.clone());
            (s.macros_list.clone(), emulator, s.global_speed_multiplier, selected_id)
        };

        let mut next_last_seen = HashMap::with_capacity(last_seen.len());
        for mac in &macros {
            // By default only the selected macro's event strands are live -
            // `always_listen` opts a macro into being watched regardless of
            // what's currently open.
            if !mac.settings.always_listen && selected_id.as_deref() != Some(mac.id.as_str()) {
                continue;
            }
            for strand in &mac.strands {
                if !matches!(strand.instructions.first().map(|i| &i.kind), Some(InstructionKind::WhenClipboardChanged)) {
                    continue;
                }

                let key = (mac.id.clone(), strand.id.clone());
                if let Some(previous) = last_seen.get(&key) {
                    if *previous != current {
                        scheduled_run::fire(mac.id.clone(), strand.instructions[1..].to_vec(), Arc::clone(&emulator), speed_multiplier, &mac.variables, &mac.lists, Arc::clone(&shared_state), app.clone());
                    }
                }
                next_last_seen.insert(key, current.clone());
            }
        }
        last_seen = next_last_seen;
    }
}
