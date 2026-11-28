//! Background input on macOS: events posted to the window's process
//! (CGEventPostToPid), with mouse events naming the window, so the frontmost
//! app, the real cursor and the user's modifier keys stay as they are.

use super::Lane;
use crate::input::types::Axis;
use crate::window::WindowRef;
use core_graphics::event::{
    CGEvent, CGEventFlags, CGEventType, CGMouseButton, EventField, ScrollEventUnit,
};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use core_graphics::geometry::CGPoint;

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXIsProcessTrusted() -> bool;
}

const NOT_TRUSTED: &str = "background input needs Accessibility access: allow Blockwork in System Settings > Privacy & Security > Accessibility";

pub(super) fn availability() -> Result<String, String> {
    if !unsafe { AXIsProcessTrusted() } {
        return Err(NOT_TRUSTED.into());
    }
    Ok("ready: input is posted to the window's app".into())
}

pub(super) fn open(window: &WindowRef) -> Result<MacLane, String> {
    if !unsafe { AXIsProcessTrusted() } {
        return Err(NOT_TRUSTED.into());
    }
    let (pid, number) = crate::window::macos_window(window)?;
    Ok(MacLane {
        window: window.clone(),
        pid,
        number,
        pointer: None,
        keys: Vec::new(),
        buttons: Vec::new(),
    })
}

pub(super) struct MacLane {
    window: WindowRef,
    pid: i32,
    number: u32,
    pointer: Option<(f64, f64)>,
    keys: Vec<u16>,
    buttons: Vec<u16>,
}

/// The modifier flag a modifier key's CGKeyCode sets.
fn modifier_flag(code: u16) -> Option<CGEventFlags> {
    Some(match code {
        55 | 54 => CGEventFlags::CGEventFlagCommand,
        56 | 60 => CGEventFlags::CGEventFlagShift,
        59 | 62 => CGEventFlags::CGEventFlagControl,
        58 | 61 => CGEventFlags::CGEventFlagAlternate,
        63 => CGEventFlags::CGEventFlagSecondaryFn,
        _ => return None,
    })
}

/// A private source, so the user's real modifier keys don't mix in.
fn source() -> Result<CGEventSource, String> {
    CGEventSource::new(CGEventSourceStateID::Private)
        .map_err(|_| "macOS couldn't make an input event".to_string())
}

impl MacLane {
    fn flags(&self) -> CGEventFlags {
        self.keys
            .iter()
            .filter_map(|k| modifier_flag(*k))
            .fold(CGEventFlags::CGEventFlagNull, |all, f| all | f)
    }

    fn origin(&self) -> Result<(f64, f64), String> {
        let bounds = crate::window::bounds_of(&self.window)?;
        Ok((bounds.x, bounds.y))
    }

    /// Where the pointer is; the middle of the window until it's moved.
    fn position(&self) -> (f64, f64) {
        self.pointer.unwrap_or_else(|| {
            crate::window::bounds_of(&self.window)
                .map(|b| (b.width / 2.0, b.height / 2.0))
                .unwrap_or((0.0, 0.0))
        })
    }

    fn send(&self, event: CGEvent) {
        event.post_to_pid(self.pid);
    }

    /// A mouse event at the pointer, addressed to the window.
    fn mouse(&self, kind: CGEventType, button: CGMouseButton) -> Result<CGEvent, String> {
        let (ox, oy) = self.origin()?;
        let (x, y) = self.position();
        let event = CGEvent::new_mouse_event(source()?, kind, CGPoint::new(ox + x, oy + y), button)
            .map_err(|_| "macOS couldn't make a mouse event".to_string())?;
        event.set_integer_value_field(
            EventField::MOUSE_EVENT_WINDOW_UNDER_MOUSE_POINTER,
            self.number as i64,
        );
        event.set_integer_value_field(
            EventField::MOUSE_EVENT_WINDOW_UNDER_MOUSE_POINTER_THAT_CAN_HANDLE_THIS_EVENT,
            self.number as i64,
        );
        event.set_flags(self.flags());
        Ok(event)
    }
}

fn cg_button(code: u16) -> Result<(CGMouseButton, CGEventType, CGEventType, CGEventType), String> {
    Ok(match code {
        0 => (
            CGMouseButton::Left,
            CGEventType::LeftMouseDown,
            CGEventType::LeftMouseUp,
            CGEventType::LeftMouseDragged,
        ),
        1 => (
            CGMouseButton::Right,
            CGEventType::RightMouseDown,
            CGEventType::RightMouseUp,
            CGEventType::RightMouseDragged,
        ),
        2..=4 => (
            CGMouseButton::Center,
            CGEventType::OtherMouseDown,
            CGEventType::OtherMouseUp,
            CGEventType::OtherMouseDragged,
        ),
        _ => return Err(format!("mouse button {code} can't be sent to a window")),
    })
}

impl Lane for MacLane {
    fn key(&mut self, code: u16, pressed: bool) -> Result<(), String> {
        if pressed == self.keys.contains(&code) {
            return Ok(());
        }
        if pressed {
            self.keys.push(code);
        } else {
            self.keys.retain(|k| *k != code);
        }
        let event = CGEvent::new_keyboard_event(source()?, code, pressed)
            .map_err(|_| "macOS couldn't make a key event".to_string())?;
        if modifier_flag(code).is_some() {
            event.set_type(CGEventType::FlagsChanged);
        }
        event.set_flags(self.flags());
        self.send(event);
        Ok(())
    }

    fn button(&mut self, code: u16, pressed: bool) -> Result<(), String> {
        if pressed == self.buttons.contains(&code) {
            return Ok(());
        }
        if pressed {
            self.buttons.push(code);
        } else {
            self.buttons.retain(|b| *b != code);
        }
        let (button, down, up, _) = cg_button(code)?;
        let event = self.mouse(if pressed { down } else { up }, button)?;
        event.set_integer_value_field(EventField::MOUSE_EVENT_CLICK_STATE, 1);
        if code >= 2 {
            event.set_integer_value_field(EventField::MOUSE_EVENT_BUTTON_NUMBER, code as i64);
        }
        self.send(event);
        Ok(())
    }

    fn motion(&mut self, x: f64, y: f64) -> Result<(), String> {
        self.pointer = Some((x, y));
        let event = match self.buttons.first() {
            Some(code) => {
                let (button, _, _, dragged) = cg_button(*code)?;
                self.mouse(dragged, button)?
            }
            None => self.mouse(CGEventType::MouseMoved, CGMouseButton::Left)?,
        };
        self.send(event);
        Ok(())
    }

    fn relative(&mut self, dx: f64, dy: f64) -> Result<(), String> {
        let (x, y) = self.position();
        self.motion(x + dx, y + dy)
    }

    fn axis(&mut self, axis: Axis, value120: i32) -> Result<(), String> {
        // CoreGraphics counts up and left as positive; the lane counts down
        // and right.
        let lines = -value120 / 120;
        let (vertical, horizontal) = match axis {
            Axis::Vertical => (lines, 0),
            Axis::Horizontal => (0, lines),
        };
        let event =
            CGEvent::new_scroll_event(source()?, ScrollEventUnit::LINE, 2, vertical, horizontal, 0)
                .map_err(|_| "macOS couldn't make a scroll event".to_string())?;
        let (ox, oy) = self.origin()?;
        let (x, y) = self.position();
        event.set_location(CGPoint::new(ox + x, oy + y));
        event.set_flags(self.flags());
        self.send(event);
        Ok(())
    }

    fn text(&mut self, text: &str) -> Result<(), String> {
        let mut first = true;
        for c in text.chars() {
            if !first {
                std::thread::sleep(crate::macros::backend::INTER_CHAR_DELAY);
            }
            first = false;
            let c = if c == '\n' { '\r' } else { c };
            for down in [true, false] {
                let event = CGEvent::new_keyboard_event(source()?, 0, down)
                    .map_err(|_| "macOS couldn't make a key event".to_string())?;
                event.set_string(&c.to_string());
                self.send(event);
            }
        }
        Ok(())
    }

    fn release_all(&mut self) -> Result<(), String> {
        for button in self.buttons.clone() {
            let _ = self.button(button, false);
        }
        for key in self.keys.clone() {
            let _ = self.key(key, false);
        }
        Ok(())
    }

    fn pointer(&self) -> (f64, f64) {
        self.position()
    }
}
