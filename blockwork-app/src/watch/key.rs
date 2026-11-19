//! Background service that fires `WhenKey` strands, and the blocks under a
//! `RemapKey`. Listens to the capture callback's key events and fires a
//! strand's body (skipping its header) when its combo is pressed, released,
//! pressed twice or held. Blocking and remapping themselves happen in the
//! capture callback (see `key_rules`).
//!
//! Watches the live macros (`scheduled_run::live_macros`) like every watcher.

use super::{OnlyWhen, StrandKey, Target, holds, variable_env};
use crate::AppHandle;
use crate::scheduled_run;
use crate::state::SharedState;
use blockwork_core::input::key_names::{ComboKey, combo_held, parse_trigger_combo};
use blockwork_core::input::types::{Direction, MacroKey};
use blockwork_core::macros::{InstructionKind, KeyGestureKind};
use blockwork_core::recording;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::mpsc::RecvTimeoutError;
use std::time::{Duration, Instant};

pub(crate) fn start(shared_state: SharedState, app: AppHandle) {
    let rx = recording::subscribe_key_events();
    let _ = std::thread::Builder::new()
        .name("key-watch".into())
        .spawn(move || run(shared_state, app, rx));
}

/// What a strand's header waits for, read from a `WhenKey` or `RemapKey`.
struct Watch {
    combo: Vec<ComboKey>,
    extra_keys: bool,
    on_press: bool,
    gesture: Option<(KeyGestureKind, Duration)>,
}

/// The key header heading `kind`, with its combo resolved, if it is one.
fn watch_of(kind: &InstructionKind, mac: &blockwork_core::macros::Macro) -> Option<Watch> {
    let env = variable_env(mac);
    let text = |v: &blockwork_core::input::value::Value| v.resolve_vars(&env).eval_text().ok();
    match kind {
        InstructionKind::WhenKey {
            key,
            direction,
            extra_keys,
            gesture,
            ..
        } => {
            let gesture = match gesture {
                Some(g) => {
                    let ms = g.ms.resolve_vars(&env).eval_number().ok()?;
                    Some((g.kind, Duration::from_secs_f64(ms.max(0.0) / 1000.0)))
                }
                None => None,
            };
            Some(Watch {
                combo: parse_trigger_combo(&text(key)?),
                extra_keys: *extra_keys,
                on_press: gesture.is_some() || !matches!(direction, Direction::Release),
                gesture,
            })
        }
        // A remap's own blocks run on each press of its key.
        InstructionKind::RemapKey { from, .. } => Some(Watch {
            combo: parse_trigger_combo(&text(from)?),
            extra_keys: true,
            on_press: true,
            gesture: None,
        }),
        _ => None,
    }
}

fn only_when(kind: &InstructionKind) -> Option<&blockwork_core::input::value::Value> {
    match kind {
        InstructionKind::WhenKey { only_when, .. }
        | InstructionKind::RemapKey { only_when, .. } => only_when.as_ref(),
        _ => None,
    }
}

/// A long press waiting for its time to pass with the combo still held.
struct Hold {
    key: StrandKey,
    combo: Vec<ComboKey>,
    due: Instant,
    target: Target,
}

fn run(
    shared_state: SharedState,
    app: AppHandle,
    rx: std::sync::mpsc::Receiver<recording::KeyEvent>,
) {
    let mut held: Vec<MacroKey> = Vec::new();
    let mut last_press: HashMap<StrandKey, Instant> = HashMap::new();
    let mut pending: Vec<Hold> = Vec::new();

    loop {
        let next_due = pending.iter().map(|h| h.due).min();
        let received = match next_due {
            Some(due) => rx.recv_timeout(due.saturating_duration_since(Instant::now())),
            None => rx.recv().map_err(|_| RecvTimeoutError::Disconnected),
        };
        let event = match received {
            Ok(event) => Some(event),
            Err(RecvTimeoutError::Timeout) => None,
            Err(RecvTimeoutError::Disconnected) => return,
        };

        let mut fires = Vec::new();
        // Long presses whose time is up, still held.
        let now = Instant::now();
        let (due, waiting): (Vec<Hold>, Vec<Hold>) = pending.drain(..).partition(|h| h.due <= now);
        pending = waiting;
        fires.extend(due.into_iter().map(|h| h.target));

        let Some(event) = event else {
            fire_all(fires, &shared_state, &app);
            continue;
        };
        let key = event.key.clone();
        let was_held = held.contains(&key);
        // Combo state as it was just before a release, and just after a press.
        let mut before = held.clone();
        if event.pressed {
            if !was_held {
                held.push(key.clone());
            }
        } else {
            held.retain(|k| k != &key);
            // Letting go of a key cancels the long presses that needed it.
            pending.retain(|h| !h.combo.iter().any(|c| c.matches(&key)));
        }
        if event.pressed && was_held {
            fire_all(fires, &shared_state, &app);
            continue;
        }
        if !event.pressed && !before.contains(&key) {
            before.push(key.clone());
        }
        let state_for_match = if event.pressed { &held } else { &before };

        // Copied out under the lock; conditions are checked after it.
        let mut candidates = Vec::new();
        {
            let Ok(s) = shared_state.lock() else { continue };
            for mac in scheduled_run::live_macros(&s) {
                for strand in &mac.strands {
                    let Some(header) = strand.instructions.first().map(|i| &i.kind) else {
                        continue;
                    };
                    let Some(watch) = watch_of(header, mac) else {
                        continue;
                    };
                    if watch.on_press != event.pressed
                        || !watch.combo.iter().any(|c| c.matches(&key))
                        || !combo_held(&watch.combo, state_for_match, watch.extra_keys)
                    {
                        continue;
                    }
                    // A remap with nothing under it has nothing to run.
                    if matches!(header, InstructionKind::RemapKey { .. })
                        && strand.instructions.len() < 2
                    {
                        continue;
                    }
                    candidates.push((
                        (mac.id.clone(), strand.id.clone()),
                        watch,
                        OnlyWhen::of(mac, only_when(header)),
                        Target::new(mac, strand),
                    ));
                }
            }
        }

        let mut trigger_keys: Vec<MacroKey> = Vec::new();
        for (strand_key, watch, condition, target) in candidates {
            if !holds(&condition) {
                continue;
            }
            match watch.gesture {
                Some((KeyGestureKind::Double, within)) => {
                    let now = Instant::now();
                    match last_press.remove(&strand_key) {
                        Some(first) if now.duration_since(first) <= within => {}
                        _ => {
                            last_press.insert(strand_key, now);
                            continue;
                        }
                    }
                }
                Some((KeyGestureKind::Hold, after)) => {
                    pending.retain(|h| h.key != strand_key);
                    pending.push(Hold {
                        key: strand_key,
                        combo: watch.combo,
                        due: Instant::now() + after,
                        target,
                    });
                    continue;
                }
                None => {}
            }
            trigger_keys.extend(
                state_for_match
                    .iter()
                    .filter(|h| watch.combo.iter().any(|c| c.matches(h)))
                    .cloned(),
            );
            fires.push(target);
        }

        if !trigger_keys.is_empty() {
            blockwork_core::macros::backend::release_held_keys(&trigger_keys);
        }
        fire_all(fires, &shared_state, &app);
    }
}

fn fire_all(fires: Vec<Target>, shared_state: &SharedState, app: &AppHandle) {
    if fires.is_empty() {
        return;
    }
    let (emulator, speed_multiplier) = {
        let Ok(s) = shared_state.lock() else { return };
        let Some(em) = s.emulator.as_ref().map(Arc::clone) else {
            return;
        };
        (em, s.global_speed_multiplier)
    };
    for target in fires {
        target.fire(&emulator, speed_multiplier, shared_state, app);
    }
}
