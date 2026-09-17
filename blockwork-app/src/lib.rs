//! Blockwork's backend: every piece of app state and every command the editor
//! UI can issue, with no dependency on Tauri or CEF. It runs inside
//! `blockwork-daemon`, which outlives the UI so hotkeys, recording and
//! scheduled macros keep working while the (memory-hungry) CEF window is
//! closed. The UI reaches it through [`Backend::dispatch`] over the daemon's
//! local socket (see `blockwork-protocol`).

pub(crate) mod battery_watch;
pub(crate) mod commands;
mod dispatch;
pub(crate) mod installed_apps;
pub(crate) mod macros_thread;
pub(crate) mod razer;
pub(crate) mod scheduled_run;
pub(crate) mod state;
pub(crate) mod time_watch;

use crate::state::{AppState, Page, RecordingPhase, SharedState, UpdateCheckState};
use blockwork_core::macros::runner::make_backend;
use blockwork_core::macros::thread_pool::ThreadPool;
use blockwork_core::recording::QueueSignal;
use blockwork_core::{config, recording};
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;

/// Feeds a key event CEF saw in the focused UI window into the hotkey
/// pipeline (Windows only; see `ClientMessage::KeyHook`).
pub use blockwork_core::macros::backend::dispatch_from_focused_window;

pub use blockwork_core::config::migrate_legacy_app_id;

/// Something the backend tells whoever is hosting it.
#[derive(Clone, Debug)]
pub enum Event {
    /// The full `StateDto`, already serialized to JSON, after any change.
    State(Arc<str>),
    /// "Close to tray" was toggled, so the host should show/hide its tray icon.
    CloseToTray(bool),
    /// The app should shut down entirely (e.g. an update was just applied).
    Quit,
}

/// Cheap, cloneable handle commands and background threads use to publish
/// [`Event`]s - the stand-in for the `tauri::AppHandle` these modules used to
/// emit `state-updated` through.
#[derive(Clone)]
pub struct AppHandle {
    events: broadcast::Sender<Event>,
}

impl AppHandle {
    pub(crate) fn emit_state<T: serde::Serialize>(&self, dto: &T) {
        match serde_json::to_string(dto) {
            Ok(json) => self.send(Event::State(json.into())),
            Err(e) => tracing::warn!("Failed to serialize state: {e}"),
        }
    }

    pub(crate) fn send(&self, event: Event) {
        // No subscribers (no UI connected yet) is fine: the next one to
        // connect asks for a fresh snapshot anyway.
        let _ = self.events.send(event);
    }
}

pub(crate) mod async_runtime {
    use std::future::Future;
    use std::sync::OnceLock;

    static HANDLE: OnceLock<tokio::runtime::Handle> = OnceLock::new();

    pub(crate) fn init(handle: tokio::runtime::Handle) {
        let _ = HANDLE.set(handle);
    }

    /// Spawns onto the backend's runtime from any thread - hotkey, watcher
    /// and macro threads are plain `std::thread`s outside any tokio context.
    pub(crate) fn spawn<F>(future: F) -> tokio::task::JoinHandle<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        HANDLE.get().expect("async_runtime::init was not called").spawn(future)
    }
}

/// The running backend. Cloning shares the same state.
#[derive(Clone)]
pub struct Backend {
    state: SharedState,
    app: AppHandle,
}

impl Backend {
    /// Loads settings and macros and starts every background service
    /// (input grab, hotkeys, IPC auto-start, battery/time watchers, update
    /// check). Must be called from within `runtime`'s context or with a
    /// handle to it.
    pub fn start(runtime: tokio::runtime::Handle) -> Backend {
        async_runtime::init(runtime);
        let (events, _) = broadcast::channel(256);
        let app = AppHandle { events };

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

            let bindings = config::load_hotkey_bindings();
            recording::update_hotkey_table(bindings.clone());
            s.hotkey_bindings = bindings;

            // Auto-start IPC server if configured
            if s.ipc_auto_start {
                if let Ok(port) = s.ipc_port_text.trim().parse::<u16>() {
                    let (tx, rx) = tokio::sync::watch::channel(false);
                    s.ipc_server = Some(async_runtime::spawn(blockwork_core::ipc::run_server(port, rx)));
                    s.ipc_shutdown_tx = Some(tx);
                    s.ipc_active_port = Some(port);
                }
            }
        }

        // ── Background battery-event watcher (see battery_watch.rs) ──────
        battery_watch::start(Arc::clone(&shared), app.clone());

        // ── Background time-event watcher (see time_watch.rs) ────────────
        time_watch::start(Arc::clone(&shared), app.clone());

        // ── QueueSignal consumer (hotkeys and the recording stop key) ────
        {
            let app = app.clone();
            let state = Arc::clone(&shared);
            async_runtime::spawn(async move {
                let mut rx = recording::take_queue_receiver();
                while let Some(signal) = rx.recv().await {
                    match signal {
                        QueueSignal::Hotkey(action) => commands::handle_hotkey_action(&state, &app, action),
                        QueueSignal::Stop => commands::stop_recording_internal(&state, &app),
                    }
                }
            });
        }

        // ── Delayed update check (Windows/macOS only) ────────────────────
        #[cfg(any(windows, target_os = "macos"))]
        {
            let state = Arc::clone(&shared);
            let app = app.clone();
            async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                commands::check_for_updates_internal(&state, &app).await;
            });
        }

        Backend { state: shared, app }
    }

    /// Receives every [`Event`] published from now on.
    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.app.events.subscribe()
    }

    /// The current `StateDto` as JSON, for a newly connected UI.
    pub fn state_json(&self) -> Result<String, String> {
        let s = self.state.lock().map_err(|e| e.to_string())?;
        serde_json::to_string(&state::build_state_dto(&s)).map_err(|e| e.to_string())
    }

    pub fn close_to_tray(&self) -> bool {
        self.state.lock().map(|s| s.close_to_tray).unwrap_or(false)
    }

    /// Asks every subscriber to shut down (publishes [`Event::Quit`]).
    pub fn request_quit(&self) {
        self.app.send(Event::Quit);
    }
}
