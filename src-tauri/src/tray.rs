use crate::state::SharedState;
use tauri::menu::{Menu, MenuBuilder};
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, WebviewWindowBuilder};

/// Builds the tray icon shown while "close to tray" is enabled: left click
/// opens the main window, right click shows the Open/Quit menu ("Quit UI"
/// only shown while the UI is actually running -- see `refresh_menu`).
///
/// Both call sites (startup, and the `set_close_to_tray` command) only ever
/// run while holding the `SharedState` lock with the main window still up,
/// so the initial menu can just assume the UI is open -- don't lock state
/// here to check, that would deadlock against the caller's own lock.
pub(crate) fn build(app: &AppHandle) -> tauri::Result<TrayIcon> {
    ensure_gtk_init(app);

    let menu = build_menu(app, true)?;

    let mut builder = TrayIconBuilder::new()
        .menu(&menu)
        .show_menu_on_left_click(false)
        .tooltip("Blockwork")
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open" => show_main_window(app),
            "quitui" => quit_ui(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                show_main_window(tray.app_handle());
            }
        });

    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }

    builder.build(app)
}

/// Builds the tray's context menu, including "Quit UI" only when `ui_open`.
fn build_menu(app: &AppHandle, ui_open: bool) -> tauri::Result<Menu<tauri::DynRuntime>> {
    let mut builder = MenuBuilder::new(app).text("open", "Open");
    if ui_open {
        builder = builder.text("quitui", "Quit UI");
    }
    builder.text("quit", "Quit").build()
}

/// Rebuilds and swaps in the tray's context menu to reflect whether the UI
/// is currently running -- called right after `main_window_label` flips.
fn refresh_menu(app: &AppHandle) {
    let shared = app.state::<SharedState>();
    let Ok(guard) = shared.lock() else {
        return;
    };
    let Some(tray) = guard.tray_icon.clone() else {
        return;
    };
    let ui_open = guard.main_window_label.is_some();
    drop(guard);

    match build_menu(app, ui_open) {
        Ok(menu) => {
            let _ = tray.set_menu(Some(menu));
        }
        Err(e) => tracing::warn!("Failed to rebuild tray menu: {e}"),
    }
}

/// Shows the main window, recreating it first if `quit_ui` had destroyed it.
pub(crate) fn show_main_window(app: &AppHandle) {
    let label = app.state::<SharedState>().lock().ok().and_then(|s| s.main_window_label.clone());

    let window = match label.and_then(|label| app.get_webview_window(&label)) {
        Some(window) => window,
        None => match rebuild_main_window(app) {
            Ok(window) => window,
            Err(e) => {
                tracing::warn!("Failed to rebuild main window: {e}");
                return;
            }
        },
    };
    let _ = window.show();
    let _ = window.set_focus();
}

/// Builds a fresh main window from the `main` entry in `tauri.conf.json`.
///
/// It gets a never-before-used label rather than reusing `"main"`: the
/// tauri-cef runtime removes a destroyed window from its own bookkeeping
/// before the OS confirms the destruction, so the later confirmation has
/// nothing left to look up and never reaches the window manager -- the
/// manager keeps thinking `"main"` is still alive forever, and handles to it
/// (like the one `show_main_window` would otherwise get from
/// `get_webview_window`) silently do nothing. Reusing the label would also
/// make this `build()` fail outright with `WindowLabelAlreadyExists`.
/// `capabilities/main.json` scopes its permissions to `main*` to cover
/// whatever label ends up live.
fn rebuild_main_window(app: &AppHandle) -> tauri::Result<tauri::WebviewWindow> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT_LABEL: AtomicU64 = AtomicU64::new(1);

    let mut config = app
        .config()
        .app
        .windows
        .iter()
        .find(|w| w.label == "main")
        .cloned()
        .expect("`main` window must be declared in tauri.conf.json");
    config.label = format!("main-{}", NEXT_LABEL.fetch_add(1, Ordering::Relaxed));

    let window = WebviewWindowBuilder::from_config(app, &config)?.build()?;

    if let Ok(mut s) = app.state::<SharedState>().lock() {
        s.main_window_label = Some(config.label);
    }
    refresh_menu(app);

    Ok(window)
}

/// Quits the tauri/chromium window to save memory, but keeps the core
/// running in the background/tray so it can still respond to keybinds and
/// everything. Opening the app again re-inits the tauri window (see
/// `show_main_window`), reusing the same core process.
pub(crate) fn quit_ui(app: &AppHandle) {
    let shared = app.state::<SharedState>();
    let Some(label) = shared.lock().ok().and_then(|s| s.main_window_label.clone()) else {
        return;
    };
    if let Some(window) = app.get_webview_window(&label) {
        if let Ok(mut s) = shared.lock() {
            s.main_window_label = None;
        }
        refresh_menu(app);
        let _ = window.destroy();
    }
}

/// tray-icon's Linux backend (muda + libappindicator) needs GTK initialized
/// before it can build a menu. The wry runtime gets this for free from tao's
/// `EventLoop::new()`, but our CEF runtime's winit event loop never touches
/// GTK, so nothing else in the app will have called `gtk::init()` yet.
#[cfg(target_os = "linux")]
fn ensure_gtk_init(app: &AppHandle) {
    static INIT: std::sync::Once = std::sync::Once::new();
    INIT.call_once(|| {
        let (tx, rx) = std::sync::mpsc::channel();
        if app
            .run_on_main_thread(move || {
                let _ = gtk::init();
                let _ = tx.send(());
            })
            .is_ok()
        {
            let _ = rx.recv();
        }
    });
}

#[cfg(not(target_os = "linux"))]
fn ensure_gtk_init(_app: &AppHandle) {}
