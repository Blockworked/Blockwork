use serde::{Deserialize, Serialize};

pub const MOD_CTRL: u8 = 1;
pub const MOD_SHIFT: u8 = 2;
pub const MOD_ALT: u8 = 4;
pub const MOD_META: u8 = 8;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct KeyCombo {
    pub modifiers: u8,
    pub key: String,
}

impl KeyCombo {
    pub fn format(&self) -> String {
        let mut parts: Vec<String> = vec![];
        if self.modifiers & MOD_CTRL != 0 {
            parts.push("Ctrl".to_string());
        }
        if self.modifiers & MOD_SHIFT != 0 {
            parts.push("Shift".to_string());
        }
        if self.modifiers & MOD_ALT != 0 {
            parts.push("Alt".to_string());
        }
        if self.modifiers & MOD_META != 0 {
            parts.push("Meta".to_string());
        }
        parts.push(rdev_key_name_to_display(&self.key));
        parts.join("+")
    }
}

fn rdev_key_name_to_display(name: &str) -> String {
    if let Some(letter) = name.strip_prefix("Key") {
        if letter.len() == 1 {
            return letter.to_uppercase();
        }
    }
    if let Some(digit) = name.strip_prefix("Num") {
        return digit.to_string();
    }
    match name {
        "LeftArrow" => "Left",
        "RightArrow" => "Right",
        "UpArrow" => "Up",
        "DownArrow" => "Down",
        "Return" => "Enter",
        "BackSlash" => "\\",
        "BackQuote" => "`",
        "SemiColon" => ";",
        "Quote" => "'",
        "LeftBracket" => "[",
        "RightBracket" => "]",
        other => other,
    }
    .to_string()
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum HotkeyAction {
    RunMacro,
    StopLoop,
    /// Kill switch: stops every run and pauses triggers until resumed.
    EmergencyStop,
    NextMacro,
    PrevMacro,
    RunSpecificMacro(String),
    StartRecordingImmediate,
    StopRecording,
    Undo,
    Redo,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HotkeyBinding {
    pub action: HotkeyAction,
    pub combo: KeyCombo,
}

/// Compare stored hotkey names using the physical keyboard translation of the
/// current OS. Mac PC keyboards report PrintScreen/ScrollLock/Pause as F13–F15.
pub(crate) fn key_names_match(configured: &str, captured: &str) -> bool {
    #[cfg(target_os = "macos")]
    fn canonical(name: &str) -> &str {
        match name {
            "PrintScreen" | "PrintScr" => "F13",
            "ScrollLock" => "F14",
            "Pause" => "F15",
            other => other,
        }
    }
    #[cfg(not(target_os = "macos"))]
    fn canonical(name: &str) -> &str { name }
    canonical(configured) == canonical(captured)
}

#[cfg(test)]
mod keyboard_alias_tests {
    use super::key_names_match;

    #[test]
    fn pc_key_hotkeys_follow_the_platform_keyboard_translation() {
        for (pc, native) in [("PrintScreen", "F13"), ("ScrollLock", "F14"), ("Pause", "F15")] {
            assert!(key_names_match(pc, pc));
            assert_eq!(key_names_match(pc, native), cfg!(target_os = "macos"));
            assert_eq!(key_names_match(native, pc), cfg!(target_os = "macos"));
        }
        assert!(!key_names_match("F13", "F14"));
        assert!(!key_names_match("F20", "F21"));
    }
}
