//! Navigation, hotkey bindings, the IPC server, tray, appearance and updates.

use super::*;

pub(crate) fn open_settings(
    state: &SharedState,
    app: &AppHandle,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.page = Page::Settings;
    s.hotkey_bindings = config::load_hotkey_bindings();
    s.combo_capture = None;
    s.pending_macro_hotkey = None;
    emit_state_updated(&app, &s);
    Ok(())
}

pub(crate) fn close_settings(
    state: &SharedState,
    app: &AppHandle,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.page = Page::Main;
    s.combo_capture = None;
    s.pending_macro_hotkey = None;
    emit_state_updated(&app, &s);
    Ok(())
}


// ─── Hotkey bindings ───────────────────────────────────────────────────────

pub(crate) fn start_combo_capture(
    state: &SharedState,
    app: &AppHandle,
    action: HotkeyActionDto,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.combo_capture = Some(ComboCapture::Named(dto_to_hotkey_action(&action)));
    emit_state_updated(&app, &s);
    Ok(())
}

pub(crate) fn start_binding_combo_capture(
    state: &SharedState,
    app: &AppHandle,
    index: usize,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if index < s.hotkey_bindings.len() {
        s.combo_capture = Some(ComboCapture::Binding(index));
    }
    emit_state_updated(&app, &s);
    Ok(())
}

pub(crate) fn start_pending_combo_capture(
    state: &SharedState,
    app: &AppHandle,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.combo_capture = Some(ComboCapture::Pending);
    emit_state_updated(&app, &s);
    Ok(())
}

pub(crate) fn combo_capture_event(
    state: &SharedState,
    app: &AppHandle,
    code: String,
    modifiers: u8,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    let key_name = match blockwork_core::key_mapping::web_code_to_rdev_name(&code) {
        Some(n) => n,
        None => return Ok(()),
    };
    let combo = KeyCombo {
        modifiers,
        key: key_name,
    };

    match s.combo_capture.take() {
        Some(ComboCapture::Named(action)) => {
            // StopRecording must never require a combo: modifier keys held
            // while it fires would themselves have already been captured as
            // macro steps before the trigger key arrives.
            let combo = if matches!(action, HotkeyAction::StopRecording) {
                KeyCombo {
                    modifiers: 0,
                    ..combo
                }
            } else {
                combo
            };
            if let Some(existing) = s.hotkey_bindings.iter_mut().find(|b| b.action == action) {
                existing.combo = combo;
            } else {
                s.hotkey_bindings.push(HotkeyBinding { action, combo });
            }
            save_hotkey_bindings_impl(&mut s);
        }
        Some(ComboCapture::Binding(index)) => {
            if let Some(existing) = s.hotkey_bindings.get_mut(index) {
                existing.combo = combo;
                save_hotkey_bindings_impl(&mut s);
            }
        }
        Some(ComboCapture::Pending) => {
            let entry = s.pending_macro_hotkey.get_or_insert((None, None));
            entry.1 = Some(combo);
        }
        None => {}
    }
    emit_state_updated(&app, &s);
    Ok(())
}

pub(crate) fn cancel_combo_capture(
    state: &SharedState,
    app: &AppHandle,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.combo_capture = None;
    emit_state_updated(&app, &s);
    Ok(())
}

pub(crate) fn set_pending_macro_idx(
    state: &SharedState,
    app: &AppHandle,
    index: Option<usize>,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    let entry = s.pending_macro_hotkey.get_or_insert((None, None));
    entry.0 = index;
    emit_state_updated(&app, &s);
    Ok(())
}

pub(crate) fn add_macro_hotkey(
    state: &SharedState,
    app: &AppHandle,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if let Some((Some(idx), Some(combo))) = s.pending_macro_hotkey.take() {
        let macros = config::get_macros_from_config();
        if let Some(mac) = macros.get(idx) {
            let binding = HotkeyBinding {
                action: HotkeyAction::RunSpecificMacro(mac.id.clone()),
                combo,
            };
            s.hotkey_bindings.push(binding);
            save_hotkey_bindings_impl(&mut s);
        }
    }
    emit_state_updated(&app, &s);
    Ok(())
}

pub(crate) fn remove_hotkey_binding(
    state: &SharedState,
    app: &AppHandle,
    index: usize,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if index < s.hotkey_bindings.len() {
        s.hotkey_bindings.remove(index);
        save_hotkey_bindings_impl(&mut s);
    }
    emit_state_updated(&app, &s);
    Ok(())
}

pub(crate) fn clear_named_hotkey(
    state: &SharedState,
    app: &AppHandle,
    action: HotkeyActionDto,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    let action = dto_to_hotkey_action(&action);
    s.hotkey_bindings.retain(|b| b.action != action);
    save_hotkey_bindings_impl(&mut s);
    emit_state_updated(&app, &s);
    Ok(())
}

pub(crate) fn reset_hotkey_to_default(
    state: &SharedState,
    app: &AppHandle,
    action: HotkeyActionDto,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    let action = dto_to_hotkey_action(&action);
    if let Some(default_combo) = config::default_combo_for_action(&action) {
        if let Some(existing) = s.hotkey_bindings.iter_mut().find(|b| b.action == action) {
            existing.combo = default_combo;
        } else {
            s.hotkey_bindings.push(HotkeyBinding {
                action,
                combo: default_combo,
            });
        }
        save_hotkey_bindings_impl(&mut s);
    }
    emit_state_updated(&app, &s);
    Ok(())
}

pub(super) fn save_hotkey_bindings_impl(s: &mut crate::state::AppState) {
    config::save_hotkey_bindings(&s.hotkey_bindings);
    recording::update_hotkey_table(s.hotkey_bindings.clone());
}

// ─── IPC server ────────────────────────────────────────────────────────────

pub(crate) fn set_ipc_port_text(
    state: &SharedState,
    app: &AppHandle,
    text: String,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    match text.trim().parse::<u16>() {
        Ok(port) => {
            s.ipc_port_invalid = false;
            config::update_settings(|settings| settings.ipc_port = Some(port));
        }
        Err(_) => {
            s.ipc_port_invalid = true;
        }
    }
    s.ipc_port_text = text;
    emit_state_updated(&app, &s);
    Ok(())
}

pub(crate) async fn start_ipc_server(
    state: &SharedState,
    app: &AppHandle,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if s.ipc_server.is_none() {
        if let Ok(port) = s.ipc_port_text.trim().parse::<u16>() {
            let (tx, rx) = tokio::sync::watch::channel(false);
            s.ipc_server = Some(crate::async_runtime::spawn(
                blockwork_core::ipc::run_server(port, rx),
            ));
            s.ipc_shutdown_tx = Some(tx);
            s.ipc_active_port = Some(port);
        }
    }
    emit_state_updated(&app, &s);
    Ok(())
}

pub(crate) fn stop_ipc_server(
    state: &SharedState,
    app: &AppHandle,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if let Some(tx) = s.ipc_shutdown_tx.take() {
        let _ = tx.send(true);
    }
    if let Some(handle) = s.ipc_server.take() {
        handle.abort();
    }
    s.ipc_active_port = None;
    emit_state_updated(&app, &s);
    Ok(())
}

pub(crate) fn set_ipc_auto_start(
    state: &SharedState,
    app: &AppHandle,
    enabled: bool,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.ipc_auto_start = enabled;
    config::update_settings(|settings| settings.ipc_auto_start = Some(enabled));
    emit_state_updated(&app, &s);
    Ok(())
}

// ─── System tray ────────────────────────────────────────────────────────────

pub(crate) fn set_close_to_tray(
    state: &SharedState,
    app: &AppHandle,
    enabled: bool,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.close_to_tray = enabled;
    config::update_settings(|settings| settings.close_to_tray = Some(enabled));
    app.send(crate::Event::CloseToTray(enabled));
    emit_state_updated(app, &s);
    Ok(())
}

pub(super) const PALETTE_LAYOUTS: [&str; 5] = ["rail", "chips", "accordion", "flyout", "tiles"];

pub(crate) fn set_palette_layout(
    state: &SharedState,
    app: &AppHandle,
    layout: String,
) -> Result<(), String> {
    if !PALETTE_LAYOUTS.contains(&layout.as_str()) {
        return Err(format!("Unknown palette layout: {layout}"));
    }
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.palette_layout = layout.clone();
    config::update_settings(|settings| settings.palette_layout = Some(layout));
    emit_state_updated(app, &s);
    Ok(())
}

/// The UI language, like "de"; empty follows the system.
pub(crate) fn set_language(state: &SharedState, app: &AppHandle, language: String) -> Result<(), String> {
    let language = language.trim().to_string();
    if !language.is_empty() && !language.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
        return Err(format!("Unknown language: {language}"));
    }
    config::update_settings(|settings| settings.language = (!language.is_empty()).then_some(language));
    let s = state.lock().map_err(|e| e.to_string())?;
    emit_state_updated(app, &s);
    Ok(())
}

/// How times show: "12h", "24h", or empty to follow the system locale.
pub(crate) fn set_clock_format(state: &SharedState, app: &AppHandle, format: String) -> Result<(), String> {
    if !matches!(format.as_str(), "" | "12h" | "24h") {
        return Err(format!("Unknown clock format: {format}"));
    }
    config::update_settings(|settings| settings.clock_format = (!format.is_empty()).then_some(format));
    let s = state.lock().map_err(|e| e.to_string())?;
    emit_state_updated(app, &s);
    Ok(())
}

/// Empty follows the system appearance.
pub(crate) fn set_color_scheme(state: &SharedState, app: &AppHandle, scheme: String) -> Result<(), String> {
    if !matches!(scheme.as_str(), "" | "light" | "dark") {
        return Err(format!("Unknown color scheme: {scheme}"));
    }
    config::update_settings(|settings| settings.color_scheme = (!scheme.is_empty()).then_some(scheme));
    let s = state.lock().map_err(|e| e.to_string())?;
    emit_state_updated(app, &s);
    Ok(())
}

/// How long UI animations take, as a duration multiplier: 0 is instant, 1 normal.
pub(crate) fn set_animation_speed(state: &SharedState, app: &AppHandle, speed: f64) -> Result<(), String> {
    if !speed.is_finite() || !(0.0..=crate::MAX_ANIMATION_SPEED).contains(&speed) {
        return Err(format!("Animation speed out of range: {speed}"));
    }
    config::update_settings(|settings| settings.animation_speed = Some(speed));
    let s = state.lock().map_err(|e| e.to_string())?;
    emit_state_updated(app, &s);
    Ok(())
}

pub(crate) fn set_start_at_startup(
    state: &SharedState,
    app: &AppHandle,
    enabled: bool,
) -> Result<(), String> {
    crate::autostart::set_enabled(enabled)?;
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.start_at_startup = enabled;
    config::update_settings(|settings| settings.start_at_startup = Some(enabled));
    emit_state_updated(app, &s);
    Ok(())
}

pub(crate) fn set_onboarding_done(
    state: &SharedState,
    app: &AppHandle,
    done: bool,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.onboarding_done = done;
    if !done {
        s.page = Page::Main;
    }
    config::update_settings(|settings| settings.onboarding_done = Some(done));
    emit_state_updated(app, &s);
    Ok(())
}

// ─── Updates (Windows/macOS) ───────────────────────────────────────────────

pub(crate) fn check_for_updates(
    state: &SharedState,
    app: &AppHandle,
) -> Result<(), String> {
    {
        let mut s = state.lock().map_err(|e| e.to_string())?;
        s.update_check_state = UpdateCheckState::Checking;
        emit_state_updated(&app, &s);
    }
    let state_clone = Arc::clone(&*state);
    let app_clone = app.clone();
    crate::async_runtime::spawn(async move {
        check_for_updates_internal(&state_clone, &app_clone).await;
    });
    Ok(())
}

pub(crate) async fn check_for_updates_internal(
    state: &SharedState,
    app: &AppHandle,
) {
    #[cfg(any(windows, target_os = "macos"))]
    {
        let version = env!("CARGO_PKG_VERSION").to_string();
        let result = tokio::task::spawn_blocking(move || {
            blockwork_core::updater::check_for_update(&version)
        })
        .await
        .unwrap_or_else(|e| Err(e.to_string()));
        if let Ok(mut s) = state.lock() {
            s.update_check_state = match result {
                Ok(Some(info)) => UpdateCheckState::UpdateAvailable(info.version),
                Ok(None) => UpdateCheckState::UpToDate,
                Err(e) => UpdateCheckState::Error(e),
            };
            emit_state_updated(app, &s);
        }
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    if let Ok(mut s) = state.lock() {
        s.update_check_state =
            UpdateCheckState::Error("Updates are only supported on Windows and macOS".to_string());
        emit_state_updated(app, &s);
    }
}

pub(crate) fn apply_update(
    state: &SharedState,
    app: &AppHandle,
) -> Result<(), String> {
    {
        let mut s = state.lock().map_err(|e| e.to_string())?;
        s.update_check_state = UpdateCheckState::Applying;
        emit_state_updated(&app, &s);
    }
    #[cfg(any(windows, target_os = "macos"))]
    {
        let state_clone = Arc::clone(&*state);
        let app_clone = app.clone();
        crate::async_runtime::spawn(async move {
            let version = env!("CARGO_PKG_VERSION").to_string();
            let result = tokio::task::spawn_blocking(move || {
                blockwork_core::updater::apply_update(&version)
            })
            .await
            .unwrap_or_else(|e| Err(e.to_string()));
            match result {
                Ok(_) => app_clone.send(crate::Event::Quit),
                Err(e) => {
                    if let Ok(mut s) = state_clone.lock() {
                        s.update_check_state = UpdateCheckState::Error(e);
                        emit_state_updated(&app_clone, &s);
                    }
                }
            }
        });
    }
    Ok(())
}
