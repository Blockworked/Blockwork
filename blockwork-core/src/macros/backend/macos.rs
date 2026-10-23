use std::sync::OnceLock;

use core_foundation::runloop::{CFRunLoop, kCFRunLoopDefaultMode};
use core_graphics::event::{CGEvent, CGEventFlags, CGEventTap, CGEventTapLocation, CGEventTapOptions, CGEventTapPlacement, CGEventType, CGMouseButton, CallbackResult, EventField};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use core_graphics::geometry::CGPoint;
use foreign_types::ForeignType;
use tracing::warn;

use crate::input::types::{Axis, Direction, MacroButton, MacroKey};
use crate::macros::backend::{CaptureDecision, CaptureEvent, CaptureTimestamp, InputBackend};

// ── Key mapping ───────────────────────────────────────────────────────────────

fn macro_key_to_cgkeycode(key: &MacroKey) -> Option<(u16, bool)> {
    // CGKeyCode values from HIToolbox/Events.h
    Some(match key {
        MacroKey::Return => (36, false),
        MacroKey::Backspace => (51, false),
        MacroKey::Tab => (48, false),
        MacroKey::Space => (49, false),
        MacroKey::Escape => (53, false),
        MacroKey::Delete => (117, false),
        MacroKey::Insert => return None, // no Insert on macOS
        MacroKey::Home => (115, false),
        MacroKey::End => (119, false),
        MacroKey::PageUp => (116, false),
        MacroKey::PageDown => (121, false),
        MacroKey::UpArrow => (126, false),
        MacroKey::DownArrow => (125, false),
        MacroKey::LeftArrow => (123, false),
        MacroKey::RightArrow => (124, false),
        MacroKey::Shift | MacroKey::LShift => (56, false),
        MacroKey::RShift => (60, false),
        MacroKey::Control | MacroKey::LControl => (59, false),
        MacroKey::RControl => (62, false),
        MacroKey::Alt | MacroKey::Option => (58, false),
        MacroKey::AltGr => (61, false),
        MacroKey::Meta => (55, false),
        MacroKey::CapsLock => (57, false),
        MacroKey::F1 => (122, false),
        MacroKey::F2 => (120, false),
        MacroKey::F3 => (99, false),
        MacroKey::F4 => (118, false),
        MacroKey::F5 => (96, false),
        MacroKey::F6 => (97, false),
        MacroKey::F7 => (98, false),
        MacroKey::F8 => (100, false),
        MacroKey::F9 => (101, false),
        MacroKey::F10 => (109, false),
        MacroKey::F11 => (103, false),
        MacroKey::F12 => (111, false),
        MacroKey::Unicode(c) => char_to_cgkeycode(*c)?,
        MacroKey::Other(n) => (*n as u16, false),
        _ => return None,
    })
}

fn char_to_cgkeycode(c: char) -> Option<(u16, bool)> {
    // Standard US keyboard layout CGKeyCodes.
    Some(match c {
        'a' | 'A' => (0, c.is_uppercase()),
        'b' | 'B' => (11, c.is_uppercase()),
        'c' | 'C' => (8, c.is_uppercase()),
        'd' | 'D' => (2, c.is_uppercase()),
        'e' | 'E' => (14, c.is_uppercase()),
        'f' | 'F' => (3, c.is_uppercase()),
        'g' | 'G' => (5, c.is_uppercase()),
        'h' | 'H' => (4, c.is_uppercase()),
        'i' | 'I' => (34, c.is_uppercase()),
        'j' | 'J' => (38, c.is_uppercase()),
        'k' | 'K' => (40, c.is_uppercase()),
        'l' | 'L' => (37, c.is_uppercase()),
        'm' | 'M' => (46, c.is_uppercase()),
        'n' | 'N' => (45, c.is_uppercase()),
        'o' | 'O' => (31, c.is_uppercase()),
        'p' | 'P' => (35, c.is_uppercase()),
        'q' | 'Q' => (12, c.is_uppercase()),
        'r' | 'R' => (15, c.is_uppercase()),
        's' | 'S' => (1, c.is_uppercase()),
        't' | 'T' => (17, c.is_uppercase()),
        'u' | 'U' => (32, c.is_uppercase()),
        'v' | 'V' => (9, c.is_uppercase()),
        'w' | 'W' => (13, c.is_uppercase()),
        'x' | 'X' => (7, c.is_uppercase()),
        'y' | 'Y' => (16, c.is_uppercase()),
        'z' | 'Z' => (6, c.is_uppercase()),
        '0' | ')' => (29, c == ')'),
        '1' | '!' => (18, c == '!'),
        '2' | '@' => (19, c == '@'),
        '3' | '#' => (20, c == '#'),
        '4' | '$' => (21, c == '$'),
        '5' | '%' => (23, c == '%'),
        '6' | '^' => (22, c == '^'),
        '7' | '&' => (26, c == '&'),
        '8' | '*' => (28, c == '*'),
        '9' | '(' => (25, c == '('),
        '-' | '_' => (27, c == '_'),
        '=' | '+' => (24, c == '+'),
        '[' | '{' => (33, c == '{'),
        ']' | '}' => (30, c == '}'),
        '\\' | '|' => (42, c == '|'),
        ';' | ':' => (41, c == ':'),
        '\'' | '"' => (39, c == '"'),
        '`' | '~' => (50, c == '~'),
        ',' | '<' => (43, c == '<'),
        '.' | '>' => (47, c == '>'),
        '/' | '?' => (44, c == '?'),
        ' ' => (49, false),
        '\n' => (36, false),
        '\t' => (48, false),
        _ => return None,
    })
}

// ── InputBackend impl ─────────────────────────────────────────────────────────

pub struct MacosBackend {
    source: CGEventSource,
}

// CGEventSource is safe to use across threads; the NonNull it wraps is an
// opaque CoreGraphics ref-counted object that CoreGraphics itself uses from
// any thread (the HID event tap runs on its own thread).
unsafe impl Send for MacosBackend {}

impl MacosBackend {
    pub fn new() -> Self {
        Self {
            source: CGEventSource::new(CGEventSourceStateID::HIDSystemState)
                .expect("CGEventSource"),
        }
    }

    fn post(&self, event: CGEvent) {
        event.post(CGEventTapLocation::HID);
    }

    fn scroll_raw(&self, wheel1: i32, wheel2: i32, wheel3: i32) {
        let wheel_count = if wheel2 != 0 || wheel3 != 0 { 2 } else { 1 };
        let raw = unsafe {
            CGEventCreateScrollWheelEvent2(
                self.source.as_ptr(),
                1, // kCGScrollEventUnitLine
                wheel_count,
                wheel1,
                wheel2,
                wheel3,
            )
        };
        if !raw.is_null() {
            let ev = unsafe { CGEvent::from_ptr(raw) };
            self.post(ev);
        }
    }

    fn emit_key(&self, keycode: u16, down: bool, needs_shift: bool) {
        if needs_shift {
            let ev = CGEvent::new_keyboard_event(self.source.clone(), 56, true).unwrap();
            self.post(ev);
        }
        let ev = CGEvent::new_keyboard_event(self.source.clone(), keycode, down).unwrap();
        self.post(ev);
        if needs_shift && !down {
            let ev = CGEvent::new_keyboard_event(self.source.clone(), 56, false).unwrap();
            self.post(ev);
        }
    }
}

impl InputBackend for MacosBackend {
    fn key(&mut self, key: MacroKey, dir: Direction) -> Result<(), String> {
        let (code, needs_shift) =
            macro_key_to_cgkeycode(&key).ok_or_else(|| format!("no CGKeyCode for {:?}", key))?;
        match dir {
            Direction::Press => {
                if needs_shift { self.emit_key(56, true, false); }
                self.emit_key(code, true, false);
            }
            Direction::Release => {
                self.emit_key(code, false, false);
                if needs_shift { self.emit_key(56, false, false); }
            }
            Direction::Click => {
                if needs_shift { self.emit_key(56, true, false); }
                self.emit_key(code, true, false);
                self.emit_key(code, false, false);
                if needs_shift { self.emit_key(56, false, false); }
            }
        }
        Ok(())
    }

    fn raw_keycode(&mut self, keycode: u16, dir: Direction) -> Result<(), String> {
        match dir {
            Direction::Press => self.emit_key(keycode, true, false),
            Direction::Release => self.emit_key(keycode, false, false),
            Direction::Click => {
                self.emit_key(keycode, true, false);
                self.emit_key(keycode, false, false);
            }
        }
        Ok(())
    }

    fn button(&mut self, button: MacroButton, dir: Direction) -> Result<(), String> {
        let cur = self.cursor_pos().unwrap_or((0, 0));
        let pos = CGPoint::new(cur.0 as f64, cur.1 as f64);

        let (down_ty, up_ty, mouse_btn) = match button {
            MacroButton::ScrollUp => {
                self.scroll_raw(1, 0, 0);
                return Ok(());
            }
            MacroButton::ScrollDown => {
                self.scroll_raw(-1, 0, 0);
                return Ok(());
            }
            MacroButton::ScrollLeft => {
                self.scroll_raw(0, -1, 0);
                return Ok(());
            }
            MacroButton::ScrollRight => {
                self.scroll_raw(0, 1, 0);
                return Ok(());
            }
            MacroButton::Left => (CGEventType::LeftMouseDown, CGEventType::LeftMouseUp, CGMouseButton::Left),
            MacroButton::Right => (CGEventType::RightMouseDown, CGEventType::RightMouseUp, CGMouseButton::Right),
            MacroButton::Middle | MacroButton::Other(_) => (CGEventType::OtherMouseDown, CGEventType::OtherMouseUp, CGMouseButton::Center),
            MacroButton::Back | MacroButton::Forward => return Ok(()),
        };

        match dir {
            Direction::Press | Direction::Click => {
                let ev = CGEvent::new_mouse_event(self.source.clone(), down_ty, pos, mouse_btn)
                    .map_err(|_| format!("CGEvent mouse down failed"))?;
                self.post(ev);
                if matches!(dir, Direction::Click) {
                    let ev = CGEvent::new_mouse_event(self.source.clone(), up_ty, pos, mouse_btn)
                        .map_err(|_| format!("CGEvent mouse up failed"))?;
                    self.post(ev);
                }
            }
            Direction::Release => {
                let ev = CGEvent::new_mouse_event(self.source.clone(), up_ty, pos, mouse_btn)
                    .map_err(|_| format!("CGEvent mouse up failed"))?;
                self.post(ev);
            }
        }
        Ok(())
    }

    fn move_mouse_rel(&mut self, dx: i32, dy: i32) -> Result<(), String> {
        let cur = self.cursor_pos().unwrap_or((0, 0));
        let new_x = cur.0 + dx;
        let new_y = cur.1 + dy;
        let pos = CGPoint::new(new_x as f64, new_y as f64);
        let ev = CGEvent::new_mouse_event(
            self.source.clone(),
            CGEventType::MouseMoved,
            pos,
            CGMouseButton::Left,
        )
        .map_err(|_| "CGEvent mouse move failed".to_string())?;
        ev.set_double_value_field(EventField::MOUSE_EVENT_DELTA_X, dx as f64);
        ev.set_double_value_field(EventField::MOUSE_EVENT_DELTA_Y, dy as f64);
        self.post(ev);
        Ok(())
    }

    fn move_mouse_abs(&mut self, x: i32, y: i32) -> Result<(), String> {
        let cur = self.cursor_pos().unwrap_or((0, 0));
        self.move_mouse_rel(x - cur.0, y - cur.1)
    }

    fn scroll(&mut self, amount: i32, axis: Axis) -> Result<(), String> {
        let (v, h) = match axis {
            Axis::Vertical => (amount, 0),
            Axis::Horizontal => (0, amount),
        };
        self.scroll_raw(v, h, 0);
        Ok(())
    }

    fn text(&mut self, s: &str) -> Result<(), String> {
        for c in s.chars() {
            let _ = self.key(MacroKey::Unicode(c), Direction::Click);
        }
        Ok(())
    }

    fn cursor_pos(&self) -> Option<(i32, i32)> {
        let loc = CGEvent::new(self.source.clone()).ok()?.location();
        Some((loc.x as i32, loc.y as i32))
    }
}

// ── Hardware event timestamp ────────────────────────────────────────────────

// `core-graphics` 0.23 does not bind `CGEventGetTimestamp` or
// `CGEventCreateScrollWheelEvent2`; declare them ourselves.
unsafe extern "C" {
    fn CGEventGetTimestamp(event: core_graphics::sys::CGEventRef) -> u64;

    // kCGScrollEventUnitPixel = 0, kCGScrollEventUnitLine = 1
    fn CGEventCreateScrollWheelEvent2(
        source: core_graphics::sys::CGEventSourceRef,
        units: u32,
        wheel_count: u32,
        wheel1: i32,
        wheel2: i32,
        wheel3: i32,
    ) -> core_graphics::sys::CGEventRef;
}

/// OS-assigned timestamp (mach_absolute_time-based ns since boot, NOT Unix
/// time) for when CoreGraphics generated this event. Stuffed into a
/// `SystemTime` purely as an opaque carrier for `CaptureTimestamp::Hardware`
/// - the recorder only ever diffs two `Hardware` values via `duration_since`,
/// never reads it as real wall-clock time.
fn cgevent_hardware_timestamp(event: &CGEvent) -> std::time::SystemTime {
    let ns = unsafe { CGEventGetTimestamp(event.as_ptr()) };
    std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_nanos(ns)
}

// ── Accessibility permission ──────────────────────────────────────────────────

/// Checks whether this process has been granted Accessibility permission.
/// When not trusted, shows the macOS system prompt asking the user to grant it.
/// Must be called from the main thread.
pub fn request_accessibility() -> bool {
    use core_foundation::base::TCFType;
    use core_foundation::boolean::CFBoolean;
    use core_foundation::dictionary::CFDictionary;
    use core_foundation::string::CFString;

    #[link(name = "ApplicationServices", kind = "framework")]
    unsafe extern "C" {
        fn AXIsProcessTrustedWithOptions(options: *const std::ffi::c_void) -> bool;
    }

    let key = CFString::from_static_string("AXTrustedCheckOptionPrompt");
    let val = CFBoolean::true_value();
    let opts = CFDictionary::from_CFType_pairs(&[(key, val)]);
    unsafe { AXIsProcessTrustedWithOptions(opts.as_concrete_TypeRef() as *const _) }
}

// ── Capture ───────────────────────────────────────────────────────────────────

pub(super) fn start_capture_thread(
    callback: Box<dyn FnMut(CaptureEvent, CaptureTimestamp) -> CaptureDecision + Send + 'static>,
) {
    static STARTED: OnceLock<()> = OnceLock::new();
    STARTED.get_or_init(|| {
        std::thread::Builder::new()
            .name("cgeventtap".into())
            .spawn(move || {
                use std::sync::{Arc, Mutex};
                crate::macros::priority::raise_current_thread_priority();

                // Shared between both tap closures. No actual mutex contention:
                // both taps run on the same single-threaded CFRunLoop.
                let callback = Arc::new(Mutex::new(callback));
                let run_loop = CFRunLoop::get_current();

                // Keep both taps and their RunLoop sources alive until run_current()
                // returns (which never happens in practice).
                let _kb_tap;
                let _kb_src;
                let _mouse_tap;
                let _mouse_src;

                // ── Keyboard tap (active, HeadInsert) ─────────────────────────
                // Must be active + HeadInsert so we can suppress keyboard events
                // when they match a hotkey. FlagsChanged is mapped to synthetic
                // KeyPress/KeyRelease so modifier-only hotkeys work.
                let cb_kb = Arc::clone(&callback);
                let prev_flags = Mutex::new(CGEventFlags::empty());
                match CGEventTap::new(
                    CGEventTapLocation::HID,
                    CGEventTapPlacement::HeadInsertEventTap,
                    CGEventTapOptions::Default,
                    vec![
                        CGEventType::KeyDown,
                        CGEventType::KeyUp,
                        CGEventType::FlagsChanged,
                    ],
                    move |_proxy, ev_type, event| {
                        if matches!(ev_type, CGEventType::FlagsChanged) {
                            let cur = event.get_flags();
                            let prev = {
                                let Ok(mut g) = prev_flags.lock() else {
                                    return CallbackResult::Replace(event.clone());
                                };
                                let p = *g;
                                *g = cur;
                                p
                            };
                            let pairs: &[(CGEventFlags, MacroKey)] = &[
                                (CGEventFlags::CGEventFlagControl,   MacroKey::LControl),
                                (CGEventFlags::CGEventFlagShift,     MacroKey::LShift),
                                (CGEventFlags::CGEventFlagAlternate, MacroKey::Alt),
                                (CGEventFlags::CGEventFlagCommand,   MacroKey::Meta),
                            ];
                            let ts = CaptureTimestamp::Hardware(cgevent_hardware_timestamp(event));
                            if let Ok(mut cb) = cb_kb.lock() {
                                for (flag, key) in pairs {
                                    let was = prev.contains(*flag);
                                    let now = cur.contains(*flag);
                                    if !was && now {
                                        cb(CaptureEvent::KeyPress(key.clone()), ts);
                                    } else if was && !now {
                                        cb(CaptureEvent::KeyRelease(key.clone()), ts);
                                    }
                                }
                            }
                            return CallbackResult::Replace(event.clone())
                        }

                        let capture_ev: Option<CaptureEvent> = match ev_type {
                            CGEventType::KeyDown => {
                                let kc = event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE) as u16;
                                cg_keycode_to_macro_key(kc).map(CaptureEvent::KeyPress)
                            }
                            CGEventType::KeyUp => {
                                let kc = event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE) as u16;
                                cg_keycode_to_macro_key(kc).map(CaptureEvent::KeyRelease)
                            }
                            _ => None,
                        };

                        if let Some(ev) = capture_ev {
                            if let Ok(mut cb) = cb_kb.lock() {
                                let ts = CaptureTimestamp::Hardware(cgevent_hardware_timestamp(event));
                                if matches!(cb(ev, ts), CaptureDecision::Suppress) {
                                    return CallbackResult::Drop;
                                }
                            }
                        }
                        CallbackResult::Replace(event.clone())
                    },
                ) {
                    Ok(tap) => {
                        let src = tap.mach_port().create_runloop_source(0).unwrap();
                        run_loop.add_source(&src, unsafe { kCFRunLoopDefaultMode });
                        tap.enable();
                        _kb_src = Some(src);
                        _kb_tap = Some(tap);
                    }
                    Err(_) => {
                        warn!("Keyboard event tap failed - grant Accessibility access in System Settings → Privacy & Security → Accessibility, then restart");
                        crate::recording::set_grab_failed(true);
                        _kb_src = None;
                        _kb_tap = None;
                    }
                }

                // ── Mouse tap (listen-only) ────────────────────────────────────
                // The OS fires this as a notification, not waiting for us before
                // delivering events elsewhere - window dragging/resizing is never
                // blocked or delayed by our recording code.
                let cb_mouse = Arc::clone(&callback);
                match CGEventTap::new(
                    CGEventTapLocation::Session,
                    CGEventTapPlacement::HeadInsertEventTap,
                    CGEventTapOptions::ListenOnly,
                    vec![
                        CGEventType::MouseMoved,
                        CGEventType::LeftMouseDown,
                        CGEventType::LeftMouseUp,
                        CGEventType::RightMouseDown,
                        CGEventType::RightMouseUp,
                        CGEventType::OtherMouseDown,
                        CGEventType::OtherMouseUp,
                        CGEventType::ScrollWheel,
                    ],
                    move |_proxy, ev_type, event| {
                        let capture_ev: Option<CaptureEvent> = match ev_type {
                            CGEventType::MouseMoved => {
                                let dx = event.get_double_value_field(EventField::MOUSE_EVENT_DELTA_X) as i32;
                                let dy = event.get_double_value_field(EventField::MOUSE_EVENT_DELTA_Y) as i32;
                                Some(CaptureEvent::MouseMoveRel(dx, dy))
                            }
                            CGEventType::LeftMouseDown => Some(CaptureEvent::ButtonPress(MacroButton::Left)),
                            CGEventType::LeftMouseUp => Some(CaptureEvent::ButtonRelease(MacroButton::Left)),
                            CGEventType::RightMouseDown => Some(CaptureEvent::ButtonPress(MacroButton::Right)),
                            CGEventType::RightMouseUp => Some(CaptureEvent::ButtonRelease(MacroButton::Right)),
                            CGEventType::OtherMouseDown => Some(CaptureEvent::ButtonPress(MacroButton::Middle)),
                            CGEventType::OtherMouseUp => Some(CaptureEvent::ButtonRelease(MacroButton::Middle)),
                            CGEventType::ScrollWheel => {
                                let v = event.get_integer_value_field(EventField::SCROLL_WHEEL_EVENT_POINT_DELTA_AXIS_1) as i32;
                                let h = event.get_integer_value_field(EventField::SCROLL_WHEEL_EVENT_POINT_DELTA_AXIS_2) as i32;
                                Some(CaptureEvent::Scroll(h, v))
                            }
                            _ => None,
                        };

                        if let Some(ev) = capture_ev {
                            if let Ok(mut cb) = cb_mouse.lock() {
                                let ts = CaptureTimestamp::Hardware(cgevent_hardware_timestamp(event));
                                cb(ev, ts); // CaptureDecision is ignored for listen-only taps
                            }
                        }
                        CallbackResult::Replace(event.clone())
                    },
                ) {
                    Ok(tap) => {
                        let src = tap.mach_port().create_runloop_source(1).unwrap();
                        run_loop.add_source(&src, unsafe { kCFRunLoopDefaultMode });
                        tap.enable();
                        _mouse_src = Some(src);
                        _mouse_tap = Some(tap);
                    }
                    Err(_) => {
                        warn!("Mouse event tap failed - mouse input will not be captured for recording");
                        _mouse_src = None;
                        _mouse_tap = None;
                    }
                }

                CFRunLoop::run_current();
            })
            .ok();
    });
}

fn cg_keycode_to_macro_key(code: u16) -> Option<MacroKey> {
    Some(match code {
        36 => MacroKey::Return,
        51 => MacroKey::Backspace,
        48 => MacroKey::Tab,
        49 => MacroKey::Space,
        53 => MacroKey::Escape,
        117 => MacroKey::Delete,
        115 => MacroKey::Home,
        119 => MacroKey::End,
        116 => MacroKey::PageUp,
        121 => MacroKey::PageDown,
        126 => MacroKey::UpArrow,
        125 => MacroKey::DownArrow,
        123 => MacroKey::LeftArrow,
        124 => MacroKey::RightArrow,
        56 => MacroKey::LShift,
        60 => MacroKey::RShift,
        59 => MacroKey::LControl,
        62 => MacroKey::RControl,
        58 => MacroKey::Alt,
        61 => MacroKey::AltGr,
        55 => MacroKey::Meta,
        57 => MacroKey::CapsLock,
        122 => MacroKey::F1,
        120 => MacroKey::F2,
        99 => MacroKey::F3,
        118 => MacroKey::F4,
        96 => MacroKey::F5,
        97 => MacroKey::F6,
        98 => MacroKey::F7,
        100 => MacroKey::F8,
        101 => MacroKey::F9,
        109 => MacroKey::F10,
        103 => MacroKey::F11,
        111 => MacroKey::F12,
        0 => MacroKey::Unicode('a'),
        11 => MacroKey::Unicode('b'),
        8 => MacroKey::Unicode('c'),
        2 => MacroKey::Unicode('d'),
        14 => MacroKey::Unicode('e'),
        3 => MacroKey::Unicode('f'),
        5 => MacroKey::Unicode('g'),
        4 => MacroKey::Unicode('h'),
        34 => MacroKey::Unicode('i'),
        38 => MacroKey::Unicode('j'),
        40 => MacroKey::Unicode('k'),
        37 => MacroKey::Unicode('l'),
        46 => MacroKey::Unicode('m'),
        45 => MacroKey::Unicode('n'),
        31 => MacroKey::Unicode('o'),
        35 => MacroKey::Unicode('p'),
        12 => MacroKey::Unicode('q'),
        15 => MacroKey::Unicode('r'),
        1 => MacroKey::Unicode('s'),
        17 => MacroKey::Unicode('t'),
        32 => MacroKey::Unicode('u'),
        9 => MacroKey::Unicode('v'),
        13 => MacroKey::Unicode('w'),
        7 => MacroKey::Unicode('x'),
        16 => MacroKey::Unicode('y'),
        6 => MacroKey::Unicode('z'),
        18 => MacroKey::Unicode('1'),
        19 => MacroKey::Unicode('2'),
        20 => MacroKey::Unicode('3'),
        21 => MacroKey::Unicode('4'),
        23 => MacroKey::Unicode('5'),
        22 => MacroKey::Unicode('6'),
        26 => MacroKey::Unicode('7'),
        28 => MacroKey::Unicode('8'),
        25 => MacroKey::Unicode('9'),
        29 => MacroKey::Unicode('0'),
        27 => MacroKey::Unicode('-'),
        24 => MacroKey::Unicode('='),
        33 => MacroKey::Unicode('['),
        30 => MacroKey::Unicode(']'),
        42 => MacroKey::Unicode('\\'),
        41 => MacroKey::Unicode(';'),
        39 => MacroKey::Unicode('\''),
        50 => MacroKey::Unicode('`'),
        43 => MacroKey::Unicode(','),
        47 => MacroKey::Unicode('.'),
        44 => MacroKey::Unicode('/'),
        _ => MacroKey::Other(code as u32),
    })
}
