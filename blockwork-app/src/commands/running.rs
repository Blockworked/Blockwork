//! Key capture, running macros and recording.

use super::*;

pub(crate) fn start_key_capture(
    state: &SharedState,
    app: &AppHandle,
    strand_id: String,
    path: Vec<PathStep>,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.key_capture = Some(KeyCaptureTarget::Strand(strand_id, path));
    emit_state_updated(&app, &s);
    Ok(())
}

/// Same capture flow, but for a key field with no backing strand/instruction
/// (the sidebar's Key prefab) - the result lands in `pending_standalone_key`
/// instead of being written into a strand.
pub(crate) fn start_standalone_key_capture(
    state: &SharedState,
    app: &AppHandle,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.key_capture = Some(KeyCaptureTarget::Standalone);
    emit_state_updated(&app, &s);
    Ok(())
}

/// `keys` are the `(code, key)` pairs held when a key was released, in
/// press order; they become one combo such as `Control+Alt+o`.
pub(crate) fn key_capture_event(
    state: &SharedState,
    app: &AppHandle,
    keys: Vec<(String, String)>,
) -> Result<(), String> {
    use blockwork_core::input::key_names::{format_key_combo, match_key};
    let mut combo: Vec<blockwork_core::input::types::MacroKey> = Vec::new();
    for (code, key) in &keys {
        let parsed = blockwork_core::key_mapping::web_code_to_macro_key(code)
            .or_else(|| blockwork_core::key_mapping::web_key_to_macro_key(key));
        if let Some(mk) = parsed {
            if !combo.iter().any(|k| match_key(k) == match_key(&mk)) {
                combo.push(mk);
            }
        }
    }
    if combo.is_empty() {
        return Ok(());
    }
    let text = format_key_combo(&combo);

    let mut s = state.lock().map_err(|e| e.to_string())?;
    let Some(target) = s.key_capture.take() else {
        return Ok(());
    };
    match target {
        KeyCaptureTarget::Strand(strand_id, path) => {
            let captured = s.current_macro.as_mut().is_some_and(|mac| {
                match mac.instruction_at_mut(&strand_id, &path) {
                    Some(Instruction {
                        kind:
                            InstructionKind::Token(InputToken::Key(value, _))
                            | InstructionKind::WhenKey(value, _),
                        ..
                    }) => {
                        *value = Value::Text { value: text };
                        true
                    }
                    _ => false,
                }
            });
            if captured {
                auto_save(&mut s);
            }
        }
        KeyCaptureTarget::Standalone => s.pending_standalone_key = Some(text),
    }
    emit_state_updated(&app, &s);
    Ok(())
}

/// Consumes `pending_standalone_key` once the frontend has copied it, so a
/// stale value can't leak into the next capture.
pub(crate) fn clear_standalone_key_capture(
    state: &SharedState,
    app: &AppHandle,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.pending_standalone_key = None;
    emit_state_updated(&app, &s);
    Ok(())
}

// ─── Execution ─────────────────────────────────────────────────────────────

pub(crate) fn run_macro(
    state: &SharedState,
    app: &AppHandle,
) -> Result<(), String> {
    run_open_macro(state, app, None)
}

/// Runs one strand of the open macro from its top block, skipping its event.
pub(crate) fn run_strand(
    state: &SharedState,
    app: &AppHandle,
    strand_id: String,
) -> Result<(), String> {
    {
        let s = state.lock().map_err(|e| e.to_string())?;
        let mac = s.current_macro.as_ref().ok_or("No macro selected")?;
        if !mac.can_run_strand(&strand_id) {
            return Err("That strand can't be run on its own".into());
        }
    }
    run_open_macro(state, app, Some(strand_id))
}

pub(super) fn run_open_macro(
    state: &SharedState,
    app: &AppHandle,
    strand: Option<String>,
) -> Result<(), String> {
    let (mac, emulator, is_looping, speed_multiplier, variables, lists) = {
        let s = state.lock().map_err(|e| e.to_string())?;
        let mac = s.current_macro.clone();
        let emulator = s.emulator.as_ref().map(Arc::clone);
        let is_looping = Arc::clone(&s.is_looping);
        let speed_multiplier =
            mac.as_ref().map_or(1.0, |m| m.speed_multiplier) * s.global_speed_multiplier;
        let variables = Arc::clone(&s.variable_values);
        let lists = Arc::clone(&s.list_values);
        (
            mac,
            emulator,
            is_looping,
            speed_multiplier,
            variables,
            lists,
        )
    };

    if let (Some(mac), Some(emulator)) = (mac, emulator) {
        let _ = loop_control::set_loop_state(&is_looping, true);
        let mac_name = mac.name.clone();
        let single_run_task = macros_thread::into_single_run_task(
            mac,
            Arc::clone(&emulator),
            Arc::clone(&is_looping),
            speed_multiplier,
            variables,
            lists,
            Arc::clone(&*state),
            app.clone(),
            strand,
        );
        let mut s = state.lock().map_err(|e| e.to_string())?;
        if let Err(e) = macros_thread::spawn_macro_thread(
            &mut s.thread_pool,
            format!("run_{}", mac_name),
            single_run_task,
        ) {
            warn!("Failed to spawn run thread: {e}");
            let _ = loop_control::stop_loop(&is_looping);
        }
    }

    let s = state.lock().map_err(|e| e.to_string())?;
    emit_state_updated(&app, &s);
    Ok(())
}

pub(crate) fn set_global_speed_multiplier(
    state: &SharedState,
    app: &AppHandle,
    multiplier: f64,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    let clamped = multiplier.clamp(
        *SPEED_MULTIPLIER_RANGE.start(),
        *SPEED_MULTIPLIER_RANGE.end(),
    );
    s.global_speed_multiplier = clamped;
    config::update_settings(|settings| settings.global_speed_multiplier = Some(clamped));
    emit_state_updated(&app, &s);
    Ok(())
}

// ─── Recording ─────────────────────────────────────────────────────────────

/// Seeds the recorder's tracked cursor position from the real, current
/// cursor position so the first captured move has a baseline - needed for
/// absolute-move recording, which adds each relative delta the backend
/// reports onto a known starting point. Best-effort: if the backend can't
/// report a position, absolute recording waits for a later seed.
pub(super) fn seed_recording_mouse_pos(s: &crate::state::AppState) {
    let Some(emulator) = s.emulator.as_ref() else {
        return;
    };
    let Ok(mut emulator) = emulator.lock() else {
        return;
    };
    // Absolute recording needs an exact origin, which on Wayland means placing
    // the cursor rather than asking where it is.
    let pos = if s.record_mouse_relative || !s.record_mouse_movement {
        emulator.cursor_pos()
    } else {
        emulator.anchor_cursor()
    };
    if let Some((x, y)) = pos {
        recording::set_last_mouse_pos(x as f64, y as f64);
    }
}

pub(crate) fn start_recording(
    state: &SharedState,
    app: &AppHandle,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    // A second start while counting down or recording must not restart the
    // countdown; the UI cancels with stop_recording instead.
    if s.current_macro.is_none() || s.recording_phase != RecordingPhase::Idle {
        return Ok(());
    }
    s.recording_countdown_generation = s.recording_countdown_generation.wrapping_add(1);
    let countdown_gen = s.recording_countdown_generation;
    let secs = crate::recording_countdown_secs();
    if secs == 0 {
        begin_recording(&mut s);
        emit_state_updated(&app, &s);
        return Ok(());
    }
    s.recording_phase = RecordingPhase::Countdown(secs);
    emit_state_updated(&app, &s);
    drop(s);

    let state_clone = Arc::clone(&*state);
    let app_clone = app.clone();
    crate::async_runtime::spawn(async move {
        for n in (0..secs).rev() {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            let mut s = match state_clone.lock() {
                Ok(g) => g,
                Err(_) => return,
            };
            if s.recording_countdown_generation != countdown_gen {
                return;
            }
            if n == 0 {
                begin_recording(&mut s);
                emit_state_updated(&app_clone, &s);
                return;
            }
            s.recording_phase = RecordingPhase::Countdown(n);
            emit_state_updated(&app_clone, &s);
        }
    });
    Ok(())
}

pub(super) fn begin_recording(s: &mut crate::state::AppState) {
    s.recording_phase = RecordingPhase::Active;
    recording::reset_timing();
    seed_recording_mouse_pos(s);
    recording::RECORDING_ACTIVE.store(true, Ordering::Relaxed);
}

/// Seconds the Record button counts down for, 0 to [`crate::MAX_RECORDING_COUNTDOWN`].
pub(crate) fn set_recording_countdown(state: &SharedState, app: &AppHandle, seconds: u8) -> Result<(), String> {
    if seconds > crate::MAX_RECORDING_COUNTDOWN {
        return Err(format!("Countdown out of range: {seconds}"));
    }
    config::update_settings(|settings| settings.recording_countdown_secs = Some(seconds));
    let s = state.lock().map_err(|e| e.to_string())?;
    emit_state_updated(app, &s);
    Ok(())
}

pub(crate) fn stop_recording(
    state: &SharedState,
    app: &AppHandle,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    stop_recording_impl(&mut s);
    emit_state_updated(&app, &s);
    Ok(())
}

/// Returns the macro to auto-save (an owned clone), if recorded instructions
/// actually got appended - saving itself happens after the state lock is
/// released, see `stop_recording_internal`.
pub(super) fn stop_recording_impl(s: &mut crate::state::AppState) -> Option<Macro> {
    recording::RECORDING_ACTIVE.store(false, Ordering::Relaxed);
    s.recording_phase = RecordingPhase::Idle;
    // Cancel any in-progress countdown
    s.recording_countdown_generation = s.recording_countdown_generation.wrapping_add(1);

    let instructions: Vec<Instruction> = recording::get_recording_queue()
        .lock()
        .unwrap()
        .drain(..)
        .collect();
    if instructions.is_empty() {
        return None;
    }
    push_undo(s);
    let mac = s.current_macro.as_mut()?;
    mac.recording_target_mut().instructions.extend(instructions);
    Some(mac.clone())
}

/// Called from the QueueSignal background task when the OS-level hook signals stop.
pub(crate) fn stop_recording_internal(state: &SharedState, app: &AppHandle) {
    let (to_save, dto) = {
        let Ok(mut s) = state.lock() else { return };
        let to_save = stop_recording_impl(&mut s);
        sync_open_macro_into_list(&mut s);
        (to_save, build_state_dto(&s))
    };
    // The disk write (and the emit below) run after the state lock is
    // released. IPC commands go through this same lock (`StartRecordingImmediate`
    // needs it for `reset_timing()`), so holding it across a save made a
    // closely-following next attempt's start signal land inconsistently.
    if let Some(mac) = to_save {
        if let Err(e) = mac.save() {
            warn!("Failed to auto-save macro: {e}");
        } else {
            config::set_selected_macro_id(Some(&mac.id));
        }
    }
    app.emit_state(&dto);
}

pub(crate) async fn toggle_record_mouse_relative(
    state: &SharedState,
    app: &AppHandle,
    relative: bool,
) -> Result<(), String> {
    if !relative && !blockwork_core::macros::backend::absolute_mouse_position_source_available() {
        return Err("Absolute mouse recording isn't available in this session.".to_string());
    }
    if !relative && !blockwork_core::macros::backend::absolute_mouse_position_available() {
        request_absolute_mouse_support(state, app).await?;
    }
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.record_mouse_relative = relative;
    recording::RECORD_MOUSE_RELATIVE.store(relative, Ordering::Relaxed);
    config::update_settings(|settings| settings.record_mouse_relative = Some(relative));
    emit_state_updated(&app, &s);
    Ok(())
}

pub(crate) fn toggle_record_mouse_movement(
    state: &SharedState,
    app: &AppHandle,
    enabled: bool,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.record_mouse_movement = enabled;
    recording::RECORD_MOUSE_MOVEMENT.store(enabled, Ordering::Relaxed);
    config::update_settings(|settings| settings.record_mouse_movement = Some(enabled));
    emit_state_updated(&app, &s);
    Ok(())
}
