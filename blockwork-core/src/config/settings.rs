use crate::hotkey_types::HotkeyBinding;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::SystemTime;
use tracing::warn;

/// Same rationale as `MACRO_CACHE` in `persistence.rs`: `load_settings()` is
/// called on every `RunMacro` dispatch (i.e. every level attempt) by a
/// long-lived, stateless reader (the Linux Wine bridge process), and used to
/// re-read-and-reparse `settings.json` from disk every single time. Cached
/// by mtime so a repeat call for an unchanged file costs one `stat()` instead
/// of a read + JSON parse.
static SETTINGS_CACHE: OnceLock<Mutex<Option<(SystemTime, AppSettings)>>> = OnceLock::new();

fn settings_cache() -> &'static Mutex<Option<(SystemTime, AppSettings)>> {
    SETTINGS_CACHE.get_or_init(|| Mutex::new(None))
}

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct AppSettings {
    pub loop_mode_enabled: Option<bool>,
    pub ipc_port: Option<u16>,
    pub ipc_auto_start: Option<bool>,
    pub global_hotkeys: Option<Vec<HotkeyBinding>>,
    pub selected_macro_id: Option<String>,
    /// Runtime-only playback speed override applied on top of each macro's
    /// own `speed_multiplier`. Persisted like the rest of `AppSettings`, but
    /// conceptually a session knob rather than a macro property.
    pub global_speed_multiplier: Option<f64>,
    /// When true, closing the main window hides it to the system tray
    /// instead of exiting the app.
    pub close_to_tray: Option<bool>,
    /// When true, recorded mouse movement is captured as relative deltas;
    /// when false, as absolute positions. Only relevant when
    /// `record_mouse_movement` is also true.
    pub record_mouse_relative: Option<bool>,
    /// Whether mouse movement is captured into recordings at all.
    pub record_mouse_movement: Option<bool>,
}

fn settings_path() -> Result<PathBuf, String> {
    let mut p = super::config_root()?;
    std::fs::create_dir_all(&p)
        .map_err(|e| format!("Failed to create config dir: {e}"))?;
    p.push("settings.json");
    Ok(p)
}

pub fn load_settings() -> AppSettings {
    let Some(path) = settings_path().ok() else { return AppSettings::default() };
    let Ok(mtime) = std::fs::metadata(&path).and_then(|m| m.modified()) else {
        return AppSettings::default();
    };

    {
        let cache = settings_cache().lock().unwrap();
        if let Some((cached_mtime, settings)) = cache.as_ref() {
            if *cached_mtime == mtime {
                return settings.clone();
            }
        }
    }

    let settings: AppSettings = std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    *settings_cache().lock().unwrap() = Some((mtime, settings.clone()));
    settings
}

pub fn save_settings(settings: &AppSettings) -> Result<(), String> {
    let path = settings_path()?;
    let tmp = path.with_extension("json.tmp");
    let json = serde_json::to_string_pretty(settings)
        .map_err(|e| format!("Failed to serialize settings: {e}"))?;
    std::fs::write(&tmp, json)
        .map_err(|e| format!("Failed to write settings: {e}"))?;
    std::fs::rename(&tmp, &path)
        .map_err(|e| format!("Failed to finalize settings: {e}"))?;
    // Same belt-and-suspenders as `Macro::save` - avoids a stale read if a
    // save and reload land within the same mtime tick.
    *settings_cache().lock().unwrap() = None;
    Ok(())
}

pub fn update_settings<F: FnOnce(&mut AppSettings)>(f: F) {
    let mut settings = load_settings();
    f(&mut settings);
    if let Err(e) = save_settings(&settings) {
        warn!("Failed to save settings: {e}");
    }
}
