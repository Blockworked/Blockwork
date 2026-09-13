pub mod accessibility;
pub mod battery;
pub mod clipboard;
pub mod config;
pub mod hotkey_types;
pub mod input;
#[cfg(feature = "ipc")]
pub mod ipc;
pub mod key_mapping;
pub mod macros;
pub mod recording;
#[cfg(all(windows, feature = "updater"))]
pub mod updater;
pub mod window;
pub mod wire;
