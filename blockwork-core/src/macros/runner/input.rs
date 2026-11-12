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

/// Waits between typed characters, scaled by playback speed. Returns true if
/// the run was stopped.
fn wait_for_typing(ctx: &mut ExecCtx, ms: f64) -> bool {
    let actual = (ms / ctx.speed_multiplier).max(0.0);
    if host_clock::enabled() {
        ctx.host_time_ms += actual;
        let flag = ctx.stop_flag.clone();
        let should_stop = move || flag.as_ref().is_some_and(stop_requested);
        return !host_clock::wait_until(ctx.host_time_ms, &should_stop);
    }
    sleep_unless_stopped(ctx, ms)
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
        InputToken::Text(token) => {
            let text = match ctx.resolve(&token.text, depth).and_then(|v| v.eval_text()) {
                Ok(t) => t,
                Err(e) => {
                    soft_fail(ctx, &ins.id, "Type text", e)?;
                    return Ok(None);
                }
            };
            let speed = match &token.speed {
                Some(speed) => {
                    let ms = match ctx
                        .resolve(&speed.value, depth)
                        .and_then(|v| v.eval_number())
                    {
                        Ok(ms) => ms.max(0.0),
                        Err(e) => {
                            soft_fail(ctx, &ins.id, "Type text", e)?;
                            return Ok(None);
                        }
                    };
                    Some(speed.per_character_ms(ms, text.chars().count()))
                }
                None => None,
            };
            if let Some(delay_ms) = speed {
                for c in text.chars() {
                    let mut em = match ctx.emulator.lock() {
                        Ok(guard) => guard,
                        Err(err) => {
                            warn!("Failed to lock emulator mutex: {}", err);
                            return Ok(Some(Flow::Normal));
                        }
                    };
                    let mut buf = [0; 4];
                    let letter = c.encode_utf8(&mut buf);
                    if let Err(err) = em.text(letter) {
                        warn!("Failed to type text '{}': {}", letter, err);
                    }
                    drop(em);
                    if delay_ms > 0.0 && wait_for_typing(ctx, delay_ms) {
                        return Ok(Some(Flow::Normal));
                    }
                }
                return Ok(None);
            }
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

/// Native contact sequences are preflighted in full before the first event.
pub(super) fn run_extended(ins: &Instruction, ctx: &mut ExecCtx, depth: u32) -> Result<(), String> {
    if matches!(
        ins.kind,
        InstructionKind::GamepadButton { .. }
            | InstructionKind::GamepadStick { .. }
            | InstructionKind::GamepadTrigger { .. }
            | InstructionKind::ResetGamepad
    ) {
        return run_gamepad(ins, ctx, depth);
    }
    if host_clock::enabled() {
        return Err("Native touch is not supported by the synchronous host clock".into());
    }
    use crate::input::touch::{Gesture, TouchGestureKind as G, TouchSequence};
    let number = |ctx: &mut ExecCtx, v: &Value| ctx.resolve(v, depth)?.eval_number();
    let (sequence, normalized, monitor) = match &ins.kind {
        InstructionKind::TouchGesture {
            gesture,
            fingers,
            x,
            y,
            end_x,
            end_y,
            duration,
            span,
            amount,
            normalized,
            monitor,
        } => {
            let x = number(ctx, x)?;
            let y = number(ctx, y)?;
            let factor = if *normalized { 10000.0 } else { 1.0 };
            let sequence = Gesture {
                kind: *gesture,
                fingers: *fingers,
                x: x * factor,
                y: y * factor,
                end_x: if *gesture == G::Swipe {
                    number(ctx, end_x)? * factor
                } else {
                    x * factor
                },
                end_y: if *gesture == G::Swipe {
                    number(ctx, end_y)? * factor
                } else {
                    y * factor
                },
                duration_ms: number(ctx, duration)?,
                span: if *fingers > 1 || matches!(gesture, G::Pinch | G::Rotate) {
                    number(ctx, span)? * factor
                } else {
                    0.0
                },
                amount: if matches!(gesture, G::Pinch | G::Rotate) {
                    number(ctx, amount)?
                } else {
                    1.0
                },
            }
            .sequence()?;
            (sequence, *normalized, *monitor)
        }
        InstructionKind::TouchSequence {
            sequence,
            normalized,
            monitor,
        } => (
            TouchSequence::parse(&ctx.resolve(sequence, depth)?.eval_text()?)?,
            *normalized,
            *monitor,
        ),
        _ => return Ok(()),
    };
    let monitors = crate::screen::monitors();
    let bounds = *monitors
        .get(monitor as usize)
        .ok_or("Touch target monitor isn't available")?;
    let sequence = crate::input::touch_playback::map_to_screen(&sequence, normalized, bounds)?;
    let flag = ctx.stop_flag.clone();
    let macro_id = ctx.source.macro_id.clone();
    let mut geometry_checked = Instant::now();
    crate::input::touch_playback::play(
        &sequence,
        Arc::clone(&ctx.emulator),
        ctx.speed_multiplier,
        || {
            if flag.as_ref().is_some_and(stop_requested)
                || crate::macros::debug::is_paused(&macro_id)
            {
                return true;
            }
            if geometry_checked.elapsed() > Duration::from_millis(250) {
                geometry_checked = Instant::now();
                return crate::screen::monitors()
                    .get(monitor as usize)
                    .is_none_or(|now| {
                        now.x != bounds.x
                            || now.y != bounds.y
                            || now.width != bounds.width
                            || now.height != bounds.height
                    });
            }
            false
        },
    )
}

fn run_gamepad(ins: &Instruction, ctx: &mut ExecCtx, depth: u32) -> Result<(), String> {
    use crate::input::gamepad::GamepadStick;
    if host_clock::enabled() {
        return Err(
            "Virtual controller output is unsupported by the synchronous host clock".into(),
        );
    }
    let number = |ctx: &mut ExecCtx, value: &Value, min: f64, max: f64| -> Result<f64, String> {
        let value = ctx.resolve(value, depth)?.eval_number()?;
        if !value.is_finite() || value < min || value > max {
            return Err(format!("Controller value must be {min} to {max}"));
        }
        Ok(value)
    };
    enum Update {
        Button(crate::input::gamepad::GamepadButton, Direction),
        Stick(GamepadStick, i16, i16),
        Trigger(bool, u16),
        Reset,
    }
    let update = match &ins.kind {
        InstructionKind::GamepadButton { button, direction } => {
            Update::Button(*button, direction.clone())
        }
        InstructionKind::GamepadStick { stick, x, y } => Update::Stick(
            *stick,
            (number(ctx, x, -1.0, 1.0)? * 32767.0).round() as i16,
            (number(ctx, y, -1.0, 1.0)? * 32767.0).round() as i16,
        ),
        InstructionKind::GamepadTrigger { left, amount } => Update::Trigger(
            *left,
            (number(ctx, amount, 0.0, 1.0)? * 1023.0).round() as u16,
        ),
        InstructionKind::ResetGamepad => Update::Reset,
        _ => return Ok(()),
    };
    let mut state = ctx
        .gamepad
        .as_ref()
        .filter(|lease| lease.is_active())
        .map(|lease| lease.state().clone())
        .unwrap_or_default();
    match update {
        Update::Button(button, Direction::Release) => {
            state.buttons.remove(&button);
        }
        Update::Button(button, _) => {
            state.buttons.insert(button);
        }
        Update::Stick(GamepadStick::Left, x, y) => {
            state.left_x = x;
            state.left_y = y;
        }
        Update::Stick(GamepadStick::Right, x, y) => {
            state.right_x = x;
            state.right_y = y;
        }
        Update::Trigger(true, value) => state.left_trigger = value,
        Update::Trigger(false, value) => state.right_trigger = value,
        Update::Reset => state = Default::default(),
    }
    if ctx.gamepad.as_ref().is_some_and(|lease| !lease.is_active()) {
        ctx.gamepad = None;
    }
    if ctx.gamepad.is_none() {
        let flag = ctx.stop_flag.clone();
        let macro_id = ctx.source.macro_id.clone();
        ctx.gamepad = Some(crate::input::virtual_gamepad::Lease::acquire(
            Arc::clone(&ctx.emulator),
            Arc::new(move || {
                flag.as_ref().is_some_and(stop_requested)
                    || crate::macros::debug::is_paused(&macro_id)
                    || !crate::config::load_settings()
                        .virtual_gamepad_enabled
                        .unwrap_or(false)
            }),
        )?);
    }
    let lease = ctx.gamepad.as_mut().unwrap();
    lease.set(state.clone())?;
    if let InstructionKind::GamepadButton {
        button,
        direction: Direction::Click,
    } = &ins.kind
    {
        let until = Instant::now() + Duration::from_millis(50);
        while Instant::now() < until {
            if !lease.is_active() {
                return Err("Controller click was cancelled".into());
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        state.buttons.remove(button);
        lease.set(state)?;
    }
    Ok(())
}
