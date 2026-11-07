use crate::input::types::MacroKey;

/// Maps a Web KeyboardEvent.code string to a MacroKey for named/special keys.
/// Most JS `code` names match our stored rdev names directly; this handles
/// the cases that don't.
pub fn web_code_to_macro_key(code: &str) -> Option<MacroKey> {
    use crate::input::key_names::string_to_key;
    // First try the rdev name lookup (handles KeyA, F1, Space, etc.)
    if let Ok(k) = string_to_key(code) {
        return Some(k);
    }
    // Handle JS code aliases that differ from rdev names
    let rdev_name = web_code_to_rdev_name(code)?;
    string_to_key(&rdev_name).ok()
}

/// Maps a Web KeyboardEvent.key string to a MacroKey for printable characters.
pub fn web_key_to_macro_key(key: &str) -> Option<MacroKey> {
    let mut chars = key.chars();
    let c = chars.next()?;
    if chars.next().is_none() {
        Some(MacroKey::Unicode(c))
    } else {
        None
    }
}

/// Maps a Web KeyboardEvent.code to the rdev key name string used in KeyCombo.key.
/// This is the translation of iced_code_to_rdev_key_name for web KeyboardEvent.code values.
pub fn web_code_to_rdev_name(code: &str) -> Option<String> {
    let name = match code {
        "KeyA" => "KeyA",
        "KeyB" => "KeyB",
        "KeyC" => "KeyC",
        "KeyD" => "KeyD",
        "KeyE" => "KeyE",
        "KeyF" => "KeyF",
        "KeyG" => "KeyG",
        "KeyH" => "KeyH",
        "KeyI" => "KeyI",
        "KeyJ" => "KeyJ",
        "KeyK" => "KeyK",
        "KeyL" => "KeyL",
        "KeyM" => "KeyM",
        "KeyN" => "KeyN",
        "KeyO" => "KeyO",
        "KeyP" => "KeyP",
        "KeyQ" => "KeyQ",
        "KeyR" => "KeyR",
        "KeyS" => "KeyS",
        "KeyT" => "KeyT",
        "KeyU" => "KeyU",
        "KeyV" => "KeyV",
        "KeyW" => "KeyW",
        "KeyX" => "KeyX",
        "KeyY" => "KeyY",
        "KeyZ" => "KeyZ",
        "Digit0" => "Num0",
        "Digit1" => "Num1",
        "Digit2" => "Num2",
        "Digit3" => "Num3",
        "Digit4" => "Num4",
        "Digit5" => "Num5",
        "Digit6" => "Num6",
        "Digit7" => "Num7",
        "Digit8" => "Num8",
        "Digit9" => "Num9",
        "F1" => "F1",
        "F2" => "F2",
        "F3" => "F3",
        "F4" => "F4",
        "F5" => "F5",
        "F6" => "F6",
        "F7" => "F7",
        "F8" => "F8",
        "F9" => "F9",
        "F10" => "F10",
        "F11" => "F11",
        "F12" => "F12",
        "F13" => "F13",
        "F14" => "F14",
        "F15" => "F15",
        "F16" => "F16",
        "F17" => "F17",
        "F18" => "F18",
        "F19" => "F19",
        "F20" => "F20",
        "ArrowLeft" => "LeftArrow",
        "ArrowRight" => "RightArrow",
        "ArrowUp" => "UpArrow",
        "ArrowDown" => "DownArrow",
        "Space" => "Space",
        "Enter" => "Return",
        "Tab" => "Tab",
        "Escape" => "Escape",
        "Backspace" => "Backspace",
        "Insert" => "Insert",
        "Delete" => "Delete",
        "Home" => "Home",
        "End" => "End",
        "PageUp" => "PageUp",
        "PageDown" => "PageDown",
        "CapsLock" => "CapsLock",
        "NumLock" => "NumLock",
        "ScrollLock" => "ScrollLock",
        "PrintScreen" => "PrintScreen",
        "Pause" => "Pause",
        "Quote" => "Quote",
        "Semicolon" => "SemiColon",
        "Comma" => "Comma",
        "Period" => "Dot",
        "Slash" => "Slash",
        "Backquote" => "BackQuote",
        "BracketLeft" => "LeftBracket",
        "BracketRight" => "RightBracket",
        "Minus" => "Minus",
        "Equal" => "Equal",
        "Backslash" => "BackSlash",
        "IntlBackslash" => "IntlBackslash",
        "ShiftLeft" => "LeftShift",
        "ShiftRight" => "RightShift",
        "ControlLeft" => "LeftControl",
        "ControlRight" => "RightControl",
        "AltLeft" => "Alt",
        "AltRight" => "AltGr",
        "MetaLeft" | "MetaRight" => "MetaLeft",
        _ => return None,
    };
    Some(name.to_string())
}

pub fn is_modifier_code(code: &str) -> bool {
    matches!(
        code,
        "ShiftLeft"
            | "ShiftRight"
            | "ControlLeft"
            | "ControlRight"
            | "AltLeft"
            | "AltRight"
            | "MetaLeft"
            | "MetaRight"
            | "CapsLock"
    )
}
