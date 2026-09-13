//! Synthetic keystrokes delivered straight to one X11 window via the core
//! `SendEvent` request — the same technique `xdotool key --window <id>`
//! uses. Backs the `SendKeyToWindow`/`TypeTextInWindow` instructions: unlike
//! every real input path in this codebase (`macros::backend::evdev`'s
//! `uinput` virtual keyboard, or `XTestFakeKeyEvent` if this ever grew an
//! X11 capture/playback backend), a `SendEvent`-delivered `KeyPress`/
//! `KeyRelease` goes directly into one window's own event queue — it never
//! touches the real keyboard, never moves input focus, and is invisible to
//! every other window on the desktop.
//!
//! This only reaches genuine X11 client windows. Under a plain X11 session
//! that's everything; under a Wayland session it's whichever apps happen to
//! be running as XWayland clients (most Electron/Chromium apps unless
//! forced to native Wayland, many GTK3/Qt5 apps) — a native Wayland toplevel
//! has no X11 window at all, and there is no cross-client synthetic-input
//! mechanism on Wayland by design, XWayland or not. That's why the two
//! public functions here are called unconditionally by
//! `super::send_key_to_window`/`type_text_in_window` rather than being
//! gated behind `super::session()` like everything else in this directory:
//! this capability only ever targets X11-backed windows in the first place,
//! so it always attempts the shared `x11::ewmh()` connection regardless of
//! desktop session, and simply reports "no matching window" when the target
//! isn't reachable that way — never falling back to global input.
//!
//! Keysym resolution is deliberately conservative: every named `MacroKey`
//! (`Return`, `Tab`, arrows, F-keys, modifiers, numpad, media keys, ...) has
//! a fixed X11 keysym, and printable ASCII/Latin-1 characters map directly
//! (X11 keysyms equal the Unicode codepoint in that range). A character
//! outside that range — or `MacroKey::Other`, which is a raw
//! platform-specific keycode from a different keycode space entirely, not a
//! keysym — has no defined mapping here and errors clearly rather than
//! attempting `xdotool`-style temporary keyboard remapping, which briefly
//! mutates the shared X keyboard mapping for every client on the
//! desktop — a shared-state side effect this feature is explicitly trying
//! to avoid.

use super::x11::{Ewmh, ewmh, find_window, root};
use crate::input::types::MacroKey;
use x11rb::connection::Connection as _;
use x11rb::protocol::xproto::{
    ConnectionExt, EventMask, KEY_PRESS_EVENT, KEY_RELEASE_EVENT, KeyButMask, KeyPressEvent, KeyReleaseEvent, Keycode, Keysym, Window,
};

/// X11 keysyms for every named `MacroKey` variant — see `X11/keysymdef.h`.
/// `None` (`MacroKey::Other`, or a `Unicode` char outside the range
/// `keysym_for_char` handles) means "no defined mapping" — see the module
/// doc comment for why that's a clean error rather than a fallback.
fn keysym_for(key: &MacroKey) -> Option<Keysym> {
    Some(match key {
        MacroKey::Return => 0xff0d,
        MacroKey::Backspace => 0xff08,
        MacroKey::Tab => 0xff09,
        MacroKey::Space => 0x0020,
        MacroKey::Escape => 0xff1b,
        MacroKey::Delete => 0xffff,
        MacroKey::Insert => 0xff63,
        MacroKey::Home => 0xff50,
        MacroKey::End => 0xff57,
        MacroKey::PageUp => 0xff55,
        MacroKey::PageDown => 0xff56,
        MacroKey::UpArrow => 0xff52,
        MacroKey::DownArrow => 0xff54,
        MacroKey::LeftArrow => 0xff51,
        MacroKey::RightArrow => 0xff53,
        MacroKey::Shift | MacroKey::LShift => 0xffe1,
        MacroKey::RShift => 0xffe2,
        MacroKey::Control | MacroKey::LControl => 0xffe3,
        MacroKey::RControl => 0xffe4,
        MacroKey::Alt | MacroKey::Option => 0xffe9,
        MacroKey::AltGr => 0xfe03,
        MacroKey::Meta => 0xffeb,
        MacroKey::CapsLock => 0xffe5,
        MacroKey::NumLock => 0xff7f,
        MacroKey::ScrollLock => 0xff14,
        MacroKey::Pause => 0xff13,
        MacroKey::PrintScr => 0xff61,
        MacroKey::F1 => 0xffbe,
        MacroKey::F2 => 0xffbf,
        MacroKey::F3 => 0xffc0,
        MacroKey::F4 => 0xffc1,
        MacroKey::F5 => 0xffc2,
        MacroKey::F6 => 0xffc3,
        MacroKey::F7 => 0xffc4,
        MacroKey::F8 => 0xffc5,
        MacroKey::F9 => 0xffc6,
        MacroKey::F10 => 0xffc7,
        MacroKey::F11 => 0xffc8,
        MacroKey::F12 => 0xffc9,
        MacroKey::F13 => 0xffca,
        MacroKey::F14 => 0xffcb,
        MacroKey::F15 => 0xffcc,
        MacroKey::F16 => 0xffcd,
        MacroKey::F17 => 0xffce,
        MacroKey::F18 => 0xffcf,
        MacroKey::F19 => 0xffd0,
        MacroKey::F20 => 0xffd1,
        MacroKey::F21 => 0xffd2,
        MacroKey::F22 => 0xffd3,
        MacroKey::F23 => 0xffd4,
        MacroKey::F24 => 0xffd5,
        MacroKey::Numpad0 => 0xffb0,
        MacroKey::Numpad1 => 0xffb1,
        MacroKey::Numpad2 => 0xffb2,
        MacroKey::Numpad3 => 0xffb3,
        MacroKey::Numpad4 => 0xffb4,
        MacroKey::Numpad5 => 0xffb5,
        MacroKey::Numpad6 => 0xffb6,
        MacroKey::Numpad7 => 0xffb7,
        MacroKey::Numpad8 => 0xffb8,
        MacroKey::Numpad9 => 0xffb9,
        MacroKey::Add => 0xffab,
        MacroKey::Subtract => 0xffad,
        MacroKey::Multiply => 0xffaa,
        MacroKey::Divide => 0xffaf,
        MacroKey::Decimal => 0xffae,
        // XF86Audio* — not in core X11 keysymdef.h, but universally mapped
        // by every desktop's keyboard layout for the dedicated media keys.
        MacroKey::VolumeDown => 0x1008ff11,
        MacroKey::VolumeMute => 0x1008ff12,
        MacroKey::VolumeUp => 0x1008ff13,
        MacroKey::LMenu => 0xff67,
        MacroKey::Select => 0xff60,
        MacroKey::Unicode(c) => return keysym_for_char(*c),
        MacroKey::Other(_) => return None,
    })
}

/// Printable ASCII/Latin-1 keysyms equal their Unicode codepoint — the
/// range X11's own keysymdef.h defines this way. Anything past it (CJK,
/// emoji, combining marks, ...) has no fixed keysym and isn't attempted —
/// see the module doc comment.
fn keysym_for_char(c: char) -> Option<Keysym> {
    match c {
        '\n' | '\r' => Some(0xff0d),
        '\t' => Some(0xff09),
        ' '..='\u{ff}' => Some(c as Keysym),
        _ => None,
    }
}

/// Finds a keycode that currently maps to `keysym` on the connection's
/// active keyboard layout, and whether the Shift level is the one that
/// produces it. Queries the full mapping fresh each call — cheap (one
/// round trip covering every keycode) and consistent with this codebase's
/// "no cached handles" philosophy elsewhere (see `accessibility`'s module
/// doc comment).
fn find_keycode(e: &Ewmh, keysym: Keysym) -> Option<(Keycode, bool)> {
    let setup = e.conn.setup();
    let count = setup.max_keycode.saturating_sub(setup.min_keycode).saturating_add(1);
    let reply = e.conn.get_keyboard_mapping(setup.min_keycode, count).ok()?.reply().ok()?;
    let per_keycode = reply.keysyms_per_keycode as usize;
    if per_keycode == 0 {
        return None;
    }
    for (i, group) in reply.keysyms.chunks(per_keycode).enumerate() {
        // Level 0 = unshifted, level 1 = shifted — the only two levels this
        // feature ever needs (AltGr/level-3 characters aren't reachable
        // through the plain `MacroKey`/ASCII surface this module supports).
        if group.first().is_some_and(|&k| k == keysym) {
            return Some((setup.min_keycode + i as u8, false));
        }
        if group.get(1).is_some_and(|&k| k == keysym) {
            return Some((setup.min_keycode + i as u8, true));
        }
    }
    None
}

fn send_key_event(e: &Ewmh, target: Window, keycode: Keycode, state: KeyButMask, press: bool) -> Result<(), String> {
    let common = (keycode, 0u16, 0u32, root(e), target, 0u32, 0i16, 0i16, 0i16, 0i16, state, true);
    let result = if press {
        e.conn.send_event(
            false,
            target,
            EventMask::NO_EVENT,
            KeyPressEvent {
                response_type: KEY_PRESS_EVENT,
                detail: common.0,
                sequence: common.1,
                time: common.2,
                root: common.3,
                event: common.4,
                child: common.5,
                root_x: common.6,
                root_y: common.7,
                event_x: common.8,
                event_y: common.9,
                state: common.10,
                same_screen: common.11,
            },
        )
    } else {
        e.conn.send_event(
            false,
            target,
            EventMask::NO_EVENT,
            KeyReleaseEvent {
                response_type: KEY_RELEASE_EVENT,
                detail: common.0,
                sequence: common.1,
                time: common.2,
                root: common.3,
                event: common.4,
                child: common.5,
                root_x: common.6,
                root_y: common.7,
                event_x: common.8,
                event_y: common.9,
                state: common.10,
                same_screen: common.11,
            },
        )
    };
    result.map_err(|e| format!("failed to send synthetic key event: {e}"))?;
    e.conn.flush().map_err(|e| format!("failed to flush X11 connection: {e}"))
}

/// Presses then releases `keysym` at `target`, holding `base_state` (already
/// resolved modifier keycodes, if any) and adding Shift automatically when
/// the keysym's own keycode needs it (and Shift isn't already held).
fn tap(e: &Ewmh, target: Window, keysym: Keysym, base_state: KeyButMask) -> Result<(), String> {
    let (keycode, needs_shift) = find_keycode(e, keysym)
        .ok_or_else(|| format!("keysym 0x{keysym:x} isn't available on the current keyboard layout"))?;
    let state = if needs_shift { base_state | KeyButMask::SHIFT } else { base_state };
    send_key_event(e, target, keycode, state, true)?;
    send_key_event(e, target, keycode, state, false)
}

fn modifier_keycodes(e: &Ewmh, modifiers: u8) -> Result<Vec<Keycode>, String> {
    use crate::hotkey_types::{MOD_ALT, MOD_CTRL, MOD_META, MOD_SHIFT};
    let mut codes = Vec::new();
    let mut want = |keysym: Keysym| -> Result<(), String> {
        let (code, _) = find_keycode(e, keysym).ok_or_else(|| "a requested modifier key isn't available on the current keyboard layout".to_string())?;
        codes.push(code);
        Ok(())
    };
    if modifiers & MOD_CTRL != 0 {
        want(0xffe3)?; // Control_L
    }
    if modifiers & MOD_SHIFT != 0 {
        want(0xffe1)?; // Shift_L
    }
    if modifiers & MOD_ALT != 0 {
        want(0xffe9)?; // Alt_L
    }
    if modifiers & MOD_META != 0 {
        want(0xffeb)?; // Super_L
    }
    Ok(codes)
}

fn modifiers_to_state(modifiers: u8) -> KeyButMask {
    use crate::hotkey_types::{MOD_ALT, MOD_CTRL, MOD_META, MOD_SHIFT};
    let mut state = KeyButMask::from(0u16);
    if modifiers & MOD_CTRL != 0 {
        state |= KeyButMask::CONTROL;
    }
    if modifiers & MOD_SHIFT != 0 {
        state |= KeyButMask::SHIFT;
    }
    if modifiers & MOD_ALT != 0 {
        state |= KeyButMask::MOD1;
    }
    if modifiers & MOD_META != 0 {
        state |= KeyButMask::MOD4;
    }
    state
}

/// Sends `key` (optionally with `modifiers` — a `hotkey_types::MOD_*`
/// bitmask — held) to the first window matching both filters, via synthetic
/// `SendEvent`-delivered `KeyPress`/`KeyRelease`. `Ok(false)` means no
/// matching X11 window was found — not an error, same spirit as
/// `window::focus_window`; this includes the case where the target app is a
/// native Wayland client with no X11 window to send to at all.
pub fn send_key_to_window(app: Option<&str>, title_contains: Option<&str>, key: &MacroKey, modifiers: u8) -> Result<bool, String> {
    let e = ewmh().ok_or_else(|| "no X11 display reachable".to_string())?;
    let Some(target) = find_window(e, app, title_contains) else { return Ok(false) };
    let keysym = keysym_for(key).ok_or_else(|| format!("{key:?} has no defined X11 keysym mapping"))?;

    let mod_codes = modifier_keycodes(e, modifiers)?;
    for &code in &mod_codes {
        send_key_event(e, target.id, code, KeyButMask::from(0u16), true)?;
    }
    let result = tap(e, target.id, keysym, modifiers_to_state(modifiers));
    for &code in mod_codes.iter().rev() {
        send_key_event(e, target.id, code, KeyButMask::from(0u16), false)?;
    }
    result.map(|()| true)
}

/// Types `text` into the first window matching both filters, one character
/// at a time (auto-holding Shift per character as needed) via the same
/// synthetic `SendEvent` mechanism as `send_key_to_window`. `Ok(false)`
/// means no matching X11 window was found.
pub fn type_text_in_window(app: Option<&str>, title_contains: Option<&str>, text: &str) -> Result<bool, String> {
    let e = ewmh().ok_or_else(|| "no X11 display reachable".to_string())?;
    let Some(target) = find_window(e, app, title_contains) else { return Ok(false) };

    for c in text.chars() {
        let keysym = keysym_for_char(c).ok_or_else(|| format!("character '{c}' isn't supported for background typing (only ASCII/Latin-1 is)"))?;
        tap(e, target.id, keysym, KeyButMask::from(0u16))?;
    }
    Ok(true)
}
