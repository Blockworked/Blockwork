pub mod persistence;
pub mod selection;
pub mod settings;

pub use persistence::{get_macros_from_config, read_macro_by_id, read_macro_file, write_macro_file};
pub use selection::{get_macro_by_id, get_selected_macro_id, set_selected_macro_id};
pub use settings::{load_settings, save_settings, update_settings, AppSettings};

pub const APP_ID: &str = "Blockwork";
/// The app's previous identifier, back when it was named "Macros" — the OS
/// config directory this crate reads/writes moved from `<config_root>/Macros`
/// to `<config_root>/Blockwork` (see `APP_ID`) when the app was renamed.
/// Kept only for `migrate_legacy_app_id`.
const LEGACY_APP_ID: &str = "Macros";
pub const GLOBAL_HOTKEYS_KEY: &str = "global_hotkeys";

use crate::hotkey_types::{HotkeyAction, HotkeyBinding, KeyCombo, MOD_ALT, MOD_CTRL};
use crate::macros::Macro;
use std::path::PathBuf;
use std::sync::OnceLock;

static CONFIG_DIR_OVERRIDE: OnceLock<PathBuf> = OnceLock::new();

/// Redirects every `config::*`/`Macro::save`/`Macro::remove` file lookup to
/// `dir/Blockwork` instead of the OS config directory — for an embedder
/// (e.g. a host process loading blockwork-ffi) whose process sees a
/// different filesystem namespace than wherever the macro files actually
/// live. First caller wins; must be called, if at all, before any other
/// `config::*` or `recording::*` function.
pub fn set_config_dir_override(dir: PathBuf) {
    let _ = CONFIG_DIR_OVERRIDE.set(dir);
}

pub(crate) fn config_root() -> Result<PathBuf, String> {
    if let Some(dir) = CONFIG_DIR_OVERRIDE.get() {
        return Ok(dir.join(APP_ID));
    }
    dirs::config_dir()
        .map(|d| d.join(APP_ID))
        .ok_or_else(|| "Unable to resolve config directory".to_string())
}

/// One-time migration for the Macros→Blockwork rename: if the new config
/// directory (`APP_ID`) doesn't exist yet but the old one (`LEGACY_APP_ID`)
/// does, moves it wholesale so every saved macro/setting/hotkey survives.
/// A no-op after the first launch, and skipped under `CONFIG_DIR_OVERRIDE`
/// (the embedder path was never on the OS config directory). Must run
/// before any other `config::*`/`recording::*` call.
pub fn migrate_legacy_app_id() {
    if CONFIG_DIR_OVERRIDE.get().is_some() {
        return;
    }
    let Some(base) = dirs::config_dir() else { return };
    let new_root = base.join(APP_ID);
    let old_root = base.join(LEGACY_APP_ID);
    if new_root.exists() || !old_root.exists() {
        return;
    }
    if let Err(err) = std::fs::rename(&old_root, &new_root) {
        tracing::warn!(
            "failed to migrate config dir from '{}' to '{}': {err}",
            old_root.display(),
            new_root.display(),
        );
    } else {
        tracing::info!("migrated config dir '{}' -> '{}'", old_root.display(), new_root.display());
    }
}

impl Macro {
    pub fn add(mut self) -> Result<(), String> {
        self.ensure_id();
        self.save()
    }
}

pub fn load_hotkey_bindings() -> Vec<HotkeyBinding> {
    load_settings()
        .global_hotkeys
        .unwrap_or_else(default_hotkey_bindings)
}

pub fn save_hotkey_bindings(bindings: &[HotkeyBinding]) {
    update_settings(|s| s.global_hotkeys = Some(bindings.to_vec()));
}

pub fn default_combo_for_action(action: &HotkeyAction) -> Option<KeyCombo> {
    default_hotkey_bindings()
        .into_iter()
        .find(|b| &b.action == action)
        .map(|b| b.combo)
}

fn default_hotkey_bindings() -> Vec<HotkeyBinding> {
    vec![
        HotkeyBinding {
            action: HotkeyAction::RunMacro,
            combo: KeyCombo { modifiers: MOD_CTRL | MOD_ALT, key: "KeyM".to_string() },
        },
        HotkeyBinding {
            action: HotkeyAction::StopLoop,
            combo: KeyCombo { modifiers: MOD_CTRL | MOD_ALT, key: "KeyS".to_string() },
        },
        HotkeyBinding {
            action: HotkeyAction::NextMacro,
            combo: KeyCombo { modifiers: MOD_CTRL | MOD_ALT, key: "RightArrow".to_string() },
        },
        HotkeyBinding {
            action: HotkeyAction::PrevMacro,
            combo: KeyCombo { modifiers: MOD_CTRL | MOD_ALT, key: "LeftArrow".to_string() },
        },
        HotkeyBinding {
            action: HotkeyAction::ToggleLoop,
            combo: KeyCombo { modifiers: MOD_CTRL | MOD_ALT, key: "KeyL".to_string() },
        },
        HotkeyBinding {
            action: HotkeyAction::StopRecording,
            combo: KeyCombo { modifiers: 0, key: "Escape".to_string() },
        },
    ]
}
