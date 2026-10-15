pub mod battery;
pub mod clipboard;
pub mod config;
pub mod detect;
pub mod files;
pub mod globals;
pub mod flatpak;
pub mod hotkey_types;
pub mod http;
pub mod input;
#[cfg(feature = "ipc")]
pub mod ipc;
pub mod key_mapping;
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
pub mod text;
#[cfg(all(feature = "updater", any(windows, target_os = "macos")))]
pub mod updater;
pub mod window;
pub mod wire;

/// Registers Blockwork's own value operators with blockstitch, so a saved
/// macro using them evaluates. Every entry point calls it; twice is fine.
pub fn init() {
    input::value::register_blockwork_operators();
}
