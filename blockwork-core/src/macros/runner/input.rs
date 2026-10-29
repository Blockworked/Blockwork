//! The input blocks: typing, keys, mouse buttons, moves and scrolls.

use super::*;

/// In a combo, plain Shift means the left one.
fn normalize_modifier_key(key: MacroKey) -> MacroKey {
    match key {
        MacroKey::Shift => MacroKey::LShift,
        _ => key,
    }
}

fn shift_is_pressed(keys: &[MacroKey]) -> bool {
    keys.iter()
        .any(|k| matches!(k, MacroKey::Shift | MacroKey::LShift | MacroKey::RShift))
}

/// Runs one input token. `Some(flow)` ends the strand with it; `None` goes
/// on to the next block.
pub(super) fn run_token(
    token: &InputToken,
    ins: &Instruction,
    ctx: &mut ExecCtx,
    depth: u32,
) -> Result<Option<Flow>, String> {
    match token {
        InputToken::Text(value) => {
            let text = match ctx.resolve(value, depth).and_then(|v| v.eval_text()) {
                Ok(t) => t,
                Err(e) => {
                    soft_fail(ctx, &ins.id, "Type text", e)?;
                    return Ok(None);
                }
            };
            let mut em = match ctx.emulator.lock() {
                Ok(guard) => guard,
                Err(err) => {
                    warn!("Failed to lock emulator mutex: {}", err);
                    return Ok(Some(Flow::Normal));
                }
            };
            if let Err(err) = em.text(&text) {
                warn!("Failed to type text '{}': {}", text, err);
            }
        }
        InputToken::Key(key, direction) => {
            let text = match ctx.resolve(key, depth).and_then(|v| v.eval_text()) {
                Ok(t) => t,
                Err(e) => {
                    soft_fail(ctx, &ins.id, "Key", e)?;
                    return Ok(None);
                }
            };
            let mut keys: Vec<MacroKey> = crate::input::key_names::parse_key_combo(&text)
                .into_iter()
                .map(normalize_modifier_key)
                .collect();
            // In a combo, `Control+A` means the A key; Shift is
            // added only when the combo names it.
            if keys.len() > 1 {
                for key in &mut keys {
                    if let MacroKey::Unicode(c) = key {
                        *c = c.to_ascii_lowercase();
                    }
                }
            }
            if keys.is_empty() {
                soft_fail(ctx, &ins.id, "Key", format!("no key in '{text}'"))?;
                return Ok(None);
            }
            let mut em = match ctx.emulator.lock() {
                Ok(guard) => guard,
                Err(err) => {
                    warn!("Failed to lock emulator mutex: {}", err);
                    return Ok(Some(Flow::Normal));
                }
            };
            // Press in order, release in reverse, like a typed chord.
            let press = !matches!(direction, Direction::Release);
            let release = !matches!(direction, Direction::Press);
            let mut errors = Vec::new();
            let mut emit = |key: &MacroKey, dir: Direction, pressed_keys: &mut Vec<MacroKey>| {
                let key_for_event = match (key, &dir) {
                    (MacroKey::Unicode(c), Direction::Click)
                        if shift_is_pressed(pressed_keys) && c.is_ascii_lowercase() =>
                    {
                        MacroKey::Unicode(c.to_ascii_uppercase())
                    }
                    _ => key.clone(),
                };
                match em.key(key_for_event, dir.clone()) {
                    Ok(()) => match dir {
                        Direction::Press => {
                            if !pressed_keys.contains(key) {
                                pressed_keys.push(key.clone());
                            }
                        }
                        Direction::Release => pressed_keys.retain(|k| k != key),
                        Direction::Click => {}
                    },
                    Err(err) => errors.push(format!("{key:?} ({dir:?}): {err}")),
                }
            };
            if keys.len() == 1 {
                emit(&keys[0], direction.clone(), ctx.pressed_keys);
            } else {
                if press {
                    for key in &keys {
                        emit(key, Direction::Press, ctx.pressed_keys);
                    }
                }
                if release {
                    for key in keys.iter().rev() {
                        emit(key, Direction::Release, ctx.pressed_keys);
                    }
                }
            }
            // Finish the release sequence before reporting a failed
            // key; Try/Catch may leave this block immediately.
            drop(em);
            if !errors.is_empty() {
                soft_fail(ctx, &ins.id, "Key", errors.join("; "))?;
            }
        }
        InputToken::Raw(keycode, direction) => {
            let mut em = match ctx.emulator.lock() {
                Ok(guard) => guard,
                Err(err) => {
                    warn!("Failed to lock emulator mutex: {}", err);
                    return Ok(Some(Flow::Normal));
                }
            };
            if let Err(err) = em.raw_keycode(*keycode, direction.clone()) {
                warn!("Failed to emit raw keycode {}: {}", keycode, err);
            }
        }
        InputToken::Button(button, direction) => {
            let mut em = match ctx.emulator.lock() {
                Ok(guard) => guard,
                Err(err) => {
                    warn!("Failed to lock emulator mutex: {}", err);
                    return Ok(Some(Flow::Normal));
                }
            };
            match em.button(button.clone(), direction.clone()) {
                Ok(()) => match direction {
                    Direction::Press => {
                        if !ctx.pressed_buttons.contains(button) {
                            ctx.pressed_buttons.push(button.clone());
                        }
                    }
                    Direction::Release => ctx.pressed_buttons.retain(|b| b != button),
                    // Press+release in one call - nothing left held.
                    Direction::Click => {}
                },
                Err(err) => {
                    warn!(
                        "Failed to click button {:?} ({:?}): {}",
                        button, direction, err
                    );
                }
            }
        }
        InputToken::MoveMouse(x, y, coordinate) => {
            let x = match ctx.resolve(x, depth).and_then(|v| v.eval_number()) {
                Ok(v) => v.round() as i32,
                Err(e) => {
                    soft_fail(ctx, &ins.id, "Move mouse", e)?;
                    return Ok(None);
                }
            };
            let y = match ctx.resolve(y, depth).and_then(|v| v.eval_number()) {
                Ok(v) => v.round() as i32,
                Err(e) => {
                    soft_fail(ctx, &ins.id, "Move mouse", e)?;
                    return Ok(None);
                }
            };
            let mut em = match ctx.emulator.lock() {
                Ok(guard) => guard,
                Err(err) => {
                    warn!("Failed to lock emulator mutex: {}", err);
                    return Ok(Some(Flow::Normal));
                }
            };
            let result = match coordinate {
                Coordinate::Rel => em.move_mouse_rel(x, y),
                Coordinate::Abs => em.move_mouse_abs(x, y),
            };
            if let Err(err) = result {
                warn!("Failed to move mouse ({}, {}): {}", x, y, err);
            }
        }
        InputToken::Scroll(amount, axis) => {
            let amount = match ctx.resolve(amount, depth).and_then(|v| v.eval_number()) {
                Ok(v) => v.round() as i32,
                Err(e) => {
                    soft_fail(ctx, &ins.id, "Scroll", e)?;
                    return Ok(None);
                }
            };
            let mut em = match ctx.emulator.lock() {
                Ok(guard) => guard,
                Err(err) => {
                    warn!("Failed to lock emulator mutex: {}", err);
                    return Ok(Some(Flow::Normal));
                }
            };
            if let Err(err) = em.scroll(amount, axis.clone()) {
                warn!("Failed to scroll by {}: {}", amount, err);
            }
        }
    }
    Ok(None)
}

/// Runs Android gestures and accessibility actions through the host backend.
pub(super) fn run_android_input(
    ins: &Instruction,
    ctx: &mut ExecCtx,
    depth: u32,
) -> Result<(), String> {
    match &ins.kind {
        InstructionKind::Element {
            action,
            selector,
            text,
            timeout,
            variable,
        } => {
            let result: Result<serde_json::Value, String> = (|| {
                use crate::input::element::ElementAction;
                selector.validate()?;
                let text = if *action == ElementAction::SetText {
                    ctx.resolve(text, depth)?.eval_text()?
                } else {
                    String::new()
                };
                let timeout = if *action == ElementAction::Wait {
                    ctx.resolve(timeout, depth)?.eval_number()?
                } else {
                    0.0
                };
                if !timeout.is_finite() || timeout < 0.0 || timeout > 86_400_000.0 {
                    return Err(
                        "element wait timeout must be 0–86400000 ms (0 waits until stopped)".into(),
                    );
                }
                let started = Instant::now();
                let stopped = || ctx.stop_flag.as_ref().is_some_and(stop_requested);
                loop {
                    if stopped() {
                        return Err("element action cancelled".into());
                    }
                    let query = if *action == ElementAction::Wait {
                        ElementAction::Exists
                    } else {
                        *action
                    };
                    let expired = || {
                        *action == ElementAction::Wait
                            && timeout > 0.0
                            && started.elapsed().as_secs_f64() * 1000.0 >= timeout
                    };
                    let response = ctx.emulator.lock().map_err(|e| e.to_string())?.element(
                        selector,
                        query,
                        &text,
                        &|| stopped() || expired(),
                    );
                    let value = match response {
                        Err(_) if !stopped() && expired() => {
                            return Err("element did not appear before the timeout".into());
                        }
                        other => other?,
                    };
                    if *action != ElementAction::Wait || value == serde_json::Value::Bool(true) {
                        return Ok(value);
                    }
                    if timeout > 0.0 && started.elapsed().as_secs_f64() * 1000.0 >= timeout {
                        return Err("element did not appear before the timeout".into());
                    }
                    for _ in 0..4 {
                        if stopped() {
                            return Err("element wait cancelled".into());
                        }
                        std::thread::sleep(Duration::from_millis(25));
                    }
                }
            })();
            match result {
                Ok(value) if !variable.is_empty() => set_variable(
                    ctx,
                    variable,
                    match value {
                        serde_json::Value::Bool(value) => Evaluated::Bool(value),
                        serde_json::Value::String(value) => Evaluated::Text(value),
                        _ => Evaluated::Text(String::new()),
                    },
                ),
                Ok(_) => {}
                Err(error) => soft_fail(ctx, &ins.id, "Element", error)?,
            }
        }
        InstructionKind::Touch {
            action,
            x,
            y,
            end_x,
            end_y,
            duration,
        } => {
            let result = (|| {
                let mut number = |value: &Value| ctx.resolve(value, depth)?.eval_number();
                let (x, y, end_x, end_y, duration) = (
                    number(x)?,
                    number(y)?,
                    number(end_x)?,
                    number(end_y)?,
                    number(duration)?,
                );
                if ![x, y, end_x, end_y, duration].iter().all(|n| n.is_finite())
                    || duration < 1.0
                    || duration > 60_000.0
                {
                    return Err(
                        "gesture coordinates must be finite and duration must be 1–60000 ms".into(),
                    );
                }
                let gesture = crate::input::touch::Gesture {
                    action: *action,
                    x,
                    y,
                    end_x,
                    end_y,
                    duration_ms: duration.round() as u64,
                };
                let stopped = || ctx.stop_flag.as_ref().is_some_and(stop_requested);
                ctx.emulator
                    .lock()
                    .map_err(|e| e.to_string())?
                    .gesture(&gesture, &stopped)
            })();
            if let Err(error) = result {
                soft_fail(ctx, &ins.id, "Touch", error)?;
            }
        }
        _ => unreachable!("not an Android input block"),
    }
    Ok(())
}
