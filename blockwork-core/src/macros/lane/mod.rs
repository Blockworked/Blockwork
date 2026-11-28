//! Input aimed at one window without moving the desktop's focus, for the
//! `InWindow` block. A `Lane` is one platform's way in; `LaneBackend` wraps
//! it as an `InputBackend` so every input block works inside the block
//! unchanged. See docs/background-input.md.

use crate::input::types::{Axis, Direction, MacroButton, MacroKey};
use crate::macros::backend::InputBackend;
use crate::window::WindowRef;

#[cfg(target_os = "linux")]
mod kde;

/// One extra input seat aimed at one window. Positions are logical pixels
/// from the top left of the window's content. Dropping it lets go of every
/// key and button it holds.
pub trait Lane: Send {
    /// Presses or releases an evdev key code.
    fn key(&mut self, code: u16, pressed: bool) -> Result<(), String>;
    /// Presses or releases an evdev button code.
    fn button(&mut self, code: u16, pressed: bool) -> Result<(), String>;
    fn motion(&mut self, x: f64, y: f64) -> Result<(), String>;
    /// Moves by a delta: relative motion when the app has locked the pointer.
    fn relative(&mut self, dx: f64, dy: f64) -> Result<(), String>;
    /// Scrolls; positive is down or right, 120 per wheel notch.
    fn axis(&mut self, axis: Axis, value120: i32) -> Result<(), String>;
    /// Types text whatever the user's layout is.
    fn text(&mut self, text: &str) -> Result<(), String>;
    fn release_all(&mut self) -> Result<(), String>;
    /// Where the lane's pointer is, in window coordinates.
    fn pointer(&self) -> (f64, f64);
}

/// Aims a new lane at `window`, or says why background input can't reach it.
pub fn open(window: &WindowRef) -> Result<Box<dyn Lane>, String> {
    #[cfg(target_os = "linux")]
    {
        if window.desktop == crate::window::DesktopKind::Kde {
            return kde::open(window).map(|lane| Box::new(lane) as Box<dyn Lane>);
        }
    }
    let _ = window;
    Err(unsupported())
}

/// Whether this desktop can send background input at all, and why not.
pub fn availability() -> Result<String, String> {
    #[cfg(target_os = "linux")]
    {
        if crate::window::desktop_kind() == Some(crate::window::DesktopKind::Kde) {
            return kde::availability();
        }
    }
    Err(unsupported())
}

fn unsupported() -> String {
    "background input only works on KDE Plasma (Wayland) with the Blockwork KWin plugin: pick \"brought to front\" in the In window block to use it here".into()
}

/// A lane as the strand's input backend. Screen positions from the blocks
/// are moved into the window by the strand's origin (the window's top left,
/// set by `InWindow`).
pub struct LaneBackend {
    lane: Box<dyn Lane>,
}

impl LaneBackend {
    pub fn new(lane: Box<dyn Lane>) -> Self {
        LaneBackend { lane }
    }

    fn local(x: f64, y: f64) -> Result<(f64, f64), String> {
        let (ox, oy) = crate::screen::origin()?;
        Ok((x - ox, y - oy))
    }

    /// Press, then release, with the same short hold the desktop backend uses.
    fn click(
        &mut self,
        press: impl Fn(&mut dyn Lane, bool) -> Result<(), String>,
    ) -> Result<(), String> {
        press(self.lane.as_mut(), true)?;
        std::thread::sleep(CLICK_HOLD);
        press(self.lane.as_mut(), false)
    }
}

use crate::macros::backend::CLICK_HOLD;

#[cfg(target_os = "linux")]
fn key_code(key: &MacroKey) -> Result<(u16, bool), String> {
    crate::macros::backend::evdev_mapping::macro_key_to_evdev(key)
        .map(|(code, shift)| (code.0, shift))
        .ok_or_else(|| format!("no key code for {key:?}"))
}

#[cfg(not(target_os = "linux"))]
fn key_code(key: &MacroKey) -> Result<(u16, bool), String> {
    Err(format!("no key code for {key:?}"))
}

#[cfg(target_os = "linux")]
fn button_code(button: &MacroButton) -> Result<u16, String> {
    crate::macros::backend::evdev_mapping::macro_button_to_evdev(button)
        .map(|code| code.0)
        .ok_or_else(|| format!("no button code for {button:?}"))
}

#[cfg(not(target_os = "linux"))]
fn button_code(button: &MacroButton) -> Result<u16, String> {
    Err(format!("no button code for {button:?}"))
}

/// evdev KEY_LEFTSHIFT.
const LEFT_SHIFT: u16 = 42;

impl InputBackend for LaneBackend {
    fn key(&mut self, key: MacroKey, dir: Direction) -> Result<(), String> {
        let (code, shift) = key_code(&key)?;
        match dir {
            Direction::Press => {
                if shift {
                    self.lane.key(LEFT_SHIFT, true)?;
                }
                self.lane.key(code, true)
            }
            Direction::Release => {
                let r = self.lane.key(code, false);
                if shift {
                    self.lane.key(LEFT_SHIFT, false)?;
                }
                r
            }
            Direction::Click => {
                if shift {
                    self.lane.key(LEFT_SHIFT, true)?;
                }
                let r = self.click(|lane, down| lane.key(code, down));
                if shift {
                    self.lane.key(LEFT_SHIFT, false)?;
                }
                r
            }
        }
    }

    fn raw_keycode(&mut self, keycode: u16, dir: Direction) -> Result<(), String> {
        match dir {
            Direction::Press => self.lane.key(keycode, true),
            Direction::Release => self.lane.key(keycode, false),
            Direction::Click => self.click(|lane, down| lane.key(keycode, down)),
        }
    }

    fn button(&mut self, button: MacroButton, dir: Direction) -> Result<(), String> {
        let notch = match button {
            MacroButton::ScrollUp => Some((Axis::Vertical, -120)),
            MacroButton::ScrollDown => Some((Axis::Vertical, 120)),
            MacroButton::ScrollLeft => Some((Axis::Horizontal, -120)),
            MacroButton::ScrollRight => Some((Axis::Horizontal, 120)),
            _ => None,
        };
        if let Some((axis, value)) = notch {
            return self.lane.axis(axis, value);
        }
        let code = button_code(&button)?;
        match dir {
            Direction::Press => self.lane.button(code, true),
            Direction::Release => self.lane.button(code, false),
            Direction::Click => self.click(|lane, down| lane.button(code, down)),
        }
    }

    fn move_mouse_rel(&mut self, dx: i32, dy: i32) -> Result<(), String> {
        self.lane.relative(dx as f64, dy as f64)
    }

    fn move_mouse_abs(&mut self, x: i32, y: i32) -> Result<(), String> {
        let (x, y) = Self::local(x as f64, y as f64)?;
        self.lane.motion(x, y)
    }

    fn scroll(&mut self, amount: i32, axis: Axis) -> Result<(), String> {
        // Blockwork's positive vertical amount is up (like REL_WHEEL);
        // Wayland's positive axis is down.
        let value = match axis {
            Axis::Vertical => -amount * 120,
            Axis::Horizontal => amount * 120,
        };
        self.lane.axis(axis, value)
    }

    fn text(&mut self, s: &str) -> Result<(), String> {
        self.lane.text(s)
    }

    fn cursor_pos(&self) -> Option<(i32, i32)> {
        let (ox, oy) = crate::screen::origin().ok()?;
        let (x, y) = self.lane.pointer();
        Some(((x + ox).round() as i32, (y + oy).round() as i32))
    }

    fn unavailable_reason(&self) -> Option<String> {
        None
    }
}

impl Drop for LaneBackend {
    fn drop(&mut self) {
        let _ = self.lane.release_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    struct Log(Arc<Mutex<Vec<String>>>);

    impl Lane for Log {
        fn key(&mut self, code: u16, pressed: bool) -> Result<(), String> {
            self.0.lock().unwrap().push(format!("key {code} {pressed}"));
            Ok(())
        }
        fn button(&mut self, code: u16, pressed: bool) -> Result<(), String> {
            self.0
                .lock()
                .unwrap()
                .push(format!("button {code} {pressed}"));
            Ok(())
        }
        fn motion(&mut self, x: f64, y: f64) -> Result<(), String> {
            self.0.lock().unwrap().push(format!("motion {x} {y}"));
            Ok(())
        }
        fn relative(&mut self, dx: f64, dy: f64) -> Result<(), String> {
            self.0.lock().unwrap().push(format!("relative {dx} {dy}"));
            Ok(())
        }
        fn axis(&mut self, axis: Axis, value120: i32) -> Result<(), String> {
            self.0
                .lock()
                .unwrap()
                .push(format!("axis {axis:?} {value120}"));
            Ok(())
        }
        fn text(&mut self, text: &str) -> Result<(), String> {
            self.0.lock().unwrap().push(format!("text {text}"));
            Ok(())
        }
        fn release_all(&mut self) -> Result<(), String> {
            self.0.lock().unwrap().push("release_all".into());
            Ok(())
        }
        fn pointer(&self) -> (f64, f64) {
            (0.0, 0.0)
        }
    }

    #[test]
    fn scrolling_up_is_a_negative_wayland_axis() {
        let log = Arc::new(Mutex::new(Vec::new()));
        let mut backend = LaneBackend::new(Box::new(Log(Arc::clone(&log))));
        backend.scroll(2, Axis::Vertical).unwrap();
        backend
            .button(MacroButton::ScrollRight, Direction::Click)
            .unwrap();
        drop(backend);
        assert_eq!(
            *log.lock().unwrap(),
            ["axis Vertical -240", "axis Horizontal 120", "release_all"]
        );
    }

    #[test]
    fn positions_count_from_the_strand_origin() {
        let log = Arc::new(Mutex::new(Vec::new()));
        let mut backend = LaneBackend::new(Box::new(Log(Arc::clone(&log))));
        // No UseCoordinates/InWindow origin: screen and window agree.
        backend.move_mouse_abs(30, 40).unwrap();
        assert_eq!(log.lock().unwrap()[0], "motion 30 40");
    }
}
