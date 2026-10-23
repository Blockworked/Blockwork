//! What the user just typed, rebuilt from raw key events for `WhenTyped`.
//! Assumes a US layout for shifted symbols.

use crate::input::types::MacroKey;

/// Longest abbreviation worth remembering keys for.
const MAX_LEN: usize = 64;

#[derive(Default)]
pub struct TypedBuffer {
    text: String,
    shift: bool,
    // Ctrl/Alt/Meta held: the keys are shortcuts, not typing.
    command: u8,
    caps_lock: bool,
}

impl TypedBuffer {
    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn clear(&mut self) {
        self.text.clear();
    }

    /// Applies one physical key event. Returns true when it typed a character.
    pub fn key(&mut self, key: &MacroKey, pressed: bool) -> bool {
        match key {
            MacroKey::Shift | MacroKey::LShift | MacroKey::RShift => {
                self.shift = pressed;
                return false;
            }
            MacroKey::Control | MacroKey::LControl | MacroKey::RControl | MacroKey::Alt | MacroKey::Meta | MacroKey::Option => {
                let bit = match key {
                    MacroKey::Alt | MacroKey::Option => 2,
                    MacroKey::Meta => 4,
                    _ => 1,
                };
                if pressed {
                    self.command |= bit;
                } else {
                    self.command &= !bit;
                }
                return false;
            }
            _ => {}
        }
        if !pressed {
            return false;
        }
        if self.command != 0 {
            // A shortcut like Ctrl+A or Ctrl+V changes the text unpredictably.
            self.clear();
            return false;
        }
        let c = match key {
            MacroKey::CapsLock => {
                self.caps_lock = !self.caps_lock;
                return false;
            }
            MacroKey::Backspace => {
                self.text.pop();
                return false;
            }
            MacroKey::Space => ' ',
            MacroKey::Unicode(c) => self.apply_shift(*c),
            MacroKey::Numpad0 => '0',
            MacroKey::Numpad1 => '1',
            MacroKey::Numpad2 => '2',
            MacroKey::Numpad3 => '3',
            MacroKey::Numpad4 => '4',
            MacroKey::Numpad5 => '5',
            MacroKey::Numpad6 => '6',
            MacroKey::Numpad7 => '7',
            MacroKey::Numpad8 => '8',
            MacroKey::Numpad9 => '9',
            MacroKey::Add => '+',
            MacroKey::Subtract => '-',
            MacroKey::Multiply => '*',
            MacroKey::Divide => '/',
            MacroKey::Decimal => '.',
            MacroKey::Other(code) => match windows_oem_char(*code) {
                Some(c) => self.apply_shift(c),
                None => {
                    self.clear();
                    return false;
                }
            },
            // Enter, Tab, arrows, clicks of other keys: the caret may have moved.
            _ => {
                self.clear();
                return false;
            }
        };
        self.text.push(c);
        if self.text.chars().count() > MAX_LEN {
            self.text.remove(0);
        }
        true
    }

    fn apply_shift(&self, c: char) -> char {
        if c.is_ascii_alphabetic() {
            return if self.shift != self.caps_lock { c.to_ascii_uppercase() } else { c.to_ascii_lowercase() };
        }
        if !self.shift {
            return c;
        }
        const PAIRS: &str = "1!2@3#4$5%6^7&8*9(0)-_=+[{]}\\|;:'\",<.>/?`~";
        let pairs: Vec<char> = PAIRS.chars().collect();
        pairs.chunks(2).find(|p| p[0] == c).map_or(c, |p| p[1])
    }

    /// Whether what was typed ends with `abbreviation`.
    pub fn ends_with(&self, abbreviation: &str) -> bool {
        !abbreviation.is_empty() && self.text.ends_with(abbreviation)
    }
}

/// Windows reports punctuation keys as raw OEM virtual-key codes.
fn windows_oem_char(code: u32) -> Option<char> {
    if !cfg!(windows) {
        return None;
    }
    Some(match code {
        0xBA => ';',
        0xBB => '=',
        0xBC => ',',
        0xBD => '-',
        0xBE => '.',
        0xBF => '/',
        0xC0 => '`',
        0xDB => '[',
        0xDC => '\\',
        0xDD => ']',
        0xDE => '\'',
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn type_keys(buffer: &mut TypedBuffer, keys: &[(MacroKey, bool)]) {
        for (key, pressed) in keys {
            buffer.key(key, *pressed);
        }
    }

    fn tap(c: char) -> [(MacroKey, bool); 2] {
        [(MacroKey::Unicode(c), true), (MacroKey::Unicode(c), false)]
    }

    #[test]
    fn rebuilds_shifted_text_and_backspaces() {
        let mut b = TypedBuffer::default();
        type_keys(&mut b, &tap(';'));
        type_keys(&mut b, &[(MacroKey::LShift, true)]);
        type_keys(&mut b, &tap('a'));
        type_keys(&mut b, &tap('1'));
        type_keys(&mut b, &[(MacroKey::LShift, false)]);
        type_keys(&mut b, &tap('d'));
        type_keys(&mut b, &tap('x'));
        type_keys(&mut b, &[(MacroKey::Backspace, true), (MacroKey::Backspace, false)]);
        assert_eq!(b.text(), ";A!d");
        assert!(b.ends_with("A!d"));
        assert!(!b.ends_with(""));

        type_keys(&mut b, &[(MacroKey::CapsLock, true), (MacroKey::CapsLock, false)]);
        type_keys(&mut b, &tap('q'));
        assert_eq!(b.text(), ";A!dQ");
    }

    #[test]
    fn shortcuts_and_navigation_reset_it() {
        let mut b = TypedBuffer::default();
        type_keys(&mut b, &tap('a'));
        type_keys(&mut b, &[(MacroKey::LControl, true)]);
        type_keys(&mut b, &tap('v'));
        type_keys(&mut b, &[(MacroKey::LControl, false)]);
        assert_eq!(b.text(), "");
        type_keys(&mut b, &tap('b'));
        type_keys(&mut b, &[(MacroKey::LeftArrow, true)]);
        assert_eq!(b.text(), "");
        type_keys(&mut b, &tap('c'));
        type_keys(&mut b, &[(MacroKey::Space, true)]);
        assert_eq!(b.text(), "c ");
    }
}
