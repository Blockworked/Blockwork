use std::{cell::RefCell, sync::OnceLock};

use core_foundation::runloop::{CFRunLoop, kCFRunLoopDefaultMode};
use core_graphics::event::{
    CGEvent, CGEventFlags, CGEventTap, CGEventTapLocation, CGEventTapOptions, CGEventTapPlacement,
    CGEventType, CGMouseButton, CallbackResult, EventField,
};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use core_graphics::geometry::CGPoint;
use foreign_types::ForeignType;
use tracing::warn;

use crate::input::types::{Axis, Direction, MacroButton, MacroKey};
use crate::macros::backend::{CaptureDecision, CaptureEvent, CaptureTimestamp, InputBackend};

mod system_keys;

const SYNTHETIC_EVENT_TAG: i64 = 0x424c4f434b574f52;

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
        MacroKey::Insert => (114, false), // Help/Insert on extended keyboards
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
        MacroKey::F13 => (105, false),
        MacroKey::F14 => (107, false),
        MacroKey::F15 => (113, false),
        MacroKey::F16 => (106, false),
        MacroKey::F17 => (64, false),
        MacroKey::F18 => (79, false),
        MacroKey::F19 => (80, false),
        MacroKey::F20 => (90, false),
        MacroKey::Numpad0 => (82, false),
        MacroKey::Numpad1 => (83, false),
        MacroKey::Numpad2 => (84, false),
        MacroKey::Numpad3 => (85, false),
        MacroKey::Numpad4 => (86, false),
        MacroKey::Numpad5 => (87, false),
        MacroKey::Numpad6 => (88, false),
        MacroKey::Numpad7 => (89, false),
        MacroKey::Numpad8 => (91, false),
        MacroKey::Numpad9 => (92, false),
        MacroKey::Add => (69, false),
        MacroKey::Subtract => (78, false),
        MacroKey::Multiply => (67, false),
        MacroKey::Divide => (75, false),
        MacroKey::Decimal => (65, false),
        MacroKey::NumLock => (71, false),
        MacroKey::LMenu => (110, false),
        MacroKey::VolumeUp => (72, false),
        MacroKey::VolumeDown => (73, false),
        MacroKey::VolumeMute => (74, false),
        MacroKey::Unicode(c) => char_to_cgkeycode(*c)?,
        MacroKey::Other(n) => (u16::try_from(*n).ok()?, false),
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

fn mouse_button_number(button: &MacroButton) -> Result<u32, String> {
    let number = match button {
        MacroButton::Left => 0,
        MacroButton::Right => 1,
        MacroButton::Middle => 2,
        MacroButton::Back => 3,
        MacroButton::Forward => 4,
        MacroButton::Other(n) => *n as u32,
        _ => return Err("a scroll wheel is not a mouse button".into()),
    };
    if number < 32 {
        Ok(number)
    } else {
        Err(format!("mouse button {number} is out of range"))
    }
}

fn captured_mouse_button(event: &CGEvent) -> MacroButton {
    match event.get_integer_value_field(EventField::MOUSE_EVENT_BUTTON_NUMBER) {
        0 => MacroButton::Left,
        1 => MacroButton::Right,
        2 => MacroButton::Middle,
        3 => MacroButton::Back,
        4 => MacroButton::Forward,
        n => MacroButton::Other(n as u8),
    }
}

fn captured_movement(event: &CGEvent, relative: bool) -> CaptureEvent {
    if relative {
        CaptureEvent::MouseMoveRel(
            event.get_double_value_field(EventField::MOUSE_EVENT_DELTA_X) as i32,
            event.get_double_value_field(EventField::MOUSE_EVENT_DELTA_Y) as i32,
        )
    } else {
        // A listen-only tap can run after the cursor has moved again. Preserve
        // the position belonging to this event instead of querying a later one.
        let point = event.location();
        CaptureEvent::MouseMoveAbs(point.x, point.y)
    }
}

fn modifier_event(event: &CGEvent) -> Option<CaptureEvent> {
    // Device-specific bits from IOLLEvent.h distinguish the two sides even
    // while both Shift/Control/Option keys are down. Aggregate flags cannot.
    let code = event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE) as u16;
    let mask = match code {
        59 => 0x0001,
        62 => 0x2000,
        56 => 0x0002,
        60 => 0x0004,
        55 => 0x0008,
        54 => 0x0010,
        58 => 0x0020,
        61 => 0x0040,
        57 => CGEventFlags::CGEventFlagAlphaShift.bits(),
        _ => return None,
    };
    let key = cg_keycode_to_macro_key(code)?;
    Some(if event.get_flags().bits() & mask != 0 {
        CaptureEvent::KeyPress(key)
    } else {
        CaptureEvent::KeyRelease(key)
    })
}

#[derive(Default)]
struct ScrollAccumulator {
    horizontal: f64,
    vertical: f64,
}

impl ScrollAccumulator {
    fn read(&mut self, event: &CGEvent) -> (i32, i32) {
        fn axis(event: &CGEvent, fixed: u32, integer: u32, remainder: &mut f64) -> i32 {
            let mut delta = event.get_double_value_field(fixed);
            if delta == 0.0 {
                delta = event.get_integer_value_field(integer) as f64;
            }
            if !delta.is_finite() {
                return 0;
            }
            *remainder += delta;
            let whole = remainder.trunc() as i32;
            *remainder -= whole as f64;
            whole
        }
        let v = axis(
            event,
            EventField::SCROLL_WHEEL_EVENT_FIXED_POINT_DELTA_AXIS_1,
            EventField::SCROLL_WHEEL_EVENT_DELTA_AXIS_1,
            &mut self.vertical,
        );
        let h = axis(
            event,
            EventField::SCROLL_WHEEL_EVENT_FIXED_POINT_DELTA_AXIS_2,
            EventField::SCROLL_WHEEL_EVENT_DELTA_AXIS_2,
            &mut self.horizontal,
        );
        (h, v)
    }
}

fn keyboard_flags(code: u16, mut flags: CGEventFlags, fn_down: bool) -> CGEventFlags {
    // Function/keypad/help identify the current key. They can be left in the
    // source's last-event state by F-keys and must not turn a later C into
    // Fn+C (Control Center) or send other letters to system shortcuts.
    flags.remove(
        CGEventFlags::CGEventFlagSecondaryFn
            | CGEventFlags::CGEventFlagNumericPad
            | CGEventFlags::CGEventFlagHelp,
    );
    if fn_down
        || matches!(
            code,
            122 | 120
                | 99
                | 118
                | 96
                | 97
                | 98
                | 100
                | 101
                | 109
                | 103
                | 111
                | 105
                | 107
                | 113
                | 106
                | 64
                | 79
                | 80
                | 90
        )
    {
        flags.insert(CGEventFlags::CGEventFlagSecondaryFn);
    }
    if matches!(code, 65 | 67 | 69 | 71 | 75 | 76 | 78 | 81 | 82..=89 | 91 | 92 | 123..=126) {
        flags.insert(CGEventFlags::CGEventFlagNumericPad);
    }
    if code == 114 {
        flags.insert(CGEventFlags::CGEventFlagHelp);
    }
    flags
}

// ── InputBackend impl ─────────────────────────────────────────────────────────

// CoreGraphics posts asynchronously. Motion counters acknowledge delivery at
// the WindowServer, so a queued move can supply the intended cursor position
// until all preceding moves have arrived. This also handles returning to the
// starting position and lets subsequent physical input update the cursor.
const MOTION_TYPES: [CGEventType; 4] = [
    CGEventType::MouseMoved,
    CGEventType::LeftMouseDragged,
    CGEventType::RightMouseDragged,
    CGEventType::OtherMouseDragged,
];

fn motion_counts() -> [u32; 4] {
    MOTION_TYPES.map(|kind| unsafe { CGEventSourceCounterForEventType(0, kind as u32) })
}

#[derive(Default)]
struct QueuedCursor {
    expected: [u32; 4],
    target: Option<(i32, i32)>,
}

impl QueuedCursor {
    fn resolve(&mut self, live: (i32, i32), counts: [u32; 4]) -> (i32, i32) {
        if let Some(target) = self.target {
            let delivered = counts
                .iter()
                .zip(self.expected)
                .all(|(&current, expected)| current.wrapping_sub(expected) < (1 << 31));
            if !delivered {
                return target;
            }
        }
        self.target = None;
        self.expected = counts;
        live
    }

    fn enqueue(&mut self, target: (i32, i32), kind: CGEventType) {
        let index = MOTION_TYPES
            .iter()
            .position(|candidate| *candidate as u32 == kind as u32)
            .expect("only motion events enter the cursor queue");
        self.expected[index] = self.expected[index].wrapping_add(1);
        self.target = Some(target);
    }
}

pub struct MacosBackend {
    source: CGEventSource,
    cursor: RefCell<QueuedCursor>,
    held_buttons: u32,
    modifier_bits: u64,
    managed_modifiers: u64,
    fn_key_down: Option<bool>,
}

// CGEventSource is safe to use across threads; the NonNull it wraps is an
// opaque CoreGraphics ref-counted object that CoreGraphics itself uses from
// any thread (the HID event tap runs on its own thread).
unsafe impl Send for MacosBackend {}

impl MacosBackend {
    pub fn new() -> Result<Self, String> {
        Ok(Self {
            source: CGEventSource::new(CGEventSourceStateID::HIDSystemState)
                .map_err(|_| "couldn't create a CoreGraphics event source".to_string())?,
            cursor: RefCell::default(),
            held_buttons: 0,
            modifier_bits: 0,
            managed_modifiers: 0,
            fn_key_down: None,
        })
    }

    fn post(&self, event: CGEvent) {
        event.set_integer_value_field(EventField::EVENT_SOURCE_USER_DATA, SYNTHETIC_EVENT_TAG);
        event.post(CGEventTapLocation::HID);
    }

    fn scroll_raw(&self, wheel1: i32, wheel2: i32, wheel3: i32) -> Result<(), String> {
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
        if raw.is_null() {
            return Err("couldn't create a scroll event".into());
        }
        let ev = unsafe { CGEvent::from_ptr(raw) };
        let (x, y) = self
            .cursor_pos()
            .ok_or("couldn't read the cursor position")?;
        ev.set_location(CGPoint::new(x as f64, y as f64));
        self.post(ev);
        Ok(())
    }

    fn keyboard_event(&mut self, keycode: u16, down: bool) -> Result<CGEvent, String> {
        let event = CGEvent::new_keyboard_event(self.source.clone(), keycode, down)
            .map_err(|_| "couldn't create a keyboard event".to_string())?;
        let pairs = [
            (0x0001 | 0x2000, CGEventFlags::CGEventFlagControl.bits()),
            (0x0002 | 0x0004, CGEventFlags::CGEventFlagShift.bits()),
            (0x0008 | 0x0010, CGEventFlags::CGEventFlagCommand.bits()),
            (0x0020 | 0x0040, CGEventFlags::CGEventFlagAlternate.bits()),
        ];
        let side = match keycode {
            59 => 0x0001,
            62 => 0x2000,
            56 => 0x0002,
            60 => 0x0004,
            55 => 0x0008,
            54 => 0x0010,
            58 => 0x0020,
            61 => 0x0040,
            _ => 0,
        };
        let mut flags = event.get_flags().bits();
        if side != 0 {
            if down {
                self.modifier_bits |= side;
            } else {
                self.modifier_bits &= !side;
            }
            for (sides, flag) in pairs {
                if side & sides != 0 {
                    self.managed_modifiers |= flag;
                }
            }
        }
        // Posting is asynchronous. Reading the HID state between two posts can
        // still return the flags from before the first event was delivered.
        for (sides, flag) in pairs {
            if self.managed_modifiers & flag == 0 {
                continue;
            }
            flags &= !(sides | flag);
            flags |= self.modifier_bits & sides;
            if self.modifier_bits & sides != 0 {
                flags |= flag;
            }
        }
        if keycode == 63 {
            self.fn_key_down = Some(down);
        }
        let fn_down = self.fn_key_down.unwrap_or_else(|| unsafe {
            CGEventSourceKeyState(CGEventSourceStateID::HIDSystemState as i32, 63)
        });
        event.set_flags(keyboard_flags(
            keycode,
            CGEventFlags::from_bits_retain(flags),
            fn_down,
        ));
        Ok(event)
    }

    fn emit_key(&mut self, keycode: u16, down: bool) -> Result<(), String> {
        let event = self.keyboard_event(keycode, down)?;
        self.post(event);
        Ok(())
    }

    fn movement_event(&self, x: i32, y: i32, dx: i32, dy: i32) -> Result<CGEvent, String> {
        let number = if self.held_buttons == 0 {
            0
        } else {
            self.held_buttons.trailing_zeros()
        };
        let kind = if self.held_buttons == 0 {
            CGEventType::MouseMoved
        } else if number == 0 {
            CGEventType::LeftMouseDragged
        } else if number == 1 {
            CGEventType::RightMouseDragged
        } else {
            CGEventType::OtherMouseDragged
        };
        let event = CGEvent::new_mouse_event(
            self.source.clone(),
            kind,
            CGPoint::new(x as f64, y as f64),
            CGMouseButton::Left,
        )
        .map_err(|_| "couldn't create a mouse movement event".to_string())?;
        event.set_integer_value_field(EventField::MOUSE_EVENT_BUTTON_NUMBER, number as i64);
        event.set_double_value_field(EventField::MOUSE_EVENT_DELTA_X, dx as f64);
        event.set_double_value_field(EventField::MOUSE_EVENT_DELTA_Y, dy as f64);
        Ok(event)
    }

    fn unicode_event(&self, text: &str, down: bool) -> Result<CGEvent, String> {
        let event = CGEvent::new_keyboard_event(self.source.clone(), 0, down)
            .map_err(|_| "couldn't create a Unicode keyboard event".to_string())?;
        // Text is independent of the active keyboard layout and held modifiers.
        event.set_flags(CGEventFlags::empty());
        event.set_string(text);
        Ok(event)
    }
}

impl InputBackend for MacosBackend {
    fn unavailable_reason(&self) -> Option<String> {
        // Creating a CGEventSource succeeds even when macOS will discard every
        // posted event. Check the actual synthesizing grant before a run so
        // the input block reports the problem instead of claiming success.
        unsafe extern "C" {
            fn CGPreflightPostEventAccess() -> bool;
        }
        if unsafe { CGPreflightPostEventAccess() } {
            None
        } else {
            Some("macOS input access is disabled for the Blockwork process running this macro; enable it in System Settings → Privacy & Security → Device Control and Data Access (Accessibility), then restart Blockwork".into())
        }
    }

    fn key(&mut self, key: MacroKey, dir: Direction) -> Result<(), String> {
        if let Some(code) = system_keys::code(&key) {
            let virtual_code = macro_key_to_cgkeycode(&key).unwrap().0;
            for down in [true, false] {
                if (down && matches!(dir, Direction::Release))
                    || (!down && matches!(dir, Direction::Press))
                {
                    continue;
                }
                let flags = self.keyboard_event(virtual_code, down)?.get_flags();
                self.post(system_keys::event(code, down, flags)?);
            }
            return Ok(());
        }
        if let MacroKey::Unicode(c) = key {
            if char_to_cgkeycode(c).is_none() {
                let mut bytes = [0; 4];
                let text = c.encode_utf8(&mut bytes);
                if !matches!(dir, Direction::Release) {
                    self.post(self.unicode_event(text, true)?);
                }
                if !matches!(dir, Direction::Press) {
                    self.post(self.unicode_event(text, false)?);
                }
                return Ok(());
            }
        }
        let (code, needs_shift) =
            macro_key_to_cgkeycode(&key).ok_or_else(|| format!("no CGKeyCode for {:?}", key))?;
        match dir {
            Direction::Press => {
                if needs_shift {
                    self.emit_key(56, true)?;
                }
                self.emit_key(code, true)?;
            }
            Direction::Release => {
                self.emit_key(code, false)?;
                if needs_shift {
                    self.emit_key(56, false)?;
                }
            }
            Direction::Click => {
                if needs_shift {
                    self.emit_key(56, true)?;
                }
                self.emit_key(code, true)?;
                self.emit_key(code, false)?;
                if needs_shift {
                    self.emit_key(56, false)?;
                }
            }
        }
        Ok(())
    }

    fn raw_keycode(&mut self, keycode: u16, dir: Direction) -> Result<(), String> {
        match dir {
            Direction::Press => self.emit_key(keycode, true)?,
            Direction::Release => self.emit_key(keycode, false)?,
            Direction::Click => {
                self.emit_key(keycode, true)?;
                self.emit_key(keycode, false)?;
            }
        }
        Ok(())
    }

    fn button(&mut self, button: MacroButton, dir: Direction) -> Result<(), String> {
        if matches!(
            button,
            MacroButton::ScrollUp
                | MacroButton::ScrollDown
                | MacroButton::ScrollLeft
                | MacroButton::ScrollRight
        ) && matches!(dir, Direction::Release)
        {
            return Ok(());
        }
        let cur = self
            .cursor_pos()
            .ok_or("couldn't read the cursor position")?;
        let pos = CGPoint::new(cur.0 as f64, cur.1 as f64);

        let (down_ty, up_ty, mouse_btn) = match button {
            MacroButton::ScrollUp => {
                self.scroll_raw(1, 0, 0)?;
                return Ok(());
            }
            MacroButton::ScrollDown => {
                self.scroll_raw(-1, 0, 0)?;
                return Ok(());
            }
            MacroButton::ScrollLeft => {
                self.scroll_raw(0, -1, 0)?;
                return Ok(());
            }
            MacroButton::ScrollRight => {
                self.scroll_raw(0, 1, 0)?;
                return Ok(());
            }
            MacroButton::Left => (
                CGEventType::LeftMouseDown,
                CGEventType::LeftMouseUp,
                CGMouseButton::Left,
            ),
            MacroButton::Right => (
                CGEventType::RightMouseDown,
                CGEventType::RightMouseUp,
                CGMouseButton::Right,
            ),
            MacroButton::Middle
            | MacroButton::Back
            | MacroButton::Forward
            | MacroButton::Other(_) => (
                CGEventType::OtherMouseDown,
                CGEventType::OtherMouseUp,
                CGMouseButton::Center,
            ),
        };

        let number = mouse_button_number(&button)?;
        match dir {
            Direction::Press | Direction::Click => {
                let ev = CGEvent::new_mouse_event(self.source.clone(), down_ty, pos, mouse_btn)
                    .map_err(|_| format!("CGEvent mouse down failed"))?;
                ev.set_integer_value_field(EventField::MOUSE_EVENT_BUTTON_NUMBER, number as i64);
                self.post(ev);
                self.held_buttons |= 1 << number;
                if matches!(dir, Direction::Click) {
                    let ev = CGEvent::new_mouse_event(self.source.clone(), up_ty, pos, mouse_btn)
                        .map_err(|_| format!("CGEvent mouse up failed"))?;
                    ev.set_integer_value_field(
                        EventField::MOUSE_EVENT_BUTTON_NUMBER,
                        number as i64,
                    );
                    self.post(ev);
                    self.held_buttons &= !(1 << number);
                }
            }
            Direction::Release => {
                let ev = CGEvent::new_mouse_event(self.source.clone(), up_ty, pos, mouse_btn)
                    .map_err(|_| format!("CGEvent mouse up failed"))?;
                ev.set_integer_value_field(EventField::MOUSE_EVENT_BUTTON_NUMBER, number as i64);
                self.post(ev);
                self.held_buttons &= !(1 << number);
            }
        }
        Ok(())
    }

    fn move_mouse_rel(&mut self, dx: i32, dy: i32) -> Result<(), String> {
        let cur = self
            .cursor_pos()
            .ok_or("couldn't read the cursor position")?;
        let event =
            self.movement_event(cur.0.saturating_add(dx), cur.1.saturating_add(dy), dx, dy)?;
        self.cursor.borrow_mut().enqueue(
            (cur.0.saturating_add(dx), cur.1.saturating_add(dy)),
            event.get_type(),
        );
        self.post(event);
        Ok(())
    }

    fn move_mouse_abs(&mut self, x: i32, y: i32) -> Result<(), String> {
        let cur = self
            .cursor_pos()
            .ok_or("couldn't read the cursor position")?;
        let event = self.movement_event(x, y, x.saturating_sub(cur.0), y.saturating_sub(cur.1))?;
        self.cursor.borrow_mut().enqueue((x, y), event.get_type());
        self.post(event);
        Ok(())
    }

    fn scroll(&mut self, amount: i32, axis: Axis) -> Result<(), String> {
        let (v, h) = match axis {
            Axis::Vertical => (amount, 0),
            Axis::Horizontal => (0, amount),
        };
        self.scroll_raw(v, h, 0)?;
        Ok(())
    }

    fn text(&mut self, s: &str) -> Result<(), String> {
        for c in s.chars() {
            let mut bytes = [0; 4];
            let text = c.encode_utf8(&mut bytes);
            let down = self.unicode_event(text, true)?;
            let up = self.unicode_event(text, false)?;
            self.post(down);
            self.post(up);
        }
        Ok(())
    }

    fn cursor_pos(&self) -> Option<(i32, i32)> {
        let counts = motion_counts();
        let loc = CGEvent::new(self.source.clone()).ok()?.location();
        Some(
            self.cursor
                .borrow_mut()
                .resolve((loc.x as i32, loc.y as i32), counts),
        )
    }
}

// ── Hardware event timestamp ────────────────────────────────────────────────

// `core-graphics` 0.23 does not bind `CGEventGetTimestamp` or
// `CGEventCreateScrollWheelEvent2`; declare them ourselves.
unsafe extern "C" {
    fn CGEventSourceCounterForEventType(state: i32, event_type: u32) -> u32;
    fn CGEventGetTimestamp(event: core_graphics::sys::CGEventRef) -> u64;
    fn CGEventSourceKeyState(state: i32, keycode: u16) -> bool;
    fn CGEventTapEnable(tap: core_foundation::mach_port::CFMachPortRef, enabled: bool);
    #[cfg(test)]
    fn CGEventTapIsEnabled(tap: core_foundation::mach_port::CFMachPortRef) -> bool;

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

fn recover_tap(kind: CGEventType, port: core_foundation::mach_port::CFMachPortRef) -> bool {
    recover_tap_type(kind as u32, port)
}

fn recover_tap_type(kind: u32, port: core_foundation::mach_port::CFMachPortRef) -> bool {
    if matches!(kind, 0xffff_fffe | 0xffff_ffff) {
        if !port.is_null() {
            unsafe {
                CGEventTapEnable(port, true);
            }
            warn!("macOS disabled an input tap; re-enabled it");
        }
        true
    } else {
        false
    }
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
                use std::sync::atomic::{AtomicPtr, Ordering};
                use core_foundation::base::TCFType;
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
                match system_keys::KeyTap::new(
                    move |ev_type, event| {
                        // Playback must not start hotkeys or re-record itself.
                        if event.get_integer_value_field(EventField::EVENT_SOURCE_USER_DATA) == SYNTHETIC_EVENT_TAG {
                            return CaptureDecision::Passthrough;
                        }
                        if ev_type == 12 {
                            if let Some(ev) = modifier_event(event) {
                                if let Ok(mut cb) = cb_kb.lock() {
                                    let ts = CaptureTimestamp::Hardware(cgevent_hardware_timestamp(event));
                                    if matches!(cb(ev, ts), CaptureDecision::Suppress) {
                                        return CaptureDecision::Suppress;
                                    }
                                }
                            }
                            return CaptureDecision::Passthrough;
                        }

                        let capture_ev: Option<CaptureEvent> = match ev_type {
                            10 => {
                                let kc = event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE) as u16;
                                cg_keycode_to_macro_key(kc).map(CaptureEvent::KeyPress)
                            }
                            11 => {
                                let kc = event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE) as u16;
                                cg_keycode_to_macro_key(kc).map(CaptureEvent::KeyRelease)
                            }
                            14 => system_keys::capture(event),
                            _ => None,
                        };

                        if let Some(ev) = capture_ev {
                            if let Ok(mut cb) = cb_kb.lock() {
                                let ts = CaptureTimestamp::Hardware(cgevent_hardware_timestamp(event));
                                if matches!(cb(ev, ts), CaptureDecision::Suppress) {
                                    return CaptureDecision::Suppress;
                                }
                            }
                        }
                        CaptureDecision::Passthrough
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
                let scroll = Mutex::new(ScrollAccumulator::default());
                let mouse_port = Arc::new(AtomicPtr::new(std::ptr::null_mut()));
                let recovery_port = Arc::clone(&mouse_port);
                match CGEventTap::new(
                    CGEventTapLocation::Session,
                    CGEventTapPlacement::HeadInsertEventTap,
                    CGEventTapOptions::ListenOnly,
                    vec![
                        CGEventType::MouseMoved,
                        CGEventType::LeftMouseDragged,
                        CGEventType::RightMouseDragged,
                        CGEventType::OtherMouseDragged,
                        CGEventType::LeftMouseDown,
                        CGEventType::LeftMouseUp,
                        CGEventType::RightMouseDown,
                        CGEventType::RightMouseUp,
                        CGEventType::OtherMouseDown,
                        CGEventType::OtherMouseUp,
                        CGEventType::ScrollWheel,
                    ],
                    move |_proxy, ev_type, event| {
                        if recover_tap(ev_type, recovery_port.load(Ordering::Acquire)) {
                            return CallbackResult::Replace(event.clone());
                        }
                        // Playback must not start hotkeys or re-record itself.
                        if event.get_integer_value_field(EventField::EVENT_SOURCE_USER_DATA) == SYNTHETIC_EVENT_TAG {
                            return CallbackResult::Replace(event.clone());
                        }
                        let capture_ev: Option<CaptureEvent> = match ev_type {
                            CGEventType::MouseMoved | CGEventType::LeftMouseDragged
                            | CGEventType::RightMouseDragged | CGEventType::OtherMouseDragged => {
                                Some(captured_movement(event, crate::recording::RECORD_MOUSE_RELATIVE.load(Ordering::Relaxed)))
                            }
                            CGEventType::LeftMouseDown => Some(CaptureEvent::ButtonPress(MacroButton::Left)),
                            CGEventType::LeftMouseUp => Some(CaptureEvent::ButtonRelease(MacroButton::Left)),
                            CGEventType::RightMouseDown => Some(CaptureEvent::ButtonPress(MacroButton::Right)),
                            CGEventType::RightMouseUp => Some(CaptureEvent::ButtonRelease(MacroButton::Right)),
                            CGEventType::OtherMouseDown => Some(CaptureEvent::ButtonPress(captured_mouse_button(event))),
                            CGEventType::OtherMouseUp => Some(CaptureEvent::ButtonRelease(captured_mouse_button(event))),
                            CGEventType::ScrollWheel => {
                                if let (Ok(mut scroll), Ok(mut cb)) = (scroll.lock(), cb_mouse.lock()) {
                                    let (h, v) = scroll.read(event);
                                    let ts = CaptureTimestamp::Hardware(cgevent_hardware_timestamp(event));
                                    // Separate events preserve both axes in the recorder's one-axis Scroll block.
                                    if v != 0 { cb(CaptureEvent::Scroll(0, v), ts); }
                                    if h != 0 { cb(CaptureEvent::Scroll(h, 0), ts); }
                                }
                                None
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
                        mouse_port.store(tap.mach_port().as_concrete_TypeRef(), Ordering::Release);
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
        36 | 76 => MacroKey::Return,
        114 => MacroKey::Insert,
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
        54 | 55 => MacroKey::Meta,
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
        105 => MacroKey::F13,
        107 => MacroKey::F14,
        113 => MacroKey::F15,
        106 => MacroKey::F16,
        64 => MacroKey::F17,
        79 => MacroKey::F18,
        80 => MacroKey::F19,
        90 => MacroKey::F20,
        82 => MacroKey::Numpad0,
        83 => MacroKey::Numpad1,
        84 => MacroKey::Numpad2,
        85 => MacroKey::Numpad3,
        86 => MacroKey::Numpad4,
        87 => MacroKey::Numpad5,
        88 => MacroKey::Numpad6,
        89 => MacroKey::Numpad7,
        91 => MacroKey::Numpad8,
        92 => MacroKey::Numpad9,
        69 => MacroKey::Add,
        78 => MacroKey::Subtract,
        67 => MacroKey::Multiply,
        75 => MacroKey::Divide,
        65 => MacroKey::Decimal,
        71 => MacroKey::NumLock,
        110 => MacroKey::LMenu,
        72 => MacroKey::VolumeUp,
        73 => MacroKey::VolumeDown,
        74 => MacroKey::VolumeMute,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn media_events_preserve_key_direction_and_modifiers() {
        for key in [
            MacroKey::VolumeUp,
            MacroKey::VolumeDown,
            MacroKey::VolumeMute,
        ] {
            for down in [true, false] {
                let event = system_keys::event(
                    system_keys::code(&key).unwrap(),
                    down,
                    CGEventFlags::CGEventFlagShift,
                )
                .unwrap();
                let captured = system_keys::capture(&event).expect("media event");
                match captured {
                    CaptureEvent::KeyPress(actual) if down => assert_eq!(actual, key),
                    CaptureEvent::KeyRelease(actual) if !down => assert_eq!(actual, key),
                    _ => panic!("wrong media event direction"),
                }
                assert_eq!(event.get_flags(), CGEventFlags::CGEventFlagShift);
            }
        }
    }

    #[test]
    #[ignore = "dispatches and suppresses media events before they change system volume"]
    fn live_media_keys_are_dispatched_and_captured() {
        use std::sync::{Arc, Mutex};
        let received = Arc::new(Mutex::new(Vec::new()));
        let captured = Arc::clone(&received);
        // Install the suppressing tap before posting any volume event.
        let tap = system_keys::KeyTap::new(move |kind, event| {
            if kind == 14
                && event.get_integer_value_field(EventField::EVENT_SOURCE_USER_DATA)
                    == SYNTHETIC_EVENT_TAG
            {
                if let Some(event) = system_keys::capture(event) {
                    if let Ok(mut received) = captured.lock() {
                        received.push(event);
                    }
                }
                return CaptureDecision::Suppress;
            }
            CaptureDecision::Passthrough
        })
        .expect("media test requires Accessibility access");
        let source = tap.mach_port().create_runloop_source(0).unwrap();
        let run_loop = CFRunLoop::get_current();
        run_loop.add_source(&source, unsafe { kCFRunLoopDefaultMode });
        tap.enable();
        let mut backend = MacosBackend::new().unwrap();
        for key in [
            MacroKey::VolumeUp,
            MacroKey::VolumeDown,
            MacroKey::VolumeMute,
        ] {
            backend.key(key, Direction::Click).unwrap();
        }
        CFRunLoop::run_in_mode(
            unsafe { kCFRunLoopDefaultMode },
            std::time::Duration::from_millis(500),
            false,
        );
        run_loop.remove_source(&source, unsafe { kCFRunLoopDefaultMode });
        let received = received.lock().unwrap();
        assert_eq!(
            received.len(),
            6,
            "each media click must deliver down and up"
        );
        for (pair, expected) in received.chunks_exact(2).zip([
            MacroKey::VolumeUp,
            MacroKey::VolumeDown,
            MacroKey::VolumeMute,
        ]) {
            assert!(matches!(&pair[0], CaptureEvent::KeyPress(key) if *key == expected));
            assert!(matches!(&pair[1], CaptureEvent::KeyRelease(key) if *key == expected));
        }
    }

    #[test]
    fn extended_keys_round_trip_without_becoming_unknown_codes() {
        for code in [
            64, 65, 67, 69, 71, 72, 73, 74, 75, 78, 79, 80, 82, 83, 84, 85, 86, 87, 88, 89, 90, 91,
            92, 105, 106, 107, 110, 113, 114,
        ] {
            let key = cg_keycode_to_macro_key(code).unwrap();
            assert!(!matches!(key, MacroKey::Other(_)), "code {code}");
            assert_eq!(macro_key_to_cgkeycode(&key), Some((code, false)));
        }
        assert_eq!(macro_key_to_cgkeycode(&MacroKey::Other(65536)), None);
    }

    #[test]
    fn unicode_events_preserve_non_us_text_and_surrogate_pairs() {
        unsafe extern "C" {
            fn CGEventKeyboardGetUnicodeString(
                event: core_graphics::sys::CGEventRef,
                max: usize,
                actual: *mut usize,
                text: *mut u16,
            );
        }
        let backend = MacosBackend::new().unwrap();
        for text in ["é", "中", "🦀", "A", "\n", "\t"] {
            for down in [true, false] {
                let event = backend.unicode_event(text, down).unwrap();
                let mut utf16 = [0; 8];
                let mut count = 0;
                unsafe {
                    CGEventKeyboardGetUnicodeString(
                        event.as_ptr(),
                        utf16.len(),
                        &mut count,
                        utf16.as_mut_ptr(),
                    );
                }
                assert_eq!(String::from_utf16(&utf16[..count]).unwrap(), text);
                assert_eq!(event.get_flags(), CGEventFlags::empty());
                assert_eq!(
                    event.get_type() as u32,
                    if down {
                        CGEventType::KeyDown as u32
                    } else {
                        CGEventType::KeyUp as u32
                    }
                );
            }
        }
    }

    #[test]
    fn right_modifier_release_is_seen_while_left_is_still_held() {
        let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState).unwrap();
        let event = CGEvent::new_keyboard_event(source, 60, false).unwrap();
        event.set_flags(CGEventFlags::from_bits_retain(
            CGEventFlags::CGEventFlagShift.bits() | 0x0002,
        ));
        assert!(matches!(
            modifier_event(&event),
            Some(CaptureEvent::KeyRelease(MacroKey::RShift))
        ));
        event.set_flags(CGEventFlags::from_bits_retain(
            CGEventFlags::CGEventFlagShift.bits() | 0x0002 | 0x0004,
        ));
        assert!(matches!(
            modifier_event(&event),
            Some(CaptureEvent::KeyPress(MacroKey::RShift))
        ));
    }

    #[test]
    fn function_key_flags_do_not_leak_into_later_letters() {
        let stale = CGEventFlags::CGEventFlagSecondaryFn
            | CGEventFlags::CGEventFlagNumericPad
            | CGEventFlags::CGEventFlagHelp
            | CGEventFlags::CGEventFlagShift;
        let flags = keyboard_flags(8, stale, false);
        assert_eq!(flags, CGEventFlags::CGEventFlagShift);
        assert!(keyboard_flags(8, stale, true).contains(CGEventFlags::CGEventFlagSecondaryFn));
        assert!(
            keyboard_flags(79, CGEventFlags::empty(), false)
                .contains(CGEventFlags::CGEventFlagSecondaryFn)
        );
    }

    #[test]
    fn queued_modifier_flags_do_not_depend_on_os_delivery_timing() {
        let mut backend = MacosBackend::new().unwrap();
        backend.keyboard_event(56, true).unwrap();
        assert!(
            backend
                .keyboard_event(0, true)
                .unwrap()
                .get_flags()
                .contains(CGEventFlags::CGEventFlagShift)
        );
        backend.keyboard_event(60, true).unwrap();
        backend.keyboard_event(56, false).unwrap();
        assert!(
            backend
                .keyboard_event(11, true)
                .unwrap()
                .get_flags()
                .contains(CGEventFlags::CGEventFlagShift)
        );
        backend.keyboard_event(60, false).unwrap();
        assert!(
            !backend
                .keyboard_event(8, true)
                .unwrap()
                .get_flags()
                .contains(CGEventFlags::CGEventFlagShift)
        );
    }

    #[test]
    fn fractional_scroll_accumulates_both_axes_in_line_units() {
        let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState).unwrap();
        let event = CGEvent::new(source).unwrap();
        event.set_type(CGEventType::ScrollWheel);
        event.set_double_value_field(
            EventField::SCROLL_WHEEL_EVENT_FIXED_POINT_DELTA_AXIS_1,
            0.25,
        );
        event.set_double_value_field(
            EventField::SCROLL_WHEEL_EVENT_FIXED_POINT_DELTA_AXIS_2,
            -0.5,
        );
        let mut scroll = ScrollAccumulator::default();
        assert_eq!(scroll.read(&event), (0, 0));
        assert_eq!(scroll.read(&event), (-1, 0));
        assert_eq!(scroll.read(&event), (0, 0));
        assert_eq!(scroll.read(&event), (-1, 1));
    }

    #[test]
    fn native_scroll_pixels_convert_to_lines_without_tenfold_playback() {
        let backend = MacosBackend::new().unwrap();
        for (units, amount) in [(0, 10), (1, 1)] {
            let raw = unsafe {
                CGEventCreateScrollWheelEvent2(backend.source.as_ptr(), units, 1, amount, 0, 0)
            };
            assert!(!raw.is_null());
            let event = unsafe { CGEvent::from_ptr(raw) };
            assert_eq!(ScrollAccumulator::default().read(&event), (0, 1));
        }
    }

    #[test]
    fn dragging_uses_the_held_button_and_absolute_target() {
        let mut backend = MacosBackend::new().unwrap();
        for (mask, kind, button) in [
            (0, CGEventType::MouseMoved, 0),
            (1, CGEventType::LeftMouseDragged, 0),
            (2, CGEventType::RightMouseDragged, 1),
            (8, CGEventType::OtherMouseDragged, 3),
        ] {
            backend.held_buttons = mask;
            let event = backend.movement_event(-20, 300, -100, 42).unwrap();
            assert_eq!(event.get_type() as u32, kind as u32);
            assert_eq!(
                event.get_integer_value_field(EventField::MOUSE_EVENT_BUTTON_NUMBER),
                button
            );
            assert_eq!((event.location().x, event.location().y), (-20.0, 300.0));
            assert_eq!(
                event.get_double_value_field(EventField::MOUSE_EVENT_DELTA_X),
                -100.0
            );
        }
    }

    #[test]
    #[ignore = "creates and disables a real macOS event tap"]
    fn live_disabled_event_tap_recovers() {
        use core_foundation::base::TCFType;
        let tap = CGEventTap::new(
            CGEventTapLocation::HID,
            CGEventTapPlacement::HeadInsertEventTap,
            CGEventTapOptions::ListenOnly,
            vec![CGEventType::KeyDown],
            |_, _, event| CallbackResult::Replace(event.clone()),
        )
        .expect("event tap needs Accessibility access");
        let port = tap.mach_port().as_concrete_TypeRef();
        unsafe {
            CGEventTapEnable(port, false);
        }
        assert!(!unsafe { CGEventTapIsEnabled(port) });
        assert!(recover_tap(CGEventType::TapDisabledByTimeout, port));
        assert!(unsafe { CGEventTapIsEnabled(port) });
    }

    #[test]
    #[ignore = "installs a real macOS event tap and suppresses an F18 probe"]
    fn live_capture_records_and_suppresses_a_keyboard_probe() {
        let title = std::env::var("BW_LIVE_WINDOW").expect("set BW_LIVE_WINDOW");
        let path = std::env::var("BLOCKWORK_TEST_EVENT_LOG").expect("set BLOCKWORK_TEST_EVENT_LOG");
        crate::window::act(
            crate::window::Action::Focus,
            &title,
            crate::window::MatchBy::Title,
        )
        .unwrap();
        let offset = std::fs::read(&path).unwrap().len();
        let (send, receive) = std::sync::mpsc::channel();
        start_capture_thread(Box::new(move |event, timestamp| {
            let down = match event {
                CaptureEvent::KeyPress(MacroKey::F18) => true,
                CaptureEvent::KeyRelease(MacroKey::F18) => false,
                _ => return CaptureDecision::Passthrough,
            };
            if send.send((down, timestamp)).is_ok() {
                CaptureDecision::Suppress
            } else {
                CaptureDecision::Passthrough
            }
        }));
        std::thread::sleep(std::time::Duration::from_millis(300));
        let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState).unwrap();
        // Untagged probes go through the real tap, unlike Blockwork playback.
        for down in [true, false] {
            let event = CGEvent::new_keyboard_event(source.clone(), 79, down).unwrap();
            event.post(CGEventTapLocation::HID);
        }
        let (down, first) = receive
            .recv_timeout(std::time::Duration::from_secs(2))
            .expect("keyboard tap needs Accessibility access");
        let (up, last) = receive
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap();
        assert!(down && !up);
        let (CaptureTimestamp::Hardware(first), CaptureTimestamp::Hardware(last)) = (first, last)
        else {
            panic!("macOS must preserve hardware timestamps")
        };
        assert!(last.duration_since(first).is_ok());
        let probe_count = || {
            let log = std::fs::read_to_string(&path).unwrap();
            log[offset..]
                .lines()
                .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
                .filter(|row| row["keyCode"] == 79)
                .count()
        };
        std::thread::sleep(std::time::Duration::from_millis(50));
        assert_eq!(
            probe_count(),
            0,
            "suppressed input must not reach the fixture"
        );
        let mut backend = MacosBackend::new().unwrap();
        backend.raw_keycode(79, Direction::Click).unwrap();
        assert!(
            receive
                .recv_timeout(std::time::Duration::from_millis(150))
                .is_err(),
            "Blockwork playback must not feed back into capture"
        );
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while probe_count() < 2 && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert_eq!(
            probe_count(),
            2,
            "tagged playback must still reach the fixture"
        );
    }

    #[test]
    fn extra_mouse_buttons_keep_their_identity() {
        let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState).unwrap();
        for button in [
            MacroButton::Middle,
            MacroButton::Back,
            MacroButton::Forward,
            MacroButton::Other(9),
        ] {
            let number = mouse_button_number(&button).unwrap();
            let event = CGEvent::new_mouse_event(
                source.clone(),
                CGEventType::OtherMouseDown,
                CGPoint::new(0.0, 0.0),
                CGMouseButton::Center,
            )
            .unwrap();
            event.set_integer_value_field(EventField::MOUSE_EVENT_BUTTON_NUMBER, number as i64);
            assert_eq!(captured_mouse_button(&event), button);
        }
        assert!(mouse_button_number(&MacroButton::Other(32)).is_err());
    }

    #[test]
    fn queued_cursor_waits_for_every_move_and_resyncs_with_physical_input() {
        let mut cursor = QueuedCursor::default();
        assert_eq!(cursor.resolve((294, 1158), [10, 20, 30, 40]), (294, 1158));
        cursor.enqueue((260, 1158), CGEventType::MouseMoved);
        assert_eq!(cursor.resolve((294, 1158), [10, 20, 30, 40]), (260, 1158));
        cursor.enqueue((234, 1158), CGEventType::MouseMoved);
        cursor.enqueue((294, 1158), CGEventType::LeftMouseDragged);
        // Returning to the starting location must not acknowledge queued moves.
        assert_eq!(cursor.resolve((294, 1158), [10, 20, 30, 40]), (294, 1158));
        assert!(cursor.target.is_some());
        assert_eq!(cursor.resolve((260, 1158), [11, 20, 30, 40]), (294, 1158));
        assert_eq!(cursor.resolve((234, 1158), [12, 20, 30, 40]), (294, 1158));
        assert_eq!(cursor.resolve((294, 1158), [12, 21, 30, 40]), (294, 1158));
        assert!(cursor.target.is_none());
        assert_eq!(cursor.resolve((600, 800), [13, 21, 30, 40]), (600, 800));
        // WindowServer counters wrap without losing the pending move.
        cursor.resolve((600, 800), [u32::MAX, 21, 30, 40]);
        cursor.enqueue((610, 810), CGEventType::MouseMoved);
        assert_eq!(
            cursor.resolve((600, 800), [u32::MAX, 21, 30, 40]),
            (610, 810)
        );
        assert_eq!(cursor.resolve((610, 810), [0, 21, 30, 40]), (610, 810));
        assert!(cursor.target.is_none());
    }

    #[test]
    fn capture_preserves_event_positions_and_raw_relative_deltas() {
        let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState).unwrap();
        let event = CGEvent::new_mouse_event(
            source,
            CGEventType::LeftMouseDragged,
            CGPoint::new(543.0, 654.0),
            CGMouseButton::Left,
        )
        .unwrap();
        event.set_double_value_field(EventField::MOUSE_EVENT_DELTA_X, 10.0);
        event.set_double_value_field(EventField::MOUSE_EVENT_DELTA_Y, -20.0);
        assert!(matches!(
            captured_movement(&event, false),
            CaptureEvent::MouseMoveAbs(543.0, 654.0)
        ));
        assert!(matches!(
            captured_movement(&event, true),
            CaptureEvent::MouseMoveRel(10, -20)
        ));
    }
    #[test]
    #[ignore = "posts input to scripts/macos/test-window.swift"]
    fn live_mouse_queue_preserves_rapid_relative_moves_clicks_drags_and_scroll() {
        let title = std::env::var("BW_LIVE_WINDOW").unwrap();
        assert_eq!(title, "Blockwork macOS Test Window");
        let path = std::env::var("BLOCKWORK_TEST_EVENT_LOG").unwrap();
        crate::window::act(
            crate::window::Action::Focus,
            &title,
            crate::window::MatchBy::Title,
        )
        .unwrap();
        let rect = crate::window::bounds(&title, crate::window::MatchBy::Title).unwrap();
        let x = rect.x as i32 + 60;
        let y = rect.y as i32 + 350;
        let offset = std::fs::read(&path).unwrap().len();
        let mut backend = MacosBackend::new().unwrap();
        backend.move_mouse_abs(x, y).unwrap();
        backend.move_mouse_rel(10, 0).unwrap();
        backend.move_mouse_rel(20, 0).unwrap();
        backend.button(MacroButton::Left, Direction::Press).unwrap();
        backend.move_mouse_rel(60, 0).unwrap();
        backend
            .button(MacroButton::Left, Direction::Release)
            .unwrap();
        backend.scroll(-2, Axis::Vertical).unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        loop {
            let log = std::fs::read_to_string(&path).unwrap();
            let rows: Vec<serde_json::Value> = log[offset..]
                .lines()
                .filter_map(|line| serde_json::from_str(line).ok())
                .collect();
            if rows.iter().any(|row| row["type"] == 22) {
                let events: Vec<_> = rows
                    .iter()
                    .filter(|row| matches!(row["type"].as_u64(), Some(1 | 2 | 6 | 22)))
                    .map(|row| {
                        (
                            row["type"].as_i64().unwrap(),
                            row["x"].as_f64().unwrap() as i32,
                            row["y"].as_f64().unwrap() as i32,
                            row["dx"].as_f64().unwrap() as i32,
                        )
                    })
                    .collect();
                assert_eq!(
                    events,
                    vec![
                        (1, x + 30, y, 0),
                        (6, x + 90, y, 60),
                        (2, x + 90, y, 0),
                        (22, x + 90, y, 0)
                    ]
                );
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "fixture did not receive the gesture: {rows:?}"
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        // After delivery, a physical move must replace the queued position.
        let point = CGPoint::new((x + 100) as f64, y as f64);
        let external = CGEvent::new_mouse_event(
            backend.source.clone(),
            CGEventType::MouseMoved,
            point,
            CGMouseButton::Left,
        )
        .unwrap();
        external.post(CGEventTapLocation::HID);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        while backend.cursor_pos() != Some((x + 100, y)) {
            assert!(
                std::time::Instant::now() < deadline,
                "cursor did not resync"
            );
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    #[test]
    #[ignore = "posts input to scripts/macos/test-window.swift"]
    fn live_keyboard_playback_preserves_text_and_modifier_order() {
        let title = std::env::var("BW_LIVE_WINDOW")
            .expect("set BW_LIVE_WINDOW to the native fixture title");
        let path = std::env::var("BLOCKWORK_TEST_EVENT_LOG")
            .expect("set BLOCKWORK_TEST_EVENT_LOG to the fixture log");
        crate::window::act(
            crate::window::Action::Focus,
            &title,
            crate::window::MatchBy::Title,
        )
        .unwrap();
        assert!(
            crate::window::active()
                .unwrap()
                .matches(&title, crate::window::MatchBy::Title)
        );
        let offset = std::fs::read(&path).unwrap().len();
        let mut backend = MacosBackend::new().unwrap();
        backend.key(MacroKey::LShift, Direction::Press).unwrap();
        backend
            .key(MacroKey::Unicode('a'), Direction::Click)
            .unwrap();
        backend.key(MacroKey::RShift, Direction::Press).unwrap();
        backend.key(MacroKey::LShift, Direction::Release).unwrap();
        backend
            .key(MacroKey::Unicode('b'), Direction::Click)
            .unwrap();
        backend.key(MacroKey::RShift, Direction::Release).unwrap();
        backend
            .key(MacroKey::Unicode('c'), Direction::Click)
            .unwrap();
        backend.text("é中🦀").unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        loop {
            let log = std::fs::read_to_string(&path).unwrap();
            let typed: String = log[offset..]
                .lines()
                .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
                .filter(|row| row["type"] == 10)
                .filter_map(|row| row["characters"].as_str().map(str::to_owned))
                .collect();
            if typed.chars().count() >= 6 || std::time::Instant::now() >= deadline {
                assert_eq!(typed, "ABcé中🦀");
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    }
}
