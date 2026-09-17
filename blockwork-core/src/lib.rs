pub mod battery;
pub mod config;
pub mod flatpak;
pub mod hotkey_types;
pub mod input;
#[cfg(feature = "ipc")]
pub mod ipc;
pub mod key_mapping;
pub mod macros;
pub mod recording;
#[cfg(all(feature = "updater", any(windows, target_os = "macos")))]
pub mod updater;
pub mod wire;
