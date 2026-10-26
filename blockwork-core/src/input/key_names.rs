use crate::input::types::MacroKey;

pub fn string_to_key(key_str: &str) -> Result<MacroKey, &'static str> {
    match key_str {
        "F1" => Ok(MacroKey::F1),
        "F2" => Ok(MacroKey::F2),
        "F3" => Ok(MacroKey::F3),
        "F4" => Ok(MacroKey::F4),
        "F5" => Ok(MacroKey::F5),
        "F6" => Ok(MacroKey::F6),
        "F7" => Ok(MacroKey::F7),
        "F8" => Ok(MacroKey::F8),
        "F9" => Ok(MacroKey::F9),
        "F10" => Ok(MacroKey::F10),
        "F11" => Ok(MacroKey::F11),
        "F12" => Ok(MacroKey::F12),
        "F13" => Ok(MacroKey::F13),
        "F14" => Ok(MacroKey::F14),
        "F15" => Ok(MacroKey::F15),
        "F16" => Ok(MacroKey::F16),
        "F17" => Ok(MacroKey::F17),
        "F18" => Ok(MacroKey::F18),
        "F19" => Ok(MacroKey::F19),
        "F20" => Ok(MacroKey::F20),
        "F21" => Ok(MacroKey::F21),
        "F22" => Ok(MacroKey::F22),
        "F23" => Ok(MacroKey::F23),
        "F24" => Ok(MacroKey::F24),
        "Escape" => Ok(MacroKey::Escape),
        "Space" => Ok(MacroKey::Space),
        "Enter" => Ok(MacroKey::Return),
        "Tab" => Ok(MacroKey::Tab),
        "Backspace" => Ok(MacroKey::Backspace),
        "CapsLock" => Ok(MacroKey::CapsLock),
        "Shift" | "LeftShift" => Ok(MacroKey::LShift),
        "Control" => Ok(MacroKey::Control),
        "Alt" => Ok(MacroKey::Alt),
        "Meta" | "Super" => Ok(MacroKey::Meta),
        "LeftArrow" => Ok(MacroKey::LeftArrow),
        "RightArrow" => Ok(MacroKey::RightArrow),
        "UpArrow" => Ok(MacroKey::UpArrow),
        "DownArrow" => Ok(MacroKey::DownArrow),
        "Insert" => Ok(MacroKey::Insert),
        "Delete" => Ok(MacroKey::Delete),
        "Home" => Ok(MacroKey::Home),
        "End" => Ok(MacroKey::End),
        "PageUp" => Ok(MacroKey::PageUp),
        "PageDown" => Ok(MacroKey::PageDown),
        "NumLock" | "Numlock" => Ok(MacroKey::NumLock),
        "ScrollLock" => Ok(MacroKey::ScrollLock),
        "Pause" => Ok(MacroKey::Pause),
        "PrintScr" | "PrintScreen" => Ok(MacroKey::PrintScr),
        "LMenu" => Ok(MacroKey::LMenu),
        "RightShift" => Ok(MacroKey::RShift),
        "LeftControl" => Ok(MacroKey::LControl),
        "RightControl" => Ok(MacroKey::RControl),
        "LeftAlt" | "Option" => Ok(MacroKey::Option),
        "VolumeDown" => Ok(MacroKey::VolumeDown),
        "VolumeMute" => Ok(MacroKey::VolumeMute),
        "VolumeUp" => Ok(MacroKey::VolumeUp),
        "Select" => Ok(MacroKey::Select),
        _ => {
            if key_str.len() == 1 {
                Ok(MacroKey::Unicode(key_str.chars().next().unwrap()))
            } else {
                Err("Unknown key string")
            }
        }
    }
}

pub fn key_to_string(key: &MacroKey) -> Result<&'static str, &'static str> {
    match key {
        MacroKey::F1 => Ok("F1"),
        MacroKey::F2 => Ok("F2"),
        MacroKey::F3 => Ok("F3"),
        MacroKey::F4 => Ok("F4"),
        MacroKey::F5 => Ok("F5"),
        MacroKey::F6 => Ok("F6"),
        MacroKey::F7 => Ok("F7"),
        MacroKey::F8 => Ok("F8"),
        MacroKey::F9 => Ok("F9"),
        MacroKey::F10 => Ok("F10"),
        MacroKey::F11 => Ok("F11"),
        MacroKey::F12 => Ok("F12"),
        MacroKey::F13 => Ok("F13"),
        MacroKey::F14 => Ok("F14"),
        MacroKey::F15 => Ok("F15"),
        MacroKey::F16 => Ok("F16"),
        MacroKey::F17 => Ok("F17"),
        MacroKey::F18 => Ok("F18"),
        MacroKey::F19 => Ok("F19"),
        MacroKey::F20 => Ok("F20"),
        MacroKey::F21 => Ok("F21"),
        MacroKey::F22 => Ok("F22"),
        MacroKey::F23 => Ok("F23"),
        MacroKey::F24 => Ok("F24"),
        MacroKey::Escape => Ok("Escape"),
        MacroKey::Space => Ok("Space"),
        MacroKey::Return => Ok("Enter"),
        MacroKey::Tab => Ok("Tab"),
        MacroKey::Backspace => Ok("Backspace"),
        MacroKey::CapsLock => Ok("CapsLock"),
        MacroKey::Shift => Ok("LeftShift"),
        MacroKey::LShift => Ok("LeftShift"),
        MacroKey::RShift => Ok("RightShift"),
        MacroKey::Control => Ok("Control"),
        MacroKey::LControl => Ok("LeftControl"),
        MacroKey::RControl => Ok("RightControl"),
        MacroKey::Alt => Ok("Alt"),
        MacroKey::AltGr => Ok("AltGr"),
        MacroKey::Option => Ok("LeftAlt"),
        MacroKey::Meta => Ok("Meta"),
        MacroKey::LeftArrow => Ok("LeftArrow"),
        MacroKey::RightArrow => Ok("RightArrow"),
        MacroKey::UpArrow => Ok("UpArrow"),
        MacroKey::DownArrow => Ok("DownArrow"),
        MacroKey::Insert => Ok("Insert"),
        MacroKey::Delete => Ok("Delete"),
        MacroKey::Home => Ok("Home"),
        MacroKey::End => Ok("End"),
        MacroKey::PageUp => Ok("PageUp"),
        MacroKey::PageDown => Ok("PageDown"),
        MacroKey::NumLock => Ok("NumLock"),
        MacroKey::ScrollLock => Ok("ScrollLock"),
        MacroKey::Pause => Ok("Pause"),
        MacroKey::PrintScr => Ok("PrintScr"),
        MacroKey::LMenu => Ok("LMenu"),
        MacroKey::VolumeDown => Ok("VolumeDown"),
        MacroKey::VolumeMute => Ok("VolumeMute"),
        MacroKey::VolumeUp => Ok("VolumeUp"),
        MacroKey::Select => Ok("Select"),
        MacroKey::Unicode(c) => {
            // Leak a one-character string for display purposes.
            let s = c.to_string();
            Ok(Box::leak(s.into_boxed_str()))
        }
        _ => Err("Unknown key"),
    }
}

/// Display/storage name for any key, including ones `key_to_string` can't name.
pub fn key_name(key: &MacroKey) -> String {
    match key {
        MacroKey::Other(n) => format!("Other:{n}"),
        _ => key_to_string(key).unwrap_or("Unknown").to_string(),
    }
}

/// Parses one combo part, accepting everything `key_name` produces.
pub fn parse_key_name(name: &str) -> Option<MacroKey> {
    if let Some(n) = name.strip_prefix("Other:") {
        return n.parse().ok().map(MacroKey::Other);
    }
    string_to_key(name.trim()).ok()
}

/// Joins keys into a combo string such as `Control+Alt+O`.
pub fn format_key_combo(keys: &[MacroKey]) -> String {
    keys.iter().map(key_name).collect::<Vec<_>>().join("+")
}

/// Splits a combo string on `+`. A `+` with nothing before it is the key
/// itself, so `Control++` is Control and the plus key. Unknown parts are dropped.
pub fn parse_key_combo(text: &str) -> Vec<MacroKey> {
    let mut parts: Vec<String> = Vec::new();
    let mut current = String::new();
    for c in text.chars() {
        if c == '+' && !current.is_empty() {
            parts.push(std::mem::take(&mut current));
        } else {
            current.push(c);
        }
    }
    if !current.is_empty() {
        parts.push(current);
    }
    parts.iter().filter_map(|p| parse_key_name(p)).collect()
}

/// Folds left/right and case variants together so a physical key event
/// compares equal to a key named in a combo.
pub fn match_key(key: &MacroKey) -> MacroKey {
    match key {
        MacroKey::LShift | MacroKey::RShift => MacroKey::Shift,
        MacroKey::LControl | MacroKey::RControl => MacroKey::Control,
        MacroKey::AltGr | MacroKey::Option | MacroKey::LMenu => MacroKey::Alt,
        #[cfg(target_os = "macos")]
        MacroKey::PrintScr => MacroKey::F13,
        #[cfg(target_os = "macos")]
        MacroKey::ScrollLock => MacroKey::F14,
        #[cfg(target_os = "macos")]
        MacroKey::Pause => MacroKey::F15,
        MacroKey::Unicode(c) => MacroKey::Unicode(c.to_ascii_lowercase()),
        other => other.clone(),
    }
}

#[cfg(test)]
mod combo_tests {
    use super::*;

    #[test]
    fn combo_round_trips() {
        let keys = parse_key_combo("Control+Alt+o+s");
        assert_eq!(keys, vec![MacroKey::Control, MacroKey::Alt, MacroKey::Unicode('o'), MacroKey::Unicode('s')]);
        assert_eq!(format_key_combo(&keys), "Control+Alt+o+s");
    }

    #[test]
    fn menu_alias_matches_the_alt_modifier_emitted_by_backends() {
        let keys = parse_key_combo("LMenu+a");
        assert_eq!(match_key(&keys[0]), MacroKey::Alt);
        assert_eq!(keys[0].modifier_bit(), crate::hotkey_types::MOD_ALT);
        assert!(keys[0].is_modifier());
        assert_ne!(match_key(&keys[0]), match_key(&MacroKey::Meta));
    }

    #[test]
    fn extended_function_keys_keep_hotkey_and_stop_recording_names() {
        for number in 13..=24 {
            let name = format!("F{number}");
            let key = parse_key_name(&name).unwrap();
            assert_eq!(key.hotkey_name().as_deref(), Some(name.as_str()));
        }
    }

    #[test]
    fn pc_key_triggers_use_native_aliases_on_mac() {
        for (pc, native) in [(MacroKey::PrintScr, MacroKey::F13), (MacroKey::ScrollLock, MacroKey::F14), (MacroKey::Pause, MacroKey::F15)] {
            assert_eq!(match_key(&pc) == match_key(&native), cfg!(target_os = "macos"));
        }
        assert_eq!(parse_key_name("PrintScreen"), Some(MacroKey::PrintScr));
    }

    #[test]
    fn plus_key_is_literal() {
        assert_eq!(parse_key_combo("+"), vec![MacroKey::Unicode('+')]);
        assert_eq!(parse_key_combo("Control++"), vec![MacroKey::Control, MacroKey::Unicode('+')]);
    }
}
