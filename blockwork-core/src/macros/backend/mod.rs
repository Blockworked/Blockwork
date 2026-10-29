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
    fn launch_android_app(&mut self, _: &str, _: &dyn Fn() -> bool) -> Result<(), String> {
        Err("Android app launches are unavailable on this platform".into())
    }

    fn element(&mut self, _: &crate::input::element::ElementSelector, _: crate::input::element::ElementAction, _: &str, _: &dyn Fn() -> bool) -> Result<serde_json::Value, String> {
        Err("accessibility elements are unavailable on this platform".into())
    }

    fn gesture(&mut self, _: &crate::input::touch::Gesture, _: &dyn Fn() -> bool) -> Result<(), String> {
        Err("touch automation is unavailable on this platform".into())
    }

    /// A matched image is clicked by a mouse on desktop and tapped on Android.
    fn click_at(&mut self, x: i32, y: i32, button: MacroButton, _: &dyn Fn() -> bool) -> Result<(), String> {
        self.click_mouse_abs(x, y, button)
    }
    fn key(&mut self, key: MacroKey, dir: Direction) -> Result<(), String>;
    fn raw_keycode(&mut self, keycode: u16, dir: Direction) -> Result<(), String>;
    fn button(&mut self, button: MacroButton, dir: Direction) -> Result<(), String>;
    fn move_mouse_rel(&mut self, dx: i32, dy: i32) -> Result<(), String>;
    fn move_mouse_abs(&mut self, x: i32, y: i32) -> Result<(), String>;
    /// Keeps positioning and clicking ordered on backends with multiple input paths.
    fn click_mouse_abs(&mut self, x: i32, y: i32, button: MacroButton) -> Result<(), String> {
        self.move_mouse_abs(x, y)?;
        self.button(button, Direction::Click)
    }
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
    /// Why this backend can't send input at all, for `UnavailableBackend`.
    fn unavailable_reason(&self) -> Option<String> {
        None
    }
}

/// Stands in when the platform backend can't start (no `/dev/uinput` access
/// on Linux), so macros still run: blocks that don't send input work, and
/// the ones that do fail with `reason` instead of the run silently not
/// starting.
pub struct UnavailableBackend {
    reason: String,
}

impl UnavailableBackend {
    pub fn new() -> Self {
        let reason = if cfg!(target_os = "linux") {
            "input emulation isn't available: Blockwork can't open /dev/uinput, so it can't press keys or move the mouse"
        } else {
            "input emulation isn't available on this system, so Blockwork can't press keys or move the mouse"
        };
        UnavailableBackend { reason: reason.to_string() }
    }
}

impl Default for UnavailableBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl InputBackend for UnavailableBackend {
    fn key(&mut self, _: MacroKey, _: Direction) -> Result<(), String> {
        Err(self.reason.clone())
    }
    fn raw_keycode(&mut self, _: u16, _: Direction) -> Result<(), String> {
        Err(self.reason.clone())
    }
    fn button(&mut self, _: MacroButton, _: Direction) -> Result<(), String> {
        Err(self.reason.clone())
    }
    fn move_mouse_rel(&mut self, _: i32, _: i32) -> Result<(), String> {
        Err(self.reason.clone())
    }
    fn move_mouse_abs(&mut self, _: i32, _: i32) -> Result<(), String> {
        Err(self.reason.clone())
    }
    fn scroll(&mut self, _: i32, _: Axis) -> Result<(), String> {
        Err(self.reason.clone())
    }
    fn text(&mut self, _: &str) -> Result<(), String> {
        Err(self.reason.clone())
    }
    fn cursor_pos(&self) -> Option<(i32, i32)> {
        None
    }
    fn unavailable_reason(&self) -> Option<String> {
        Some(self.reason.clone())
    }
}

/// Releases modifier keys the user is physically holding, so a macro started
/// by a hotkey or key trigger doesn't type with the trigger's Ctrl/Alt/Shift
/// still down. Linux only; Windows does this in `prepare_for_macro_execution`.
pub fn release_held_modifiers() {
    release_held_keys(&[]);
}

/// `release_held_modifiers`, plus `keys` if they're held: a key trigger's own
/// key is still down when its macro starts, and typing that letter again
/// while it's held does nothing.
pub fn release_held_keys(keys: &[MacroKey]) {
    #[cfg(target_os = "linux")]
    evdev::release_held_keys(keys);
    #[cfg(not(target_os = "linux"))]
    let _ = keys;
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
pub(crate) mod cursor_track;
#[cfg(target_os = "linux")]
pub(crate) mod x11_cursor;
#[cfg(target_os = "linux")]
pub(crate) mod wayland_display;

#[cfg(windows)]
pub mod windows;

#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(test)]
mod compatibility_tests {
    use super::*;

    struct OrderedClick(bool);
    impl InputBackend for OrderedClick {
        fn key(&mut self, _: MacroKey, _: Direction) -> Result<(), String> { Ok(()) }
        fn raw_keycode(&mut self, _: u16, _: Direction) -> Result<(), String> { Ok(()) }
        fn button(&mut self, _: MacroButton, _: Direction) -> Result<(), String> { Err("Separate button path must not be used".into()) }
        fn move_mouse_rel(&mut self, _: i32, _: i32) -> Result<(), String> { Ok(()) }
        fn move_mouse_abs(&mut self, _: i32, _: i32) -> Result<(), String> { Err("Separate positioning path must not be used".into()) }
        fn scroll(&mut self, _: i32, _: Axis) -> Result<(), String> { Ok(()) }
        fn text(&mut self, _: &str) -> Result<(), String> { Ok(()) }
        fn cursor_pos(&self) -> Option<(i32, i32)> { None }
        fn click_mouse_abs(&mut self, x: i32, y: i32, button: MacroButton) -> Result<(), String> {
            assert_eq!((x, y, button), (12, 34, MacroButton::Right));
            self.0 = true;
            Ok(())
        }
    }
    #[test]
    fn default_image_click_uses_the_desktop_ordered_click_path() {
        let mut backend = OrderedClick(false);
        backend.click_at(12, 34, MacroButton::Right, &|| false).unwrap();
        assert!(backend.0);
    }
}
