//! Blockwork on this machine: running macros, their input backends,
//! watchers, config, files, network, clipboard and the rest. The blocks and
//! documents themselves are `blockwork_model`, re-exported here under the
//! same paths.

pub use blockwork_model::{
    block_packs, capabilities, editing, frontend, hotkey_types, key_mapping, redact, text,
};

pub mod backoff;
pub mod battery;
pub mod browser;
pub mod clipboard;
pub mod config;
#[cfg(target_os = "linux")]
pub mod dbus;
pub mod detect;
pub mod files;
pub mod flatpak;
pub mod globals;
pub mod http;
pub mod input;
#[cfg(feature = "ipc")]
pub mod ipc;
#[cfg(target_os = "linux")]
pub mod kwin;
pub mod macros;
pub mod notify;
pub mod permissions;
pub mod prompt;
pub mod recording;
pub mod run_log;
pub mod screen;
pub mod secrets;
pub mod shell;
pub mod system;
#[cfg(all(feature = "updater", any(windows, target_os = "macos")))]
pub mod updater;
pub mod window;
pub mod wine_bridge;

/// Registers Blockwork's own value operators with blockstitch, so a saved
/// macro using them evaluates. Every entry point calls it; twice is fine.
pub fn init() {
    input::value::register_blockwork_operators();
    redact::set_masker(secrets::redact);
}
