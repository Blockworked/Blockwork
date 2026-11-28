//! Background input on Windows: window messages posted straight to the
//! window (the way AutoHotkey's ControlSend and ControlClick work), so the
//! foreground window, the real keyboard and the cursor stay as they are.
//! Apps that read raw input or DirectInput (most games) don't see these.

use super::Lane;
use crate::input::types::{Axis, MacroKey};
use crate::window::WindowRef;
use windows_sys::Win32::Foundation::{GetLastError, HWND, LPARAM, POINT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{ClientToScreen, ScreenToClient};
use windows_sys::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CWP_SKIPDISABLED, CWP_SKIPINVISIBLE, CWP_SKIPTRANSPARENT, ChildWindowFromPointEx,
    GUITHREADINFO, GetGUIThreadInfo, GetWindowThreadProcessId, IsChild, PostMessageW, WM_CHAR,
    WM_KEYDOWN, WM_KEYUP, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDOWN, WM_MBUTTONUP,
    WM_MOUSEHWHEEL, WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_RBUTTONDOWN, WM_RBUTTONUP, WM_SYSKEYDOWN,
    WM_SYSKEYUP, WM_XBUTTONDOWN, WM_XBUTTONUP,
};

// Mouse key-state flags in a mouse message's wParam.
const MK_LBUTTON: usize = 0x1;
const MK_RBUTTON: usize = 0x2;
const MK_SHIFT: usize = 0x4;
const MK_CONTROL: usize = 0x8;
const MK_MBUTTON: usize = 0x10;
const MK_XBUTTON1: usize = 0x20;
const MK_XBUTTON2: usize = 0x40;

pub(super) fn availability() -> Result<String, String> {
    Ok("ready: input is posted to the window; games that read raw input ignore it".into())
}

pub(super) fn open(window: &WindowRef) -> Result<WindowsLane, String> {
    let hwnd = crate::window::windows_hwnd(window)?;
    // SAFETY: a valid window; the process id out-parameter is optional.
    let thread = unsafe { GetWindowThreadProcessId(hwnd, std::ptr::null_mut()) };
    if thread == 0 {
        return Err("the window closed".into());
    }
    Ok(WindowsLane {
        top: hwnd as isize,
        thread,
        pointer: None,
        keys: Vec::new(),
        buttons: Vec::new(),
        saved_state: None,
    })
}

pub(super) struct WindowsLane {
    top: isize,
    /// The window's UI thread, whose key state modifiers are set on.
    thread: u32,
    pointer: Option<(f64, f64)>,
    keys: Vec<u16>,
    buttons: Vec<u16>,
    /// The thread's key state from before the lane held a modifier.
    saved_state: Option<[u8; 256]>,
}

fn is_modifier(vk: u16) -> bool {
    matches!(
        vk,
        VK_SHIFT
            | VK_LSHIFT
            | VK_RSHIFT
            | VK_CONTROL
            | VK_LCONTROL
            | VK_RCONTROL
            | VK_MENU
            | VK_LMENU
            | VK_RMENU
            | VK_LWIN
            | VK_RWIN
    )
}

fn is_extended(vk: u16) -> bool {
    matches!(
        vk,
        VK_RCONTROL
            | VK_RMENU
            | VK_INSERT
            | VK_DELETE
            | VK_HOME
            | VK_END
            | VK_PRIOR
            | VK_NEXT
            | VK_LEFT
            | VK_RIGHT
            | VK_UP
            | VK_DOWN
            | VK_DIVIDE
            | VK_NUMLOCK
            | VK_LWIN
            | VK_RWIN
            | VK_APPS
    )
}

fn post(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> Result<(), String> {
    // SAFETY: posting to a window handle; a stale one only fails.
    if unsafe { PostMessageW(hwnd, message, wparam, lparam) } == 0 {
        // SAFETY: reads this thread's last error.
        return Err(if unsafe { GetLastError() } == 5 {
            "Windows blocked it: the window belongs to a program running as administrator, so run Blockwork as administrator too".into()
        } else {
            "the window closed".into()
        });
    }
    Ok(())
}

fn lparam_point(x: i32, y: i32) -> LPARAM {
    ((y as u16 as u32) << 16 | x as u16 as u32) as i32 as LPARAM
}

impl WindowsLane {
    fn top(&self) -> HWND {
        self.top as HWND
    }

    /// The control that has the window's keyboard focus, which may be a
    /// child (an edit box) rather than the window itself.
    fn focus(&self) -> HWND {
        // SAFETY: GUITHREADINFO is plain data with its size filled in.
        unsafe {
            let mut info: GUITHREADINFO = std::mem::zeroed();
            info.cbSize = size_of::<GUITHREADINFO>() as u32;
            let top = self.top();
            if GetGUIThreadInfo(self.thread, &mut info) != 0
                && !info.hwndFocus.is_null()
                && (info.hwndFocus == top || IsChild(top, info.hwndFocus) != 0)
            {
                return info.hwndFocus;
            }
            top
        }
    }

    fn held(&self, vk: u16) -> bool {
        self.keys.contains(&vk)
    }

    fn alt_held(&self) -> bool {
        self.held(VK_MENU) || self.held(VK_LMENU) || self.held(VK_RMENU)
    }

    /// Sets the window thread's key state to what the lane holds, so
    /// shortcuts that check GetKeyState (Ctrl+S) see the modifiers.
    fn sync_modifiers(&mut self) {
        let modifiers: Vec<u16> = self
            .keys
            .iter()
            .copied()
            .filter(|k| is_modifier(*k))
            .collect();
        // SAFETY: attaching our input queue to the window's thread for the
        // calls, then detaching; the key state buffers are 256 bytes.
        unsafe {
            let ours = GetCurrentThreadId();
            if AttachThreadInput(ours, self.thread, 1) == 0 {
                return;
            }
            let mut state = [0u8; 256];
            GetKeyboardState(state.as_mut_ptr());
            let base = *self.saved_state.get_or_insert(state);
            let mut next = base;
            for vk in modifiers.iter() {
                next[*vk as usize] |= 0x80;
                let generic = match *vk {
                    VK_LSHIFT | VK_RSHIFT => VK_SHIFT,
                    VK_LCONTROL | VK_RCONTROL => VK_CONTROL,
                    VK_LMENU | VK_RMENU => VK_MENU,
                    other => other,
                };
                next[generic as usize] |= 0x80;
            }
            SetKeyboardState(next.as_ptr());
            if modifiers.is_empty() {
                self.saved_state = None;
            }
            AttachThreadInput(ours, self.thread, 0);
        }
    }

    /// The child control under a window-local point, and the point in its
    /// own client coordinates.
    fn target_at(&self, x: f64, y: f64) -> (HWND, i32, i32) {
        let top = self.top();
        let mut point = POINT {
            x: x.round() as i32,
            y: y.round() as i32,
        };
        // SAFETY: plain coordinate conversions on live handles.
        unsafe {
            let mut screen = point;
            ClientToScreen(top, &mut screen);
            let mut hwnd = top;
            loop {
                let mut local = screen;
                ScreenToClient(hwnd, &mut local);
                let child = ChildWindowFromPointEx(
                    hwnd,
                    local,
                    CWP_SKIPINVISIBLE | CWP_SKIPDISABLED | CWP_SKIPTRANSPARENT,
                );
                if child.is_null() || child == hwnd {
                    point = local;
                    break;
                }
                hwnd = child;
            }
            (hwnd, point.x, point.y)
        }
    }

    /// wParam key-state flags for mouse messages.
    fn mouse_flags(&self) -> usize {
        let mut flags = 0;
        for button in &self.buttons {
            flags |= match button {
                0 => MK_LBUTTON,
                1 => MK_RBUTTON,
                2 => MK_MBUTTON,
                3 => MK_XBUTTON1,
                4 => MK_XBUTTON2,
                _ => 0,
            };
        }
        if self.held(VK_SHIFT) || self.held(VK_LSHIFT) || self.held(VK_RSHIFT) {
            flags |= MK_SHIFT;
        }
        if self.held(VK_CONTROL) || self.held(VK_LCONTROL) || self.held(VK_RCONTROL) {
            flags |= MK_CONTROL;
        }
        flags
    }

    /// Where the pointer is; the middle of the window until it's moved.
    fn position(&self) -> (f64, f64) {
        self.pointer.unwrap_or_else(|| {
            crate::window::bounds_of(&WindowRef {
                id: self.top.to_string(),
                desktop: crate::window::DesktopKind::Windows,
            })
            .map(|b| (b.width / 2.0, b.height / 2.0))
            .unwrap_or((0.0, 0.0))
        })
    }
}

impl Lane for WindowsLane {
    fn key(&mut self, vk: u16, pressed: bool) -> Result<(), String> {
        if pressed == self.held(vk) {
            return Ok(());
        }
        if pressed {
            self.keys.push(vk);
        } else {
            self.keys.retain(|k| *k != vk);
        }
        if is_modifier(vk) {
            self.sync_modifiers();
        }
        // SAFETY: a pure layout lookup.
        let scan = unsafe { MapVirtualKeyW(vk as u32, MAPVK_VK_TO_VSC) };
        let mut lparam = 1 | (scan & 0xFF) << 16;
        if is_extended(vk) {
            lparam |= 1 << 24;
        }
        let alt = self.alt_held() || matches!(vk, VK_MENU | VK_LMENU | VK_RMENU);
        if alt {
            lparam |= 1 << 29;
        }
        if !pressed {
            lparam |= 1 << 30 | 1 << 31;
        }
        let message = match (pressed, alt) {
            (true, false) => WM_KEYDOWN,
            (true, true) => WM_SYSKEYDOWN,
            (false, false) => WM_KEYUP,
            (false, true) => WM_SYSKEYUP,
        };
        post(self.focus(), message, vk as WPARAM, lparam as i32 as LPARAM)
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
        let (message, extra) = match (code, pressed) {
            (0, true) => (WM_LBUTTONDOWN, 0),
            (0, false) => (WM_LBUTTONUP, 0),
            (1, true) => (WM_RBUTTONDOWN, 0),
            (1, false) => (WM_RBUTTONUP, 0),
            (2, true) => (WM_MBUTTONDOWN, 0),
            (2, false) => (WM_MBUTTONUP, 0),
            (3, true) => (WM_XBUTTONDOWN, 1usize << 16),
            (3, false) => (WM_XBUTTONUP, 1 << 16),
            (4, true) => (WM_XBUTTONDOWN, 2 << 16),
            (4, false) => (WM_XBUTTONUP, 2 << 16),
            _ => return Err(format!("mouse button {code} can't be sent to a window")),
        };
        let (x, y) = self.position();
        let (hwnd, cx, cy) = self.target_at(x, y);
        post(
            hwnd,
            message,
            self.mouse_flags() | extra,
            lparam_point(cx, cy),
        )
    }

    fn motion(&mut self, x: f64, y: f64) -> Result<(), String> {
        self.pointer = Some((x, y));
        let (hwnd, cx, cy) = self.target_at(x, y);
        post(hwnd, WM_MOUSEMOVE, self.mouse_flags(), lparam_point(cx, cy))
    }

    fn relative(&mut self, dx: f64, dy: f64) -> Result<(), String> {
        let (x, y) = self.position();
        self.motion(x + dx, y + dy)
    }

    fn axis(&mut self, axis: Axis, value120: i32) -> Result<(), String> {
        // WM_MOUSEWHEEL counts up as positive; the lane counts down.
        let (message, delta) = match axis {
            Axis::Vertical => (WM_MOUSEWHEEL, -value120),
            Axis::Horizontal => (WM_MOUSEHWHEEL, value120),
        };
        let (x, y) = self.position();
        let (hwnd, _, _) = self.target_at(x, y);
        // Wheel messages carry screen coordinates.
        let mut screen = POINT {
            x: x.round() as i32,
            y: y.round() as i32,
        };
        // SAFETY: a coordinate conversion on a live handle.
        unsafe { ClientToScreen(self.top(), &mut screen) };
        let wparam = (delta as i16 as u16 as usize) << 16 | self.mouse_flags();
        post(hwnd, message, wparam, lparam_point(screen.x, screen.y))
    }

    fn text(&mut self, text: &str) -> Result<(), String> {
        let target = self.focus();
        let mut first = true;
        for c in text.chars() {
            if !first {
                std::thread::sleep(crate::macros::backend::INTER_CHAR_DELAY);
            }
            first = false;
            let c = if c == '\n' { '\r' } else { c };
            let mut units = [0u16; 2];
            for unit in c.encode_utf16(&mut units) {
                post(target, WM_CHAR, *unit as WPARAM, 1)?;
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
        self.keys.clear();
        self.buttons.clear();
        if self.saved_state.is_some() {
            self.sync_modifiers();
        }
        Ok(())
    }

    fn pointer(&self) -> (f64, f64) {
        self.position()
    }
}

/// A key's virtual-key code, and whether it needs Shift (for symbols typed
/// as keys, in the user's layout).
pub(super) fn key_code(key: &MacroKey) -> Option<(u16, bool)> {
    Some(match key {
        MacroKey::Return => (VK_RETURN, false),
        MacroKey::Backspace => (VK_BACK, false),
        MacroKey::Tab => (VK_TAB, false),
        MacroKey::Space => (VK_SPACE, false),
        MacroKey::Escape => (VK_ESCAPE, false),
        MacroKey::Delete => (VK_DELETE, false),
        MacroKey::Insert => (VK_INSERT, false),
        MacroKey::Home => (VK_HOME, false),
        MacroKey::End => (VK_END, false),
        MacroKey::PageUp => (VK_PRIOR, false),
        MacroKey::PageDown => (VK_NEXT, false),
        MacroKey::UpArrow => (VK_UP, false),
        MacroKey::DownArrow => (VK_DOWN, false),
        MacroKey::LeftArrow => (VK_LEFT, false),
        MacroKey::RightArrow => (VK_RIGHT, false),
        MacroKey::Shift | MacroKey::LShift => (VK_LSHIFT, false),
        MacroKey::RShift => (VK_RSHIFT, false),
        MacroKey::Control | MacroKey::LControl => (VK_LCONTROL, false),
        MacroKey::RControl => (VK_RCONTROL, false),
        MacroKey::Alt | MacroKey::Option | MacroKey::LMenu => (VK_LMENU, false),
        MacroKey::AltGr => (VK_RMENU, false),
        MacroKey::Meta => (VK_LWIN, false),
        MacroKey::CapsLock => (VK_CAPITAL, false),
        MacroKey::NumLock => (VK_NUMLOCK, false),
        MacroKey::ScrollLock => (VK_SCROLL, false),
        MacroKey::Pause => (VK_PAUSE, false),
        MacroKey::PrintScr => (VK_SNAPSHOT, false),
        MacroKey::F1 => (VK_F1, false),
        MacroKey::F2 => (VK_F2, false),
        MacroKey::F3 => (VK_F3, false),
        MacroKey::F4 => (VK_F4, false),
        MacroKey::F5 => (VK_F5, false),
        MacroKey::F6 => (VK_F6, false),
        MacroKey::F7 => (VK_F7, false),
        MacroKey::F8 => (VK_F8, false),
        MacroKey::F9 => (VK_F9, false),
        MacroKey::F10 => (VK_F10, false),
        MacroKey::F11 => (VK_F11, false),
        MacroKey::F12 => (VK_F12, false),
        MacroKey::F13 => (VK_F13, false),
        MacroKey::F14 => (VK_F14, false),
        MacroKey::F15 => (VK_F15, false),
        MacroKey::F16 => (VK_F16, false),
        MacroKey::F17 => (VK_F17, false),
        MacroKey::F18 => (VK_F18, false),
        MacroKey::F19 => (VK_F19, false),
        MacroKey::F20 => (VK_F20, false),
        MacroKey::F21 => (VK_F21, false),
        MacroKey::F22 => (VK_F22, false),
        MacroKey::F23 => (VK_F23, false),
        MacroKey::F24 => (VK_F24, false),
        MacroKey::Numpad0 => (VK_NUMPAD0, false),
        MacroKey::Numpad1 => (VK_NUMPAD1, false),
        MacroKey::Numpad2 => (VK_NUMPAD2, false),
        MacroKey::Numpad3 => (VK_NUMPAD3, false),
        MacroKey::Numpad4 => (VK_NUMPAD4, false),
        MacroKey::Numpad5 => (VK_NUMPAD5, false),
        MacroKey::Numpad6 => (VK_NUMPAD6, false),
        MacroKey::Numpad7 => (VK_NUMPAD7, false),
        MacroKey::Numpad8 => (VK_NUMPAD8, false),
        MacroKey::Numpad9 => (VK_NUMPAD9, false),
        MacroKey::Add => (VK_ADD, false),
        MacroKey::Subtract => (VK_SUBTRACT, false),
        MacroKey::Multiply => (VK_MULTIPLY, false),
        MacroKey::Divide => (VK_DIVIDE, false),
        MacroKey::Decimal => (VK_DECIMAL, false),
        MacroKey::VolumeDown => (VK_VOLUME_DOWN, false),
        MacroKey::VolumeMute => (VK_VOLUME_MUTE, false),
        MacroKey::VolumeUp => (VK_VOLUME_UP, false),
        MacroKey::Select => (0x29, false),
        MacroKey::Unicode(c) => {
            let mut units = [0u16; 2];
            if c.encode_utf16(&mut units).len() != 1 {
                return None;
            }
            // SAFETY: a pure layout lookup; -1 means the layout lacks it.
            let scan = unsafe { VkKeyScanW(units[0]) };
            if scan == -1 {
                return None;
            }
            ((scan & 0xFF) as u16, scan & 0x100 != 0)
        }
        MacroKey::Other(n) => (u16::try_from(*n).ok()?, false),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_keys_map_to_left_hand_modifiers() {
        assert_eq!(key_code(&MacroKey::Control), Some((VK_LCONTROL, false)));
        assert_eq!(key_code(&MacroKey::Return), Some((VK_RETURN, false)));
    }

    #[test]
    fn points_pack_into_lparam() {
        assert_eq!(lparam_point(3, 4), 0x0004_0003);
        assert_eq!(lparam_point(-1, 0) & 0xFFFF, 0xFFFF);
    }
}
