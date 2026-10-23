//! Background service that fires `WhenKey` strands. Listens to the capture
//! callback's key events and fires a strand's body (skipping its header) when
//! its combo is pressed or released.
//!
//! Scoping matches `battery_watch`: the selected macro, plus any macro with
//! `MacroSettings::always_listen`.

use crate::scheduled_run;
use crate::state::SharedState;
use crate::AppHandle;
use blockwork_core::input::key_names::{match_key, parse_key_combo};
use blockwork_core::input::types::{Direction, MacroKey};
use blockwork_core::macros::InstructionKind;
use blockwork_core::recording;
use std::collections::HashMap;
use std::sync::Arc;

pub(crate) fn start(shared_state: SharedState, app: AppHandle) {
    let rx = recording::subscribe_key_events();
    let _ = std::thread::Builder::new().name("key-watch".into()).spawn(move || run(shared_state, app, rx));
}

/// Exactly the combo's keys are held, no more and no fewer.
fn combo_matches(combo: &[MacroKey], held: &[MacroKey]) -> bool {
    combo.iter().all(|k| held.contains(k)) && held.iter().all(|k| combo.contains(k))
}

fn run(shared_state: SharedState, app: AppHandle, rx: std::sync::mpsc::Receiver<recording::KeyEvent>) {
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
            let Some(em) = s.emulator.as_ref().map(Arc::clone) else { continue };
            emulator = em;
            speed_multiplier = s.global_speed_multiplier;
            // Edits land in `current_macro`; `macros_list` can lag behind them.
            let selected_id = s.current_macro.as_ref().map(|m| m.id.clone());
            let others = s.macros_list.iter().filter(|m| selected_id.as_deref() != Some(m.id.as_str()));
            for mac in s.current_macro.iter().chain(others) {
                if !mac.settings.always_listen && selected_id.as_deref() != Some(mac.id.as_str()) {
                    continue;
                }
                for strand in &mac.strands {
                    let Some(InstructionKind::WhenKey(value, direction)) = strand.instructions.first().map(|i| &i.kind) else {
                        continue;
                    };
                    let on_press = !matches!(direction, Direction::Release);
                    if on_press != event.pressed {
                        continue;
                    }
                    let env: HashMap<_, _> = mac.variables.iter().map(|v| (v.name.clone(), v.value.clone())).collect();
                    let Ok(text) = value.resolve_vars(&env).eval_text() else { continue };
                    let combo: Vec<MacroKey> = parse_key_combo(&text).iter().map(match_key).collect();
                    if combo.is_empty() || !combo.contains(&key) || !combo_matches(&combo, state_for_match) {
                        continue;
                    }
                    trigger_keys.extend(combo);
                    fires.push((mac.id.clone(), strand.instructions[1..].to_vec(), mac.variables.clone(), mac.lists.clone()));
                }
            }
        }

        if !fires.is_empty() {
            blockwork_core::macros::backend::release_held_keys(&trigger_keys);
        }
        for (macro_id, body, variables, lists) in fires {
            scheduled_run::fire(macro_id, body, Arc::clone(&emulator), speed_multiplier, &variables, &lists, Arc::clone(&shared_state), app.clone());
        }
    }
}
