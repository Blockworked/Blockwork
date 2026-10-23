#[cfg(feature = "system")]
pub mod backoff;
#[cfg(feature = "system")]
pub mod battery;
pub mod block_packs;
#[cfg(feature = "system")]
pub mod clipboard;
#[cfg(feature = "system")]
pub mod config;
#[cfg(feature = "system")]
pub mod detect;
#[cfg(feature = "system")]
pub mod files;
#[cfg(feature = "system")]
pub mod globals;
#[cfg(feature = "system")]
pub mod flatpak;
pub mod frontend;
pub mod hotkey_types;
#[cfg(feature = "system")]
pub mod http;
pub mod input;
#[cfg(feature = "ipc")]
pub mod ipc;
#[cfg(feature = "system")]
pub mod key_mapping;
#[cfg(all(feature = "system", target_os = "linux"))]
pub mod dbus;
#[cfg(all(feature = "system", target_os = "linux"))]
pub mod kwin;
pub mod macros;
#[cfg(feature = "system")]
pub mod notify;
#[cfg(feature = "system")]
pub mod permissions;
#[cfg(feature = "system")]
pub mod prompt;
#[cfg(feature = "system")]
pub mod recording;
#[cfg(feature = "system")]
pub mod run_log;
#[cfg(feature = "system")]
pub mod screen;
#[cfg(feature = "system")]
pub mod secrets;
#[cfg(feature = "system")]
pub mod shell;
#[cfg(feature = "system")]
pub mod system;
#[cfg(feature = "system")]
pub mod text;
#[cfg(all(feature = "updater", any(windows, target_os = "macos")))]
pub mod updater;
#[cfg(feature = "system")]
pub mod window;
#[cfg(feature = "system")]
pub mod wire;

/// Registers Blockwork's own value operators with blockstitch, so a saved
/// macro using them evaluates. Every entry point calls it; twice is fine.
pub fn init() {
    #[cfg(feature = "system")]
    input::value::register_blockwork_operators();
}
