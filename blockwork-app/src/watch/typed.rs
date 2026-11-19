//! Text expansion: fires `WhenTyped` strands when what the user just typed
//! ends with their abbreviation. Scoped like the other watchers.

use super::{OnlyWhen, Target, holds, variable_env};
use crate::AppHandle;
use crate::scheduled_run;
use crate::state::SharedState;
use blockwork_core::input::typed::TypedBuffer;
use blockwork_core::input::types::{Direction, InputToken};
use blockwork_core::input::value::Value;
use blockwork_core::macros::{Instruction, InstructionKind, TypedWord};
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

/// Characters that end a word, for the word options.
fn ends_word(c: char) -> bool {
    c.is_whitespace() || c.is_ascii_punctuation()
}

/// Whether what was typed ends with `abbreviation` as `word` asks. `Some`
/// holds the character typed after a whole word (to type again once the
/// abbreviation is erased), or `None` when nothing follows it.
fn typed_match(
    typed: &str,
    abbreviation: &str,
    ignore_case: bool,
    word: Option<TypedWord>,
) -> Option<Option<char>> {
    if abbreviation.is_empty() {
        return None;
    }
    let (typed, abbreviation) = if ignore_case {
        (typed.to_lowercase(), abbreviation.to_lowercase())
    } else {
        (typed.to_string(), abbreviation.to_string())
    };
    let (rest, end) = match word {
        Some(TypedWord::Whole) => {
            let end = typed.chars().last().filter(|c| ends_word(*c))?;
            (&typed[..typed.len() - end.len_utf8()], Some(end))
        }
        _ => (typed.as_str(), None),
    };
    let before = rest.strip_suffix(abbreviation.as_str())?;
    // Typing just started (or the caret moved) counts as a word's start.
    if word.is_some() && before.chars().last().is_some_and(|c| !ends_word(c)) {
        return None;
    }
    Some(end)
}

/// Types `c` again: the space, Enter or punctuation that ended a whole word.
fn retype(c: char) -> Instruction {
    let token = match c {
        '\n' => InputToken::Key(Value::text("Enter"), Direction::Click),
        '\t' => InputToken::Key(Value::text("Tab"), Direction::Click),
        c => InputToken::Text(Value::text(c.to_string()).into()),
    };
    Instruction::new(InstructionKind::Token(token))
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
                        ignore_case,
                        word,
                        only_when,
                    }) = strand.instructions.first().map(|i| &i.kind)
                    else {
                        continue;
                    };
                    let Ok(text) = text.resolve_vars(&env).eval_text() else {
                        continue;
                    };
                    let Some(end) = typed_match(typed.text(), &text, *ignore_case, *word) else {
                        continue;
                    };
                    let mut body = if *erase_it {
                        erase(text.chars().count() + usize::from(end.is_some()))
                    } else {
                        Vec::new()
                    };
                    body.extend_from_slice(&strand.instructions[1..]);
                    if *erase_it && let Some(end) = end {
                        body.push(retype(end));
                    }
                    fires.push((
                        Target::with_body(mac, strand, body),
                        OnlyWhen::of(mac, only_when.as_ref()),
                    ));
                }
            }
            (emulator, s.global_speed_multiplier)
        };
        if fires.is_empty() {
            continue;
        }
        // The expansion's own output isn't typing to match again.
        typed.clear();
        // Conditions may ask the window system, so checked unlocked.
        for (target, condition) in fires {
            if holds(&condition) {
                target.fire(&emulator, speed_multiplier, &shared_state, &app);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn word_options_decide_where_an_abbreviation_may_sit() {
        assert_eq!(typed_match("say brb", "brb", false, None), Some(None));
        assert_eq!(typed_match("xbrb", "brb", false, None), Some(None));
        assert_eq!(
            typed_match("xbrb", "brb", false, Some(TypedWord::Start)),
            None
        );
        assert_eq!(
            typed_match("ok brb", "brb", false, Some(TypedWord::Start)),
            Some(None)
        );
        assert_eq!(
            typed_match("brb", "brb", false, Some(TypedWord::Start)),
            Some(None)
        );
        assert_eq!(
            typed_match("ok brb", "brb", false, Some(TypedWord::Whole)),
            None
        );
        assert_eq!(
            typed_match("ok brb.", "brb", false, Some(TypedWord::Whole)),
            Some(Some('.'))
        );
        assert_eq!(
            typed_match("ok brb\n", "brb", false, Some(TypedWord::Whole)),
            Some(Some('\n'))
        );
        assert_eq!(
            typed_match("ok xbrb ", "brb", false, Some(TypedWord::Whole)),
            None
        );
    }

    #[test]
    fn ignoring_case_matches_any_letters() {
        assert_eq!(typed_match("BRB", "brb", false, None), None);
        assert_eq!(typed_match("BRB", "brb", true, None), Some(None));
        assert_eq!(
            typed_match("Brb ", "bRb", true, Some(TypedWord::Whole)),
            Some(Some(' '))
        );
    }

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
