//! Background service that fires `WhenKey` strands. Listens to the capture
//! callback's key events and fires a strand's body (skipping its header) when
//! its combo is pressed or released.
//!
//! Watches the live macros (`scheduled_run::live_macros`) like every watcher.

use super::{Target, variable_env};
use crate::AppHandle;
use crate::scheduled_run;
use crate::state::SharedState;
use blockwork_core::input::key_names::{match_key, parse_key_combo};
use blockwork_core::input::types::{Direction, MacroKey};
use blockwork_core::macros::InstructionKind;
use blockwork_core::recording;
use std::sync::Arc;

pub(crate) fn start(shared_state: SharedState, app: AppHandle) {
    let rx = recording::subscribe_key_events();
    let _ = std::thread::Builder::new()
        .name("key-watch".into())
        .spawn(move || run(shared_state, app, rx));
}

/// Exactly the combo's keys are held, no more and no fewer.
fn combo_matches(combo: &[MacroKey], held: &[MacroKey]) -> bool {
    combo.iter().all(|k| held.contains(k)) && held.iter().all(|k| combo.contains(k))
}

fn run(
    shared_state: SharedState,
    app: AppHandle,
    rx: std::sync::mpsc::Receiver<recording::KeyEvent>,
) {
    let mut held: Vec<MacroKey> = Vec::new();

    while let Ok(event) = rx.recv() {
        let key = match_key(&event.key);
        let was_held = held.contains(&key);
        // Combo state as it was just before a release, and just after a press.
        let mut before = held.clone();
        if event.pressed {
            if !was_held {
                held.push(key.clone());
            }
        } else {
            held.retain(|k| k != &key);
        }
        if event.pressed && was_held {
            continue;
        }
        if !event.pressed && !before.contains(&key) {
            before.push(key.clone());
        }
        let state_for_match = if event.pressed { &held } else { &before };

        let mut fires = Vec::new();
        let mut trigger_keys: Vec<MacroKey> = Vec::new();
        let emulator;
        let speed_multiplier;
        {
            let Ok(s) = shared_state.lock() else { continue };
            let Some(em) = s.emulator.as_ref().map(Arc::clone) else {
                continue;
            };
            emulator = em;
            speed_multiplier = s.global_speed_multiplier;
            for mac in scheduled_run::live_macros(&s) {
                for strand in &mac.strands {
                    let Some(InstructionKind::WhenKey(value, direction)) =
                        strand.instructions.first().map(|i| &i.kind)
                    else {
                        continue;
                    };
                    let on_press = !matches!(direction, Direction::Release);
                    if on_press != event.pressed {
                        continue;
                    }
                    let Ok(text) = value.resolve_vars(&variable_env(mac)).eval_text() else {
                        continue;
                    };
                    let combo: Vec<MacroKey> =
                        parse_key_combo(&text).iter().map(match_key).collect();
                    if combo.is_empty()
                        || !combo.contains(&key)
                        || !combo_matches(&combo, state_for_match)
                    {
                        continue;
                    }
                    trigger_keys.extend(combo);
                    fires.push(Target::new(mac, strand));
                }
            }
        }

        if !fires.is_empty() {
            blockwork_core::macros::backend::release_held_keys(&trigger_keys);
        }
        for target in fires {
            target.fire(&emulator, speed_multiplier, &shared_state, &app);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_combo_matches_exactly_the_keys_held() {
        let ctrl_alt = [MacroKey::LControl, MacroKey::LMenu];
        assert!(
            combo_matches(&ctrl_alt, &[MacroKey::LMenu, MacroKey::LControl]),
            "in any order"
        );
        assert!(!combo_matches(&ctrl_alt, &[MacroKey::LControl]), "too few");
        assert!(
            !combo_matches(
                &ctrl_alt,
                &[MacroKey::LControl, MacroKey::LShift, MacroKey::LMenu]
            ),
            "too many"
        );
    }
}
