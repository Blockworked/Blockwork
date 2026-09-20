use crate::input::types::{Axis, Direction, MacroButton, MacroKey};
use std::sync::{Arc, Mutex};

pub enum CaptureDecision {
    Passthrough,
    Suppress,
}

#[derive(Clone, Copy)]
pub enum CaptureTimestamp {
    /// No better timestamp available than "right now" (Windows - low-level
    /// hooks are delivered synchronously enough, and the hook's own `time`
    /// field is lower-resolution than `Instant::now()`/QPC anyway).
    Now,
    /// Kernel/OS-supplied hardware timestamp (Linux evdev; macOS CGEvent).
    /// Not guaranteed to be wall-clock/Unix time - only meaningful as a
    /// relative delta between two values from the same backend/session.
    Hardware(std::time::SystemTime),
}

pub enum CaptureEvent {
    KeyPress(MacroKey),
    KeyRelease(MacroKey),
    ButtonPress(MacroButton),
    ButtonRelease(MacroButton),
    /// Relative mouse movement in pixels (dx, dy).
    MouseMoveRel(i32, i32),
    /// Absolute mouse position.
    MouseMoveAbs(f64, f64),
    /// Scroll ticks (horizontal, vertical).
    Scroll(i32, i32),
}

pub trait InputBackend: Send + 'static {
    fn key(&mut self, key: MacroKey, dir: Direction) -> Result<(), String>;
    fn raw_keycode(&mut self, keycode: u16, dir: Direction) -> Result<(), String>;
    fn button(&mut self, button: MacroButton, dir: Direction) -> Result<(), String>;
    fn move_mouse_rel(&mut self, dx: i32, dy: i32) -> Result<(), String>;
    fn move_mouse_abs(&mut self, x: i32, y: i32) -> Result<(), String>;
    fn scroll(&mut self, amount: i32, axis: Axis) -> Result<(), String>;
    fn text(&mut self, s: &str) -> Result<(), String>;
    fn cursor_pos(&self) -> Option<(i32, i32)>;
    /// Called when absolute recording starts, so `cursor_pos` is exact for it.
    /// Only Wayland has work to do here (see `cursor_track`).
    fn anchor_cursor(&mut self) -> Option<(i32, i32)> {
        self.cursor_pos()
    }
    /// Requests any platform permission needed for absolute mouse movement.
    fn ensure_absolute_mouse_support(&mut self) -> Result<(), String> {
        Ok(())
    }
}

/// Whether this is a Wayland session, where absolute recording tracks the
/// cursor rather than querying it (see `cursor_track`).
pub fn is_wayland_session() -> bool {
    #[cfg(target_os = "linux")]
    {
        wayland_display::is_wayland_session()
    }

    #[cfg(not(target_os = "linux"))]
    {
        false
    }
}

/// Whether this session can report an absolute cursor position at all
/// (checked before asking for a libei portal session on Linux).
pub fn absolute_mouse_position_source_available() -> bool {
    #[cfg(target_os = "linux")]
    {
        // Wayland has no cursor query, so the position is tracked instead;
        // X11 answers `XQueryPointer` directly.
        wayland_display::is_wayland_session() || x11_cursor::is_available()
    }

    #[cfg(not(target_os = "linux"))]
    {
        true
    }
}

/// Whether absolute movement is ready to use, including any required portal
/// permission. This remains false until the first absolute-mouse request.
pub fn absolute_mouse_position_available() -> bool {
    #[cfg(target_os = "linux")]
    {
        evdev::libei_available()
    }

    #[cfg(not(target_os = "linux"))]
    {
        true
    }
}

/// Returns the current cursor position in the coordinate space used by
/// absolute playback, when the active platform can provide one.
pub fn absolute_mouse_position() -> Option<(i32, i32)> {
    #[cfg(target_os = "linux")]
    {
        return cursor_track::current();
    }

    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}

/// Start the global input capture thread for the current platform. The
/// callback is called for each input event and returns whether to suppress it.
pub fn start_capture(
    callback: Box<dyn FnMut(CaptureEvent, CaptureTimestamp) -> CaptureDecision + Send + 'static>,
) {
    #[cfg(target_os = "linux")]
    evdev::start_capture_thread(callback);

    #[cfg(windows)]
    windows::start_capture_thread(callback);

    #[cfg(target_os = "macos")]
    macos::start_capture_thread(callback);

    // Silence unused-variable warning on unsupported platforms.
    #[cfg(not(any(target_os = "linux", windows, target_os = "macos")))]
    {
        let _ = callback;
        tracing::warn!("Input capture is not supported on this platform.");
    }
}

/// Called the instant a hotkey combo matches, before the action is
/// dispatched. Lets a backend snapshot state only meaningful at that
/// moment - on Windows, the foreground window to type the macro back into.
/// No-op elsewhere.
pub fn note_hotkey_matched() {
    #[cfg(windows)]
    windows::note_hotkey_matched();
}

/// Feeds a key event from the app's own webview into the same hotkey
/// pipeline the global OS hook uses (Chromium starves the Windows low-level
/// keyboard hook while the app's own window has focus). No-op elsewhere.
pub fn dispatch_from_focused_window(vk: u16, pressed: bool) -> bool {
    #[cfg(windows)]
    return windows::dispatch_from_focused_window(vk, pressed);
    #[cfg(not(windows))]
    {
        let _ = (vk, pressed);
        false
    }
}

/// Create the platform-specific input backend wrapped in
/// `Arc<Mutex<dyn InputBackend>>`.
pub fn create_backend() -> Option<Arc<Mutex<dyn InputBackend>>> {
    #[cfg(target_os = "linux")]
    {
        match evdev::EvdevBackend::new() {
            Ok(b) => {
                let arc: Arc<Mutex<dyn InputBackend>> = Arc::new(Mutex::new(b));
                return Some(arc);
            }
            Err(e) => {
                tracing::warn!("Failed to create evdev backend: {}", e);
                return None;
            }
        }
    }

    #[cfg(windows)]
    {
        match windows::WinApiBackend::new() {
            Ok(b) => {
                let arc: Arc<Mutex<dyn InputBackend>> = Arc::new(Mutex::new(b));
                return Some(arc);
            }
            Err(e) => {
                tracing::warn!("Failed to create Windows input backend: {}", e);
                return None;
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        let b = macos::MacosBackend::new();
        let arc: Arc<Mutex<dyn InputBackend>> = Arc::new(Mutex::new(b));
        return Some(arc);
    }

    #[cfg(not(any(target_os = "linux", windows, target_os = "macos")))]
    {
        tracing::warn!("No input backend available on this platform.");
        None
    }
}

#[cfg(target_os = "linux")]
pub mod evdev_mapping;
#[cfg(target_os = "linux")]
pub mod evdev;
#[cfg(target_os = "linux")]
mod cursor_track;
#[cfg(target_os = "linux")]
mod x11_cursor;
#[cfg(target_os = "linux")]
mod wayland_display;

#[cfg(windows)]
pub mod windows;

#[cfg(target_os = "macos")]
pub mod macos;
