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
