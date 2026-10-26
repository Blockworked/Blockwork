//! Fires `WhenMouseButton` strands on physical mouse button presses and
//! releases (and scrolls). Scoped like the other watchers.

use crate::scheduled_run;
use crate::state::SharedState;
use crate::AppHandle;
use blockwork_core::input::key_names::{match_key, parse_key_combo};
use blockwork_core::macros::InstructionKind;
use blockwork_core::recording;
use std::collections::HashMap;
use std::sync::Arc;

pub(crate) fn start(shared_state: SharedState, app: AppHandle) {
    let rx = recording::subscribe_button_events();
    let _ = std::thread::Builder::new().name("mouse-watch".into()).spawn(move || run(shared_state, app, rx));
}

/// Modifier bits of a combo like `Control+Shift`; `None` when it names a
/// key that isn't a modifier.
fn modifier_bits(combo: &str) -> Option<u8> {
    parse_key_combo(combo).iter().map(match_key).try_fold(0, |bits, key| {
        let bit = key.modifier_bit();
        (bit != 0).then_some(bits | bit)
    })
}

fn run(shared_state: SharedState, app: AppHandle, rx: std::sync::mpsc::Receiver<recording::ButtonEvent>) {
    while let Ok(event) = rx.recv() {
        let mut fires = Vec::new();
        let (emulator, speed_multiplier) = {
            let Ok(s) = shared_state.lock() else { continue };
            let Some(emulator) = s.emulator.as_ref().map(Arc::clone) else { continue };
            for mac in scheduled_run::live_macros(&s) {
                let env: HashMap<_, _> = mac.variables.iter().map(|v| (v.name.clone(), v.value.clone())).collect();
                for strand in &mac.strands {
                    let Some(InstructionKind::WhenMouseButton { button, pressed, modifiers }) =
                        strand.instructions.first().map(|i| &i.kind)
                    else {
                        continue;
                    };
                    if *button != event.button || *pressed != event.pressed {
                        continue;
                    }
                    // Empty means "whatever is held"; otherwise exactly these modifiers.
                    let combo = modifiers.as_ref().and_then(|v| v.resolve_vars(&env).eval_text().ok()).unwrap_or_default();
                    if !combo.trim().is_empty() && modifier_bits(&combo) != Some(event.modifiers) {
                        continue;
                    }
                    fires.push((mac.id.clone(), strand.instructions[1..].to_vec(), mac.variables.clone(), mac.lists.clone()));
                }
            }
            (emulator, s.global_speed_multiplier)
        };
        for (macro_id, body, variables, lists) in fires {
            scheduled_run::fire(macro_id, body, Arc::clone(&emulator), speed_multiplier, &variables, &lists, Arc::clone(&shared_state), app.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use blockwork_core::input::types::MacroKey;

    #[test]
    fn reads_modifier_combos() {
        let ctrl = MacroKey::LControl.modifier_bit();
        let shift = MacroKey::LShift.modifier_bit();
        assert_eq!(modifier_bits("Control"), Some(ctrl));
        assert_eq!(modifier_bits("Control+Shift"), Some(ctrl | shift));
        assert_eq!(modifier_bits("Control+a"), None);
    }
}
