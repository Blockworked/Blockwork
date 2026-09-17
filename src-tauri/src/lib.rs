//! Blockwork's editor UI: a Tauri app on a CEF runtime. It holds no app
//! state of its own - everything lives in `blockwork-daemon`, which this
//! process starts on demand and forwards every frontend command to (see
//! `daemon.rs`). Closing the window ends this process, and with it every
//! Chromium process, while the daemon keeps hotkeys and scheduled macros
//! running if "close to tray" is on.

mod daemon;
mod theme;

use crate::daemon::{Connected, Daemon};
use serde_json::Value;
use std::sync::Arc;
use tauri::{Manager, State};

pub fn run() {
    // `blockwork --daemon` runs the daemon instead (see `daemon::exec_daemon`).
    if std::env::args().nth(1).as_deref() == Some(daemon::DAEMON_ARG) {
        daemon::exec_daemon();
    }

    tracing_subscriber::fmt::init();
    let _ = tracing_log::LogTracer::init();

    // CEF re-execs this same binary for its helper processes (renderer, GPU,
    // zygote, ...), tagged with a `--type=` switch -- those must fall
    // straight through to `tauri::Builder::run`, which hands them to
    // `cef::execute_process` and exits. Only the real browser process talks
    // to the daemon.
    let is_cef_subprocess = std::env::args().any(|a| a.starts_with("--type="));

    let connection = if is_cef_subprocess {
        None
    } else {
        match tauri::async_runtime::block_on(daemon::connect()) {
            Ok(Connected::Ui(daemon, reader)) => Some((daemon, reader)),
            // Another window is already open and has been brought to the
            // front -- don't open a second one.
            Ok(Connected::AlreadyOpen) => {
                tracing::info!("Blockwork is already open; brought its window to the front");
                return;
            }
            Err(e) => {
                tracing::error!("Couldn't reach the Blockwork daemon: {e}");
                std::process::exit(1);
            }
        }
    };

    let mut builder = tauri::Builder::default().runtime(
        tauri_runtime_cef::Cef::default().command_line_args([("--use-mock-keychain", None::<String>)]),
    );

    if let Some((daemon, reader)) = connection {
        // Chromium grabs raw keyboard input for its own focused window on
        // Windows (chromiumembedded/cef#2609), starving the daemon's
        // WH_KEYBOARD_LL hook while this window has focus. This CEF callback
        // still sees every keystroke, so forward it to the same hotkey
        // pipeline as a fallback.
        #[cfg(windows)]
        {
            let daemon = Arc::clone(&daemon);
            tauri_runtime_cef::set_focused_key_hook(move |vk, pressed| {
                daemon
                    .call_blocking(
                        blockwork_protocol::FOCUSED_KEY_EVENT.to_string(),
                        serde_json::json!({ "vk": vk, "pressed": pressed }),
                        std::time::Duration::from_millis(50),
                    )
                    .and_then(|suppress| suppress.as_bool())
                    .unwrap_or(false)
            });
        }

        builder = builder.manage(Arc::clone(&daemon)).setup(move |app| {
            theme::create_main_window(app.handle())?;
            tauri::async_runtime::spawn(daemon::pump(app.handle().clone(), daemon, reader));
            Ok(())
        });
    }

    builder
        .invoke_handler(tauri::generate_handler![
            daemon_call,
            reset_zoom,
            pick_macro_file,
            theme::set_theme_background
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Forwards a frontend command to the daemon (see `ui/src/bridge.ts`).
#[tauri::command]
async fn daemon_call(daemon: State<'_, Arc<Daemon>>, cmd: String, args: Value) -> Result<Value, String> {
    daemon.call(cmd, args).await
}

/// Resets Chromium page zoom to 100%. Handled here rather than by the
/// browser's own Ctrl+0 accelerator, which this CEF runtime can't be relied
/// on to deliver (opt-in per webview; absent entirely on Alloy-style
/// webviews).
#[tauri::command]
fn reset_zoom(app: tauri::AppHandle) -> Result<(), String> {
    let windows = app.webview_windows();
    if windows.is_empty() {
        return Err("no app window".to_string());
    }
    for window in windows.values() {
        window.set_zoom(1.0).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Shows a file dialog for a `.macro` file - a save dialog pre-filled with
/// `default_name` when `save` is set, an open dialog otherwise. Runs here
/// rather than in the daemon so the dialog belongs to the editor window.
/// Returns `None` if the user cancelled.
#[tauri::command]
async fn pick_macro_file(save: bool, default_name: Option<String>) -> Option<String> {
    let dialog = rfd::AsyncFileDialog::new().add_filter("Macro", &["macro"]);
    let file = if save {
        dialog
            .set_title("Export Macro")
            .set_file_name(default_name.unwrap_or_default())
            .save_file()
            .await
    } else {
        dialog.set_title("Import Macro").pick_file().await
    };
    file.map(|file| file.path().to_string_lossy().into_owned())
}
