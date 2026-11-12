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
    /// Legacy name for the left Alt modifier.
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
            MacroKey::Alt | MacroKey::AltGr | MacroKey::Option | MacroKey::LMenu => MOD_ALT,
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
            MacroKey::F13 => "F13".into(),
            MacroKey::F14 => "F14".into(),
            MacroKey::F15 => "F15".into(),
            MacroKey::F16 => "F16".into(),
            MacroKey::F17 => "F17".into(),
            MacroKey::F18 => "F18".into(),
            MacroKey::F19 => "F19".into(),
            MacroKey::F20 => "F20".into(),
            MacroKey::F21 => "F21".into(),
            MacroKey::F22 => "F22".into(),
            MacroKey::F23 => "F23".into(),
            MacroKey::F24 => "F24".into(),
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

/// How a text block paces its typing: a delay for each character, or a
/// total time for the whole string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TextSpeedMode {
    PerCharacter,
    Total,
}

/// The text block's optional typing pace. `value` is in milliseconds.
#[derive(Debug, Clone, PartialEq, Hash, Serialize, Deserialize)]
pub struct TextSpeed {
    pub mode: TextSpeedMode,
    pub value: Value,
}

impl TextSpeed {
    /// Milliseconds between characters when typing `chars` characters.
    pub fn per_character_ms(&self, ms: f64, chars: usize) -> f64 {
        match self.mode {
            TextSpeedMode::PerCharacter => ms,
            TextSpeedMode::Total if chars == 0 => 0.0,
            TextSpeedMode::Total => ms / chars as f64,
        }
    }
}

/// A text block's payload: the text, and optionally how fast to type it.
#[derive(Debug, Clone, PartialEq, Hash, Serialize, Deserialize)]
#[serde(from = "TextTokenWire", into = "TextTokenWire")]
pub struct TextToken {
    pub text: Value,
    pub speed: Option<TextSpeed>,
}

impl TextToken {
    pub fn new(text: Value) -> Self {
        Self { text, speed: None }
    }
}

impl From<Value> for TextToken {
    fn from(text: Value) -> Self {
        Self::new(text)
    }
}

// Saves from before typing speeds stored just the text value.
#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum TextTokenWire {
    Legacy(Value),
    Current {
        text: Value,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        speed: Option<TextSpeed>,
    },
}

impl From<TextTokenWire> for TextToken {
    fn from(wire: TextTokenWire) -> Self {
        match wire {
            TextTokenWire::Legacy(text) => Self::new(text),
            TextTokenWire::Current { text, speed } => Self { text, speed },
        }
    }
}

impl From<TextToken> for TextTokenWire {
    fn from(token: TextToken) -> Self {
        match token.speed {
            Some(speed) => TextTokenWire::Current {
                text: token.text,
                speed: Some(speed),
            },
            None => TextTokenWire::Legacy(token.text),
        }
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
    /// The key slot holds a combo string such as `Control+Alt+O`, so a
    /// variable or operator can supply it.
    Key(
        #[serde(deserialize_with = "deserialize_key_value")] Value,
        Direction,
    ),
    Button(MacroButton, Direction),
    MoveMouse(Value, Value, Coordinate),
    Scroll(Value, Axis),
    Text(TextToken),
    Raw(u16, Direction),
}

/// Saves from before the key became a value stored a bare `MacroKey`.
fn deserialize_key_value<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Value, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum KeyDe {
        Legacy(MacroKey),
        Current(Value),
    }
    Ok(match KeyDe::deserialize(d)? {
        KeyDe::Legacy(key) => Value::Text {
            value: crate::input::key_names::key_name(&key),
        },
        KeyDe::Current(value) => value,
    })
}

#[cfg(test)]
mod key_token_tests {
    use super::*;

    #[test]
    fn legacy_key_token_loads_as_text_value() {
        let token: InputToken = serde_json::from_str(r#"{"Key":["Return","Press"]}"#).unwrap();
        assert_eq!(
            token,
            InputToken::Key(
                Value::Text {
                    value: "Enter".into()
                },
                Direction::Press
            )
        );
        let token: InputToken =
            serde_json::from_str(r#"{"Key":[{"Unicode":"a"},"Click"]}"#).unwrap();
        assert_eq!(
            token,
            InputToken::Key(Value::Text { value: "a".into() }, Direction::Click)
        );
    }

    #[test]
    fn current_key_token_round_trips() {
        let token = InputToken::Key(
            Value::Text {
                value: "Control+s".into(),
            },
            Direction::Click,
        );
        let json = serde_json::to_string(&token).unwrap();
        assert_eq!(serde_json::from_str::<InputToken>(&json).unwrap(), token);
    }
}

#[cfg(test)]
mod text_token_tests {
    use super::*;

    #[test]
    fn legacy_text_token_keeps_its_bare_shape() {
        let token: InputToken =
            serde_json::from_str(r#"{"Text":{"kind":"Text","value":"hello"}}"#).unwrap();
        assert_eq!(token, InputToken::Text(Value::text("hello").into()));
        assert_eq!(
            serde_json::to_value(&token).unwrap(),
            serde_json::json!({"Text":{"kind":"Text","value":"hello"}})
        );
    }

    #[test]
    fn text_token_with_speed_round_trips() {
        let token = InputToken::Text(TextToken {
            text: Value::text("hello"),
            speed: Some(TextSpeed {
                mode: TextSpeedMode::Total,
                value: Value::number(1000.0),
            }),
        });
        let json = serde_json::to_value(&token).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "Text": {
                    "text": {"kind": "Text", "value": "hello"},
                    "speed": {"mode": "Total", "value": {"kind": "Number", "value": 1000.0}}
                }
            })
        );
        assert_eq!(serde_json::from_value::<InputToken>(json).unwrap(), token);
    }

    #[test]
    fn total_text_speed_divides_evenly() {
        let speed = TextSpeed {
            mode: TextSpeedMode::Total,
            value: Value::number(100.0),
        };
        assert_eq!(speed.per_character_ms(100.0, 4), 25.0);
        assert_eq!(speed.per_character_ms(100.0, 0), 0.0);
        let speed = TextSpeed {
            mode: TextSpeedMode::PerCharacter,
            value: Value::number(30.0),
        };
        assert_eq!(speed.per_character_ms(30.0, 4), 30.0);
    }
}
