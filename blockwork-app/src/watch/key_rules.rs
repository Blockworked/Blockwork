//! Keeps the capture callback's rule table (`recording::key_rules`) in step
//! with the live macros: `when key` and `when mouse button` blocks that block
//! their input, and `remap key` blocks. [`refresh`] rebuilds the table when
//! the app state changes. One thread re-checks each rule's conditions (the
//! macro's own and the block's "only when") about ten times a second, since
//! the callback can't wait for them; another presses the keys remaps ask for.

use super::{OnlyWhen, holds, permitted, variable_env};
use crate::scheduled_run;
use crate::state::{AppState, SharedState};
use blockwork_core::input::key_names::{keys_to_press, parse_trigger_combo};
use blockwork_core::input::types::{Direction, MacroKey};
use blockwork_core::macros::{InstructionKind, Macro, MacroSettings};
use blockwork_core::recording::key_rules::{
    RemapEvent, Rule, RuleAction, RuleInput, set_rules, subscribe_remaps,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::RecvTimeoutError;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// How often conditions are re-checked.
const ARM_INTERVAL: Duration = Duration::from_millis(100);

/// What arms one rule: the macro's settings and the block's own condition.
struct Arming {
    armed: Arc<AtomicBool>,
    settings: MacroSettings,
    only_when: Option<OnlyWhen>,
}

/// The table as last built, by what built it, so an unrelated state change
/// doesn't rebuild it (and briefly disarm every rule).
static BUILT: Mutex<Option<String>> = Mutex::new(None);
static ARMING: Mutex<Vec<Arc<Arming>>> = Mutex::new(Vec::new());

/// The rule `kind` asks for, if it blocks or remaps anything.
fn rule_of(kind: &InstructionKind, mac: &Macro, can_remap: bool) -> Option<Rule> {
    let env = variable_env(mac);
    let text = |v: &blockwork_core::input::value::Value| v.resolve_vars(&env).eval_text().ok();
    let (input, action) = match kind {
        InstructionKind::WhenKey {
            key,
            block: true,
            extra_keys,
            ..
        } => (
            RuleInput::Keys {
                combo: parse_trigger_combo(&text(key)?),
                extra_keys: *extra_keys,
            },
            RuleAction::Block,
        ),
        InstructionKind::WhenMouseButton {
            button,
            modifiers,
            block: true,
            ..
        } => {
            let combo = modifiers.as_ref().and_then(&text).unwrap_or_default();
            let modifiers = if combo.trim().is_empty() {
                None
            } else {
                Some(super::mouse::modifier_bits(&combo)?)
            };
            (
                RuleInput::Button {
                    button: button.clone(),
                    modifiers,
                },
                RuleAction::Block,
            )
        }
        // Without a way to press the new key, remapping would only lose it.
        InstructionKind::RemapKey { from, to, .. } if can_remap => {
            let to = keys_to_press(&text(to)?);
            if to.is_empty() {
                return None;
            }
            (
                RuleInput::Keys {
                    combo: parse_trigger_combo(&text(from)?),
                    extra_keys: true,
                },
                RuleAction::Remap(to),
            )
        }
        _ => return None,
    };
    if let RuleInput::Keys { combo, .. } = &input
        && combo.is_empty()
    {
        return None;
    }
    Some(Rule::new(input, action))
}

fn only_when(kind: &InstructionKind) -> Option<&blockwork_core::input::value::Value> {
    match kind {
        InstructionKind::WhenKey { only_when, .. }
        | InstructionKind::WhenMouseButton { only_when, .. }
        | InstructionKind::RemapKey { only_when, .. } => only_when.as_ref(),
        _ => None,
    }
}

/// Rebuilds the rule table from the live macros, if they changed what it
/// holds. Paused triggers block and remap nothing.
pub(crate) fn refresh(s: &AppState) {
    let mut headers = Vec::new();
    if !s.triggers_paused {
        for mac in scheduled_run::live_macros(s) {
            for strand in &mac.strands {
                if let Some(ins) = strand.instructions.first()
                    && matches!(
                        ins.kind,
                        InstructionKind::WhenKey { block: true, .. }
                            | InstructionKind::WhenMouseButton { block: true, .. }
                            | InstructionKind::RemapKey { .. }
                    )
                {
                    headers.push((mac, &ins.kind));
                }
            }
        }
    }
    let can_remap = s.emulator.is_some();
    // What the table depends on: the headers, their macros' variables (a
    // combo can be one) and conditions, and whether remapping can work.
    let signature = serde_json::to_string(
        &headers
            .iter()
            .map(|(mac, kind)| (&mac.id, kind, &mac.variables, &mac.settings, &mac.name))
            .collect::<Vec<_>>(),
    )
    .unwrap_or_default()
        + if can_remap { "+remap" } else { "" };
    let Ok(mut built) = BUILT.lock() else { return };
    if built.as_deref() == Some(signature.as_str()) {
        return;
    }
    *built = Some(signature);

    let mut rules = Vec::new();
    let mut arming = Vec::new();
    for (mac, kind) in headers {
        if !permitted(mac, "input") {
            continue;
        }
        let Some(rule) = rule_of(kind, mac, can_remap) else {
            continue;
        };
        let only_when = OnlyWhen::of(mac, only_when(kind));
        if only_when.is_some() || mac.settings.has_conditions() {
            // Off until the arming thread has checked it once.
            rule.armed.store(false, Ordering::Relaxed);
            arming.push(Arc::new(Arming {
                armed: Arc::clone(&rule.armed),
                settings: mac.settings.clone(),
                only_when,
            }));
        }
        rules.push(rule);
    }
    if let Ok(mut slot) = ARMING.lock() {
        *slot = arming;
    }
    set_rules(rules);
}

/// Starts the arming and remap threads.
pub(crate) fn start(shared_state: SharedState) {
    let _ = std::thread::Builder::new()
        .name("key-rules-arm".into())
        .spawn(arm_forever);
    let rx = subscribe_remaps();
    let _ = std::thread::Builder::new()
        .name("key-remap".into())
        .spawn(move || press_remaps(shared_state, rx));
}

fn arm_forever() {
    loop {
        // Checked unlocked: a condition may ask the window system.
        let checks = ARMING.lock().map(|a| a.clone()).unwrap_or_default();
        for a in checks {
            let on = blockwork_core::macros::conditions::blocked_by(&a.settings).is_none()
                && holds(&a.only_when);
            a.armed.store(on, Ordering::Relaxed);
        }
        std::thread::sleep(ARM_INTERVAL);
    }
}

fn press_remaps(shared_state: SharedState, rx: std::sync::mpsc::Receiver<RemapEvent>) {
    // The keys held down for a remap, and when the last one repeats next.
    let mut down: Option<(Vec<MacroKey>, Instant)> = None;
    // Held remap keys repeat like a held key would, at the system's own
    // rate, where the system doesn't repeat injected keys itself (Linux does).
    let repeats = !cfg!(target_os = "linux");
    let (mut repeat_delay, mut repeat_every) = (Duration::ZERO, Duration::ZERO);
    loop {
        let event = match down.as_ref().filter(|_| repeats) {
            Some((_, due)) => rx.recv_timeout(due.saturating_duration_since(Instant::now())),
            None => rx.recv().map_err(|_| RecvTimeoutError::Disconnected),
        };
        let emulator = shared_state
            .lock()
            .ok()
            .and_then(|s| s.emulator.as_ref().map(Arc::clone));
        let send = |key: &MacroKey, dir: Direction| {
            if let Some(Ok(mut em)) = emulator.as_ref().map(|e| e.lock())
                && let Err(e) = em.key(key.clone(), dir)
            {
                tracing::warn!("remap: couldn't send {key:?}: {e}");
            }
        };
        match event {
            Ok(RemapEvent::Press(keys)) => {
                keys.iter().for_each(|k| send(k, Direction::Press));
                // Read per press, so a changed setting applies right away.
                if repeats {
                    (repeat_delay, repeat_every) = blockwork_core::input::key_repeat::settings();
                }
                down = Some((keys, Instant::now() + repeat_delay));
            }
            Ok(RemapEvent::Release(keys)) => {
                keys.iter().rev().for_each(|k| send(k, Direction::Release));
                if down.as_ref().is_some_and(|(held, _)| *held == keys) {
                    down = None;
                }
            }
            Err(RecvTimeoutError::Timeout) => {
                if let Some((keys, due)) = down.as_mut() {
                    if let Some(last) = keys.last() {
                        send(last, Direction::Press);
                    }
                    *due = Instant::now() + repeat_every;
                }
            }
            Err(RecvTimeoutError::Disconnected) => return,
        }
    }
}
