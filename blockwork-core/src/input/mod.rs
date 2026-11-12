//! The model's input types and values (`blockwork_model::input`), plus what
//! reads or replays input on this machine.

pub use blockwork_model::input::*;

pub mod controllers;
pub mod cron;
pub mod instruction_utils;
#[cfg(target_os = "linux")]
pub mod native_gamepad;
#[cfg(any(target_os = "linux", windows))]
pub mod native_touch;
pub mod touch_playback;
pub mod typed;
pub mod ui_utils;
pub mod value;
pub mod virtual_gamepad;

/// Name of the virtual touchscreen touch playback creates (Linux).
pub const VIRTUAL_TOUCH_NAME: &str = "Blockwork Virtual Touch";
/// Name of the virtual controller controller blocks drive (Linux).
pub const VIRTUAL_GAMEPAD_NAME: &str = "Blockwork Virtual Gamepad";

/// True for a device Blockwork made itself. Readers skip these so playback
/// is never read back as input.
pub fn is_own_device(name: &str) -> bool {
    name.starts_with("Blockwork Virtual ")
}

#[cfg(test)]
mod tests {
    #[test]
    fn readers_skip_every_virtual_device() {
        assert!(super::is_own_device(super::VIRTUAL_TOUCH_NAME));
        assert!(super::is_own_device(super::VIRTUAL_GAMEPAD_NAME));
        assert!(!super::is_own_device("Wacom Touchscreen"));
    }
}
