//! Keeps the native window's background in step with the editor's theme.
//!
//! Until the page paints (and again while the window closes, after its browser
//! has already been torn down) the only thing on screen is the window's own
//! background and Chromium's page base color, both white unless set. The
//! theme itself lives in the page's `localStorage`, which this process can't
//! read before the page loads, so the frontend reports it
//! (`set_theme_background`) and the last one is remembered in a small file
//! for the next launch.

use tauri::webview::Color;
use tauri::{AppHandle, Manager, WebviewWindowBuilder};

/// `--blockstitch-bg` from blockstitch's theme CSS.
const DARK: Color = Color(0x1c, 0x1c, 0x1e, 0xff);
const LIGHT: Color = Color(0xf2, 0xf2, 0xf5, 0xff);

fn color(theme: &str) -> Color {
    if theme == "light" { LIGHT } else { DARK }
}

fn theme_file() -> Option<std::path::PathBuf> {
    Some(dirs::cache_dir()?.join("blockwork").join("theme"))
}

/// Builds the `main` window from `tauri.conf.json` (declared with
/// `"create": false`) using the last theme's background color.
pub(crate) fn create_main_window(app: &AppHandle) -> tauri::Result<()> {
    let config = app
        .config()
        .app
        .windows
        .iter()
        .find(|w| w.label == "main")
        .cloned()
        .expect("`main` window must be declared in tauri.conf.json");
    // Dark is blockstitch's default theme, so a first launch starts dark too.
    let theme = theme_file()
        .and_then(|path| std::fs::read_to_string(path).ok())
        .unwrap_or_default();
    WebviewWindowBuilder::from_config(app, &config)?
        .background_color(color(theme.trim()))
        .build()?;
    Ok(())
}

/// Called by the frontend with the theme it resolved (`"light"`/`"dark"`) on
/// startup and whenever it changes.
#[tauri::command]
pub(crate) fn set_theme_background(app: AppHandle, theme: String) {
    let color = color(&theme);
    for window in app.webview_windows().values() {
        let _ = window.set_background_color(Some(color));
    }
    if let Some(path) = theme_file() {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(path, theme);
    }
}
