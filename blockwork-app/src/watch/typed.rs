//! Text expansion: fires `WhenTyped` strands when what the user just typed
//! ends with their abbreviation. Scoped like the other watchers.

use super::{Target, variable_env};
use crate::AppHandle;
use crate::scheduled_run;
use crate::state::SharedState;
use blockwork_core::input::typed::TypedBuffer;
use blockwork_core::input::types::{Direction, InputToken};
use blockwork_core::input::value::Value;
use blockwork_core::macros::{Instruction, InstructionKind};
use blockwork_core::recording;
use std::sync::Arc;

/// Lets the key that finished the abbreviation come back up before erasing.
const ERASE_DELAY_MS: f64 = 40.0;

pub(crate) fn start(shared_state: SharedState, app: AppHandle) {
    let rx = recording::subscribe_key_events();
    let _ = std::thread::Builder::new()
        .name("type-watch".into())
        .spawn(move || run(shared_state, app, rx));
}

/// `count` backspaces, after a short pause.
fn erase(count: usize) -> Vec<Instruction> {
    std::iter::once(Instruction::new(InstructionKind::Wait(Value::number(
        ERASE_DELAY_MS,
    ))))
    .chain((0..count).map(|_| {
        Instruction::new(InstructionKind::Token(InputToken::Key(
            Value::text("Backspace"),
            Direction::Click,
        )))
    }))
    .collect()
}

fn run(
    shared_state: SharedState,
    app: AppHandle,
    rx: std::sync::mpsc::Receiver<recording::KeyEvent>,
) {
    let mut typed = TypedBuffer::default();
    let mut clicks = recording::click_count();

    while let Ok(event) = rx.recv() {
        // A click may have moved the caret.
        let now_clicks = recording::click_count();
        if now_clicks != clicks {
            clicks = now_clicks;
            typed.clear();
        }
        if !typed.key(&event.key, event.pressed) {
            continue;
        }

        let mut fires = Vec::new();
        let (emulator, speed_multiplier) = {
            let Ok(s) = shared_state.lock() else { continue };
            let Some(emulator) = s.emulator.as_ref().map(Arc::clone) else {
                continue;
            };
            for mac in scheduled_run::live_macros(&s) {
                let env = variable_env(mac);
                for strand in &mac.strands {
                    let Some(InstructionKind::WhenTyped {
                        text,
                        erase: erase_it,
                    }) = strand.instructions.first().map(|i| &i.kind)
                    else {
                        continue;
                    };
                    let Ok(text) = text.resolve_vars(&env).eval_text() else {
                        continue;
                    };
                    if !typed.ends_with(&text) {
                        continue;
                    }
                    let mut body = if *erase_it {
                        erase(text.chars().count())
                    } else {
                        Vec::new()
                    };
                    body.extend_from_slice(&strand.instructions[1..]);
                    fires.push(Target::with_body(mac, strand, body));
                }
            }
            (emulator, s.global_speed_multiplier)
        };
        if fires.is_empty() {
            continue;
        }
        // The expansion's own output isn't typing to match again.
        typed.clear();
        for target in fires {
            target.fire(&emulator, speed_multiplier, &shared_state, &app);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn erasing_waits_then_backspaces_once_per_character() {
        let steps = erase("brb".chars().count());
        assert!(matches!(steps[0].kind, InstructionKind::Wait(_)));
        assert_eq!(steps.len(), 4);
        assert!(steps[1..].iter().all(|s| matches!(
            &s.kind,
            InstructionKind::Token(InputToken::Key(_, Direction::Click))
        )));
    }
}
