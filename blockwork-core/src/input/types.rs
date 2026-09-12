use serde::{Deserialize, Serialize};

use crate::hotkey_types::{MOD_ALT, MOD_CTRL, MOD_META, MOD_SHIFT};
use crate::input::value::Value;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Direction {
    Press,
    Release,
    Click,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Coordinate {
    Abs,
    Rel,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Axis {
    Vertical,
    Horizontal,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MacroKey {
    Return,
    Backspace,
    Tab,
    Space,
    Escape,
    Delete,
    Insert,
    Home,
    End,
    PageUp,
    PageDown,
    UpArrow,
    DownArrow,
    LeftArrow,
    RightArrow,
    Shift,
    LShift,
    RShift,
    Control,
    LControl,
    RControl,
    Alt,
    AltGr,
    Meta,
    Option,
    CapsLock,
    NumLock,
    ScrollLock,
    Pause,
    PrintScr,
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
    F13,
    F14,
    F15,
    F16,
    F17,
    F18,
    F19,
    F20,
    F21,
    F22,
    F23,
    F24,
    Numpad0,
    Numpad1,
    Numpad2,
    Numpad3,
    Numpad4,
    Numpad5,
    Numpad6,
    Numpad7,
    Numpad8,
    Numpad9,
    Add,
    Subtract,
    Multiply,
    Divide,
    Decimal,
    VolumeDown,
    VolumeMute,
    VolumeUp,
    LMenu,
    Select,
    Unicode(char),
    Other(u32),
}

impl MacroKey {
    pub fn modifier_bit(&self) -> u8 {
        match self {
            MacroKey::Control | MacroKey::LControl | MacroKey::RControl => MOD_CTRL,
            MacroKey::Shift | MacroKey::LShift | MacroKey::RShift => MOD_SHIFT,
            MacroKey::Alt | MacroKey::AltGr | MacroKey::Option => MOD_ALT,
            MacroKey::Meta => MOD_META,
            _ => 0,
        }
    }

    pub fn is_modifier(&self) -> bool {
        self.modifier_bit() != 0
    }

    /// Returns the rdev-compatible debug string used for hotkey storage/matching.
    pub fn hotkey_name(&self) -> Option<String> {
        Some(match self {
            MacroKey::Return => "Return".into(),
            MacroKey::Backspace => "Backspace".into(),
            MacroKey::Tab => "Tab".into(),
            MacroKey::Space => "Space".into(),
            MacroKey::Escape => "Escape".into(),
            MacroKey::Delete => "Delete".into(),
            MacroKey::Insert => "Insert".into(),
            MacroKey::Home => "Home".into(),
            MacroKey::End => "End".into(),
            MacroKey::PageUp => "PageUp".into(),
            MacroKey::PageDown => "PageDown".into(),
            MacroKey::UpArrow => "UpArrow".into(),
            MacroKey::DownArrow => "DownArrow".into(),
            MacroKey::LeftArrow => "LeftArrow".into(),
            MacroKey::RightArrow => "RightArrow".into(),
            MacroKey::F1 => "F1".into(),
            MacroKey::F2 => "F2".into(),
            MacroKey::F3 => "F3".into(),
            MacroKey::F4 => "F4".into(),
            MacroKey::F5 => "F5".into(),
            MacroKey::F6 => "F6".into(),
            MacroKey::F7 => "F7".into(),
            MacroKey::F8 => "F8".into(),
            MacroKey::F9 => "F9".into(),
            MacroKey::F10 => "F10".into(),
            MacroKey::F11 => "F11".into(),
            MacroKey::F12 => "F12".into(),
            MacroKey::CapsLock => "CapsLock".into(),
            MacroKey::NumLock => "NumLock".into(),
            MacroKey::ScrollLock => "ScrollLock".into(),
            MacroKey::Pause => "Pause".into(),
            MacroKey::PrintScr => "PrintScreen".into(),
            MacroKey::Unicode(c) => match c {
                'a'..='z' => format!("Key{}", c.to_ascii_uppercase()),
                '0'..='9' => format!("Num{}", c),
                '-' => "Minus".into(),
                '=' => "Equal".into(),
                '[' => "LeftBracket".into(),
                ']' => "RightBracket".into(),
                '\\' => "BackSlash".into(),
                ';' => "SemiColon".into(),
                '\'' => "Quote".into(),
                '`' => "BackQuote".into(),
                ',' => "Comma".into(),
                '.' => "Dot".into(),
                '/' => "Slash".into(),
                _ => return None,
            },
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MacroButton {
    Left,
    Right,
    Middle,
    Back,
    Forward,
    ScrollUp,
    ScrollDown,
    ScrollLeft,
    ScrollRight,
    Other(u8),
}

#[derive(Debug, Clone, PartialEq, Hash, Serialize, Deserialize)]
pub enum InputToken {
    Key(MacroKey, Direction),
    Button(MacroButton, Direction),
    MoveMouse(Value, Value, Coordinate),
    Scroll(Value, Axis),
    Text(Value),
    Raw(u16, Direction),
}

impl InputToken {
    /// Every `Value` this token owns, for `InstructionKind`'s
    /// `BlockKind::visit_values_mut`. None of them are boolean-typed.
    pub fn visit_values_mut(
        &mut self,
        f: &mut dyn FnMut(&mut Value, blockstitch_core::graph::InputValueType),
    ) {
        use blockstitch_core::graph::InputValueType::Any;
        match self {
            InputToken::MoveMouse(x, y, _) => {
                f(x, Any);
                f(y, Any);
            }
            InputToken::Scroll(amount, _) => f(amount, Any),
            InputToken::Text(value) => f(value, Any),
            InputToken::Key(..) | InputToken::Button(..) | InputToken::Raw(..) => {}
        }
    }

    /// Renames a list inside every embedded `Value` tree - see
    /// `crate::macros::rename_list_in_value`.
    pub fn rename_list(&mut self, old: &str, new: &str) {
        self.visit_values_mut(&mut |v, _| crate::macros::rename_list_in_value(v, old, new));
    }
}
