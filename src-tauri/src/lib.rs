pub(crate) mod battery_watch;
pub(crate) mod commands;
#[cfg(feature = "dev-bridge")]
pub(crate) mod dev_bridge;
pub(crate) mod installed_apps;
pub(crate) mod macros_thread;
pub(crate) mod razer;
pub(crate) mod scheduled_run;
pub(crate) mod single_instance;
pub(crate) mod state;
pub(crate) mod time_watch;
pub(crate) mod tray;

use crate::state::{AppState, ComboCapture, Page, RecordingPhase, SharedState, UpdateCheckState};
use blockwork_core::macros::runner::make_backend;
use blockwork_core::macros::thread_pool::ThreadPool;
use blockwork_core::recording::QueueSignal;
use blockwork_core::{config, recording};
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use tauri::{Manager};

pub fn run() {
    tracing_subscriber::fmt::init();
    let _ = tracing_log::LogTracer::init();

    // CEF re-execs this same binary for its helper processes (renderer, GPU,
    // zygote, ...), tagged with a `--type=` switch -- those fall straight
    // through to `tauri::Builder::run`, which hands them to
    // `cef::execute_process` and exits. The single-instance check below only
    // makes sense for the real browser process.
    let is_cef_subprocess = std::env::args().any(|a| a.starts_with("--type="));

    if !is_cef_subprocess {
        config::migrate_legacy_app_id();
    }

    let activation_listener = if is_cef_subprocess {
        None
    } else {
        match single_instance::claim_or_activate_existing() {
            Some(listener) => Some(listener),
            // Another instance is already running and has been asked to
            // show its window -- don't spin up a second one.
            None => return,
        }
    };

    tauri::Builder::default()
        .runtime(
            tauri_runtime_cef::Cef::default()
                .command_line_args([("--use-mock-keychain", None::<String>)]),
        )
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let close_to_tray = window.state::<SharedState>().lock().map(|s| s.close_to_tray).unwrap_or(false);
                if close_to_tray {
                    if tauri_runtime_cef::is_wayland() {
                        // `window.hide()` is a no-op under Wayland (no
                        // protocol-level way to unmap/remap a toplevel), so
                        // let the close go ahead and treat it like "Quit UI"
                        // from the tray. Calling `quit_ui` directly here
                        // would deadlock: the runtime still holds this
                        // window's listener lock while dispatching
                        // `CloseRequested`.
                        tray::mark_ui_closed(window.app_handle());
                    } else {
                        api.prevent_close();
                        let _ = window.hide();
                    }
                }
            }
        })
        .setup(move |app| {
            let settings = config::load_settings();

            let initial_state = AppState {
                macro_selected: None,
                current_macro: None,
                macros_list: vec![],
                macro_strs: vec![],
                emulator: make_backend(),
                variable_values: Arc::new(Mutex::new(HashMap::new())),
                thread_pool: ThreadPool::new(),
                is_looping: Arc::new(Mutex::new(false)),
                loop_mode_enabled: settings.loop_mode_enabled.unwrap_or(false),
                global_speed_multiplier: settings.global_speed_multiplier.unwrap_or(1.0),
                ipc_server: None,
                ipc_shutdown_tx: None,
                ipc_active_port: None,
                ipc_auto_start: settings.ipc_auto_start.unwrap_or(false),
                close_to_tray: settings.close_to_tray.unwrap_or(false),
                tray_icon: None,
                main_window_label: Some("main".to_string()),
                confirm_clear_instructions: false,
                clear_confirm_remaining_secs: 0,
                clear_confirm_generation: 0,
                key_capture: None,
                pending_standalone_key: None,
                undo_stack: vec![],
                redo_stack: vec![],
                text_edit_session: None,
                recording_phase: RecordingPhase::Idle,
                recording_countdown_generation: 0,
                record_mouse_relative: settings.record_mouse_relative.unwrap_or(true),
                record_mouse_movement: settings.record_mouse_movement.unwrap_or(false),
                page: Page::Main,
                combo_capture: None,
                hotkey_bindings: vec![],
                pending_macro_hotkey: None,
                invalid_field_buffers: HashMap::new(),
                ipc_port_text: settings.ipc_port.unwrap_or(47821).to_string(),
                ipc_port_invalid: false,
                update_check_state: UpdateCheckState::Idle,
                pending_import: None,
            };

            let shared: SharedState = Arc::new(Mutex::new(initial_state));
            app.manage(shared.clone());

            // ── Startup setup ──────────────────────────────────────────────
            {
                let mut s = shared.lock().unwrap();

                // Load macros; create a default one if empty
                let macros = config::get_macros_from_config();
                if macros.is_empty() {
                    let _ = blockwork_core::macros::Macro::new("New Macro".into(), "".into(), vec![]).add();
                }
                let macros = config::get_macros_from_config();
                s.macro_strs = macros.iter().map(|m| m.name.clone()).collect();

                // Restore selection
                if let Some(ref id) = settings.selected_macro_id {
                    if let Some((idx, mac)) = macros.iter().enumerate().find(|(_, m)| &m.id == id) {
                        s.macro_selected = Some(idx);
                        s.current_macro = Some(mac.clone());
                    }
                }
                s.macros_list = macros;

                // The selected macro's live variable store backs reporter
                // previews and execution. Populate it at startup too, not
                // just on UI selection, or persisted variables preview as
                // empty until a reselect.
                let variables = s
                    .current_macro
                    .as_ref()
                    .map(|mac| {
                        mac.variables
                            .iter()
                            .map(|variable| (variable.name.clone(), variable.value.clone()))
                            .collect()
                    })
                    .unwrap_or_default();
                if let Ok(mut store) = s.variable_values.lock() {
                    *store = variables;
                }

                // macOS accessibility
                #[cfg(target_os = "macos")]
                {
                    let trusted = blockwork_core::macros::backend::macos::request_accessibility();
                    if !trusted {
                        recording::set_grab_failed(true);
                    }
                }

                recording::start_grab_thread();
                recording::RECORD_MOUSE_RELATIVE.store(s.record_mouse_relative, Ordering::Relaxed);
                recording::RECORD_MOUSE_MOVEMENT.store(s.record_mouse_movement, Ordering::Relaxed);

                // Chromium grabs raw keyboard input for its own focused
                // window on Windows (chromiumembedded/cef#2609), starving the
                // WH_KEYBOARD_LL hook `start_grab_thread` installs. This CEF
                // callback still sees every keystroke, so it's wired to feed
                // the same hotkey pipeline as a fallback.
                tauri_runtime_cef::set_focused_key_hook(|vk, pressed| {
                    blockwork_core::macros::backend::dispatch_from_focused_window(vk as u16, pressed)
                });

                let bindings = config::load_hotkey_bindings();
                recording::update_hotkey_table(bindings.clone());
                s.hotkey_bindings = bindings;

                // Auto-start IPC server if configured
                if s.ipc_auto_start {
                    if let Ok(port) = s.ipc_port_text.trim().parse::<u16>() {
                        let (tx, rx) = tokio::sync::watch::channel(false);
                        s.ipc_server = Some(tauri::async_runtime::spawn(blockwork_core::ipc::run_server(port, rx)));
                        s.ipc_shutdown_tx = Some(tx);
                        s.ipc_active_port = Some(port);
                    }
                }

                // Show the tray icon if "close to tray" was left enabled from a
                // previous session.
                if s.close_to_tray {
                    match tray::build(app.handle()) {
                        Ok(icon) => s.tray_icon = Some(icon),
                        Err(e) => tracing::warn!("Failed to create tray icon: {e}"),
                    }
                }
            }

            // ── Single-instance activation listener ─────────────────────────
            // A later launch that finds this instance already running
            // connects to the activation port instead of starting its own
            // window; any connection here is that launch asking us to come
            // to the foreground.
            if let Some(listener) = activation_listener {
                let app_handle = app.handle().clone();
                std::thread::spawn(move || {
                    for stream in listener.incoming() {
                        if stream.is_err() {
                            continue;
                        }
                        tracing::info!("Another launch asked us to come to the foreground");
                        let handle = app_handle.clone();
                        let _ = app_handle.run_on_main_thread(move || {
                            tray::show_main_window(&handle);
                        });
                    }
                });
            }

            // ── Dev-only browser bridge (see src/dev_bridge.rs) ────────────
            #[cfg(feature = "dev-bridge")]
            {
                let bridge_handle = app.handle().clone();
                tauri::async_runtime::spawn(crate::dev_bridge::run(bridge_handle));
            }

            // ── Background battery-event watcher (see src/battery_watch.rs) ──
            battery_watch::start(Arc::clone(&shared), app.handle().clone());

            // ── Background time-event watcher (see src/time_watch.rs) ──────
            time_watch::start(Arc::clone(&shared), app.handle().clone());

            // ── QueueSignal consumer (replaces iced hotkey subscription) ──
            let app_handle = app.handle().clone();
            let state_for_task = Arc::clone(&shared);
            tauri::async_runtime::spawn(async move {
                let mut rx = recording::take_queue_receiver();
                while let Some(signal) = rx.recv().await {
                    match signal {
                        QueueSignal::Hotkey(action) => {
                            commands::handle_hotkey_action(&state_for_task, &app_handle, action);
                        }
                        QueueSignal::Stop => {
                            commands::stop_recording_internal(&state_for_task, &app_handle);
                        }
                    }
                }
            });

            // ── Delayed update check (Windows/macOS only) ──────────────────
            #[cfg(any(windows, target_os = "macos"))]
            {
                let state_w = Arc::clone(&shared);
                let app_w = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                    commands::check_for_updates_internal(&state_w, &app_w).await;
                });
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_state,
            commands::select_macro,
            commands::new_macro,
            commands::remove_macro,
            commands::set_title,
            commands::set_macro_speed_multiplier,
            commands::set_macro_always_listen,
            commands::save_macro,
            commands::export_macro,
            commands::import_macro,
            commands::confirm_import_macro,
            commands::cancel_import_macro,
            commands::add_instruction,
            commands::edit_instruction,
            commands::create_variable,
            commands::rename_variable,
            commands::delete_variable,
            commands::create_block,
            commands::edit_block,
            commands::delete_block,
            commands::edit_value_field,
            commands::set_value_kind,
            commands::take_value,
            commands::put_value,
            commands::preview_value,
            commands::create_floating_value,
            commands::move_floating_value,
            commands::remove_floating_value,
            commands::create_comment,
            commands::create_attached_comment,
            commands::move_comment,
            commands::remove_comment,
            commands::edit_comment_text,
            commands::set_comment_collapsed,
            commands::remove_instruction,
            commands::reorder_instruction,
            commands::clear_instructions,
            commands::add_strand,
            commands::remove_strand,
            commands::move_strand,
            commands::split_strand,
            commands::merge_strand,
            commands::delete_instruction,
            commands::paste_instructions,
            commands::set_recording_target,
            commands::undo,
            commands::redo,
            commands::start_key_capture,
            commands::start_standalone_key_capture,
            commands::key_capture_event,
            commands::clear_standalone_key_capture,
            commands::run_macro,
            commands::toggle_loop_mode,
            commands::set_global_speed_multiplier,
            commands::start_recording,
            commands::stop_recording,
            commands::toggle_record_mouse_relative,
            commands::toggle_record_mouse_movement,
            commands::open_settings,
            commands::close_settings,
            commands::reset_zoom,
            commands::start_combo_capture,
            commands::start_pending_combo_capture,
            commands::combo_capture_event,
            commands::cancel_combo_capture,
            commands::set_pending_macro_idx,
            commands::add_macro_hotkey,
            commands::remove_hotkey_binding,
            commands::clear_named_hotkey,
            commands::reset_hotkey_to_default,
            commands::set_ipc_port_text,
            commands::start_ipc_server,
            commands::stop_ipc_server,
            commands::set_ipc_auto_start,
            commands::set_close_to_tray,
            commands::check_for_updates,
            commands::apply_update,
            commands::list_installed_apps,
        ])
        .build(tauri::generate_context!())
        .expect("error while running tauri application")
        .run(|app, event| {
            // `code` is `None` when exit was requested by the last window
            // closing rather than an explicit `AppHandle::exit`. That covers
            // both a genuine "close the last window" (should quit) and
            // `tray::quit_ui` destroying the main window to save memory
            // (should not) -- `quit_ui` clears `main_window_label` right
            // before destroying the window, distinguishing the two.
            if let tauri::RunEvent::ExitRequested { code: None, api, .. } = event {
                let ui_quit = app
                    .state::<SharedState>()
                    .lock()
                    .map(|s| s.main_window_label.is_none())
                    .unwrap_or(false);
                if ui_quit {
                    api.prevent_exit();
                }
            }
        });
}
