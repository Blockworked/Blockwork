//! What a hotkey does when it fires.

use super::*;

pub(crate) fn handle_hotkey_action(state: &SharedState, app: &AppHandle, action: HotkeyAction) {
    match &action {
        HotkeyAction::RunMacro | HotkeyAction::RunSpecificMacro(_) => {
            let s = match state.lock() {
                Ok(g) => g,
                Err(_) => return,
            };
            if let Some(emulator) = s.emulator.as_ref() {
                let emulator = Arc::clone(emulator);
                let is_looping = Arc::clone(&s.is_looping);
                let global_speed_multiplier = s.global_speed_multiplier;
                let shared_state = Arc::clone(state);
                let mac = match &action {
                    HotkeyAction::RunMacro => s.current_macro.clone(),
                    HotkeyAction::RunSpecificMacro(id) => {
                        let macs = s.macros_list.clone();
                        macs.iter().find(|m| &m.id == id).cloned()
                    }
                    _ => None,
                };
                drop(s);
                if let Some(mac) = mac {
                    if let Some(reason) = mac.settings.blocked_by() {
                        let source = blockwork_core::run_log::Source::for_macro(&mac.id, &mac.name);
                        blockwork_core::run_log::info(&source, format!("Hotkey ignored: {reason}"));
                        return;
                    }
                    let speed_multiplier = mac.speed_multiplier * global_speed_multiplier;
                    // Seeded from this macro's own persisted values, not
                    // `AppState::variable_values` (tracks the *selected* macro).
                    let variables: VariableStore = Arc::new(Mutex::new(
                        mac.variables
                            .iter()
                            .map(|v| (v.name.clone(), v.value.clone()))
                            .collect(),
                    ));
                    let lists: ListStore = Arc::new(Mutex::new(
                        mac.lists
                            .iter()
                            .map(|list| (list.name.clone(), list.items.clone()))
                            .collect(),
                    ));
                    run_macro_task(
                        mac,
                        emulator,
                        is_looping,
                        speed_multiplier,
                        variables,
                        lists,
                        shared_state,
                        app.clone(),
                    );
                }
            }
        }
        HotkeyAction::EmergencyStop => emergency_stop(state, app),
        HotkeyAction::StopLoop => {
            // Clears every in-flight run's stop flag, not just loop mode's -
            // a single run has its own stop flag that `is_looping` doesn't reach.
            let cleared = blockwork_core::macros::run_registry::stop_all();
            tracing::info!(cleared, "StopLoop hotkey handled");
            if let Ok(s) = state.lock() {
                if let Ok(mut lk) = s.is_looping.lock() {
                    *lk = false;
                }
                emit_state_updated(app, &s);
            }
        }
        HotkeyAction::NextMacro => {
            let macros = config::get_macros_from_config();
            if !macros.is_empty()
                && let Ok(mut s) = state.lock()
            {
                let current_id = config::get_selected_macro_id();
                let current_idx = current_id
                    .and_then(|id| macros.iter().position(|m| m.id == id))
                    .unwrap_or(0);
                let next = (current_idx + 1) % macros.len();
                s.macro_selected = Some(next);
                s.current_macro = macros.get(next).cloned();
                if let Some(mac) = &s.current_macro {
                    config::set_selected_macro_id(Some(&mac.id));
                }
                sync_variable_values(&mut s);
                sync_list_values(&mut s);
                emit_state_updated(app, &s);
            }
        }
        HotkeyAction::PrevMacro => {
            let macros = config::get_macros_from_config();
            if !macros.is_empty()
                && let Ok(mut s) = state.lock()
            {
                let current_id = config::get_selected_macro_id();
                let current_idx = current_id
                    .and_then(|id| macros.iter().position(|m| m.id == id))
                    .unwrap_or(0);
                let prev = if current_idx == 0 {
                    macros.len() - 1
                } else {
                    current_idx - 1
                };
                s.macro_selected = Some(prev);
                s.current_macro = macros.get(prev).cloned();
                if let Some(mac) = &s.current_macro {
                    config::set_selected_macro_id(Some(&mac.id));
                }
                sync_variable_values(&mut s);
                sync_list_values(&mut s);
                emit_state_updated(app, &s);
            }
        }
        HotkeyAction::StartRecordingImmediate => {
            if let Ok(mut s) = state.lock()
                && s.current_macro.is_some()
            {
                s.recording_countdown_generation = s.recording_countdown_generation.wrapping_add(1);
                begin_recording(&mut s);
                emit_state_updated(app, &s);
            }
        }
        HotkeyAction::StopRecording => {
            // Handled directly in recording::start_grab_thread while active;
            // reached here only if pressed while idle - no-op.
        }
        HotkeyAction::Undo => {
            let _ = undo(state, app);
        }
        HotkeyAction::Redo => {
            let _ = redo(state, app);
        }
    }
}

pub(super) fn run_macro_task(
    mac: Macro,
    emulator: Arc<std::sync::Mutex<dyn blockwork_core::macros::backend::InputBackend>>,
    is_looping: Arc<std::sync::Mutex<bool>>,
    speed_multiplier: f64,
    variables: VariableStore,
    lists: ListStore,
    state: SharedState,
    app: AppHandle,
) {
    // Kept alive past the move into the task so the pre-run focus/modifier
    // cleanup can play its releases through the very backend the run will use.
    #[cfg(windows)]
    let prep_backend = Arc::clone(&emulator);

    if let Ok(mut st) = is_looping.lock() {
        *st = true;
    }
    let stop_flag = Arc::clone(&is_looping);
    let task = macros_thread::into_single_run_task(
        mac,
        emulator,
        stop_flag,
        speed_multiplier,
        variables,
        lists,
        state,
        app,
        None,
    );
    tokio::task::spawn_blocking(move || {
        // Same pre-registration as the loop branch above.
        let playback_guard = blockwork_core::macros::run_registry::begin_run();
        #[cfg(windows)]
        blockwork_core::macros::backend::windows::prepare_for_macro_execution(&prep_backend);
        #[cfg(not(windows))]
        blockwork_core::macros::backend::release_held_modifiers();
        task();
        blockwork_core::macros::run_registry::end_run(&playback_guard);
    });
}
