//! The character a physical key types under the user's keyboard layout, for
//! typed-text triggers. Captured keys are named by where they sit on a US
//! keyboard, so on a German layout the key captured as `y` types `z`.
//! Without a layout to read (macOS, or no configured layout), callers fall
//! back to the US characters.

use crate::input::types::MacroKey;

/// Held modifiers that change which character a key types.
#[derive(Clone, Copy, Default, Debug)]
pub struct Mods {
    pub shift: bool,
    pub alt_gr: bool,
    pub caps_lock: bool,
}

/// What `key` types: `None` when the layout can't be read (use US
/// characters), `Some(None)` when it types nothing by itself (a dead key).
pub fn char_for(key: &MacroKey, mods: Mods) -> Option<Option<char>> {
    // Tests type on a US keyboard whatever the machine is set to.
    if cfg!(test) {
        return None;
    }
    imp::char_for(key, mods)
}

#[cfg(target_os = "linux")]
mod imp {
    use super::Mods;
    use crate::input::types::MacroKey;
    use crate::macros::backend::evdev_mapping::char_to_evdev;
    use std::cell::RefCell;
    use std::time::{Duration, Instant};
    use xkbcommon::xkb;

    /// How long a read layout is trusted before it's read again.
    const FRESH: Duration = Duration::from_secs(30);

    struct Loaded {
        at: Instant,
        keymap: Option<xkb::Keymap>,
    }

    thread_local! {
        static LOADED: RefCell<Option<Loaded>> = const { RefCell::new(None) };
    }

    pub(super) fn char_for(key: &MacroKey, mods: Mods) -> Option<Option<char>> {
        let MacroKey::Unicode(c) = key else {
            return None;
        };
        let (code, _) = char_to_evdev(*c)?;
        LOADED.with(|loaded| {
            let mut loaded = loaded.borrow_mut();
            if loaded.as_ref().is_none_or(|l| l.at.elapsed() > FRESH) {
                *loaded = Some(Loaded {
                    at: Instant::now(),
                    keymap: super::configured_layout()
                        .and_then(|(layout, variant)| keymap(&layout, &variant)),
                });
            }
            let keymap = loaded.as_ref()?.keymap.as_ref()?;
            Some(typed(keymap, u32::from(code.code()) + 8, mods))
        })
    }

    fn keymap(layout: &str, variant: &str) -> Option<xkb::Keymap> {
        let context = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
        xkb::Keymap::new_from_names(
            &context,
            "",
            "",
            layout,
            variant,
            None,
            xkb::KEYMAP_COMPILE_NO_FLAGS,
        )
    }

    fn typed(keymap: &xkb::Keymap, keycode: u32, mods: Mods) -> Option<char> {
        let bit = |name: &str| {
            let index = keymap.mod_get_index(name);
            if index == xkb::MOD_INVALID {
                0
            } else {
                1u32 << index
            }
        };
        let mut depressed = 0;
        if mods.shift {
            depressed |= bit(xkb::MOD_NAME_SHIFT);
        }
        // AltGr is the third level, which layouts put on Mod5.
        if mods.alt_gr {
            depressed |= bit("Mod5");
        }
        let locked = if mods.caps_lock {
            bit(xkb::MOD_NAME_CAPS)
        } else {
            0
        };
        let mut state = xkb::State::new(keymap);
        state.update_mask(depressed, 0, locked, 0, 0, 0);
        char::from_u32(state.key_get_utf32(xkb::Keycode::new(keycode))).filter(|c| *c != '\0')
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn a_german_keymap_swaps_y_and_z() {
            // Needs the xkeyboard-config data; skipped where it isn't installed.
            let Some(keymap) = keymap("de", "") else {
                return;
            };
            let code = |c| u32::from(char_to_evdev(c).unwrap().0.code()) + 8;
            let plain = Mods::default();
            assert_eq!(typed(&keymap, code('y'), plain), Some('z'));
            let shift = Mods {
                shift: true,
                ..plain
            };
            assert_eq!(typed(&keymap, code('7'), shift), Some('/'));
            let alt_gr = Mods {
                alt_gr: true,
                ..plain
            };
            assert_eq!(typed(&keymap, code('q'), alt_gr), Some('@'));
        }
    }
}

#[cfg(windows)]
mod imp {
    use super::Mods;
    use crate::input::types::MacroKey;
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        GetKeyboardLayout, MAPVK_VK_TO_VSC, MapVirtualKeyExW, ToUnicodeEx,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowThreadProcessId,
    };

    /// Leaves the system's dead-key state alone (Windows 10 1607 and later).
    const NO_STATE_CHANGE: u32 = 1 << 2;

    pub(super) fn char_for(key: &MacroKey, mods: Mods) -> Option<Option<char>> {
        // Windows already reports letters by the layout's virtual key.
        let vk = match key {
            MacroKey::Unicode(c) if c.is_ascii_alphanumeric() => c.to_ascii_uppercase() as u32,
            MacroKey::Other(code) if (0xBA..=0xE2).contains(code) => *code,
            _ => return None,
        };
        let mut state = [0u8; 256];
        if mods.shift {
            state[0x10] = 0x80;
        }
        if mods.alt_gr {
            // AltGr is Ctrl+Alt to Windows.
            state[0x11] = 0x80;
            state[0x12] = 0x80;
        }
        if mods.caps_lock {
            state[0x14] = 0x01;
        }
        let mut out = [0u16; 8];
        // SAFETY: the buffers outlive the calls and their sizes are passed.
        let typed = unsafe {
            let thread = GetWindowThreadProcessId(GetForegroundWindow(), std::ptr::null_mut());
            let layout = GetKeyboardLayout(thread);
            let scan = MapVirtualKeyExW(vk, MAPVK_VK_TO_VSC, layout);
            ToUnicodeEx(
                vk,
                scan,
                state.as_ptr(),
                out.as_mut_ptr(),
                out.len() as i32,
                NO_STATE_CHANGE,
                layout,
            )
        };
        Some(match typed {
            n if n > 0 => char::decode_utf16(out[..n as usize].iter().copied())
                .next()
                .and_then(Result::ok)
                .filter(|c| !c.is_control()),
            _ => None,
        })
    }
}

#[cfg(not(any(target_os = "linux", windows)))]
mod imp {
    use super::Mods;
    use crate::input::types::MacroKey;

    pub(super) fn char_for(_key: &MacroKey, _mods: Mods) -> Option<Option<char>> {
        None
    }
}

/// The first layout and variant the desktop is set to, unless it's plain US.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn configured_layout() -> Option<(String, String)> {
    let (layout, variant) = read_layout()?;
    let first = |s: &str| s.split(',').next().unwrap_or("").trim().to_string();
    let (layout, variant) = (first(&layout), first(&variant));
    (!layout.is_empty() && !(layout == "us" && variant.is_empty())).then_some((layout, variant))
}

#[cfg(target_os = "linux")]
fn read_layout() -> Option<(String, String)> {
    if let Ok(layout) = std::env::var("XKB_DEFAULT_LAYOUT")
        && !layout.is_empty()
    {
        return Some((
            layout,
            std::env::var("XKB_DEFAULT_VARIANT").unwrap_or_default(),
        ));
    }
    let home = dirs::home_dir().unwrap_or_default();
    // KDE.
    if let Ok(text) = std::fs::read_to_string(home.join(".config/kxkbrc"))
        && let Some(layout) = ini_value(&text, "LayoutList")
    {
        return Some((layout, ini_value(&text, "VariantList").unwrap_or_default()));
    }
    // GNOME: [('xkb', 'de+nodeadkeys'), ...]
    if let Ok(out) = std::process::Command::new("gsettings")
        .args(["get", "org.gnome.desktop.input-sources", "sources"])
        .output()
        && let Some(parsed) = gnome_source(&String::from_utf8_lossy(&out.stdout))
    {
        return Some(parsed);
    }
    // System-wide settings, as localectl writes them.
    for path in ["/etc/X11/xorg.conf.d/00-keyboard.conf"] {
        if let Ok(text) = std::fs::read_to_string(path) {
            let option = |name: &str| {
                text.lines()
                    .find(|l| l.contains(name))
                    .and_then(|l| l.split('"').nth(3))
                    .map(str::to_string)
            };
            if let Some(layout) = option("XkbLayout") {
                return Some((layout, option("XkbVariant").unwrap_or_default()));
            }
        }
    }
    for path in ["/etc/default/keyboard", "/etc/vconsole.conf"] {
        if let Ok(text) = std::fs::read_to_string(path)
            && let Some(layout) = ini_value(&text, "XKBLAYOUT")
        {
            return Some((layout, ini_value(&text, "XKBVARIANT").unwrap_or_default()));
        }
    }
    None
}

#[cfg(not(target_os = "linux"))]
fn read_layout() -> Option<(String, String)> {
    None
}

/// `key=value` in a config file, unquoted.
#[cfg_attr(not(any(target_os = "linux", test)), allow(dead_code))]
fn ini_value(text: &str, key: &str) -> Option<String> {
    text.lines().find_map(|line| {
        let (k, v) = line.split_once('=')?;
        (k.trim() == key)
            .then(|| v.trim().trim_matches('"').to_string())
            .filter(|v| !v.is_empty())
    })
}

/// The first xkb source in GNOME's input-sources list.
#[cfg_attr(not(any(target_os = "linux", test)), allow(dead_code))]
fn gnome_source(text: &str) -> Option<(String, String)> {
    let start = text.find("('xkb', '")? + "('xkb', '".len();
    let name = &text[start..start + text[start..].find('\'')?];
    let (layout, variant) = name.split_once('+').unwrap_or((name, ""));
    Some((layout.to_string(), variant.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_layout_settings() {
        assert_eq!(
            gnome_source("[('xkb', 'de+nodeadkeys'), ('xkb', 'us')]\n"),
            Some(("de".into(), "nodeadkeys".into()))
        );
        assert_eq!(gnome_source("@a(ss) []"), None);
        assert_eq!(
            ini_value("[Layout]\nLayoutList=fr,us\nUse=true\n", "LayoutList").as_deref(),
            Some("fr,us")
        );
        assert_eq!(
            ini_value("XKBLAYOUT=\"de\"\n", "XKBLAYOUT").as_deref(),
            Some("de")
        );
    }
}
