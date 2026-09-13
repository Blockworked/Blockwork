//! Cross-platform application/window sensing and control, backing
//! `Op::FocusedApplication`/`FocusedWindowTitle`/`ApplicationIsRunning`/
//! `WindowExists` and the `FocusWindow`/`MinimizeWindow`/`MaximizeWindow`/
//! `MoveWindow` instructions. Structured like `battery.rs`/`backend/mod.rs`:
//! one cross-platform dispatch function per capability, `#[cfg(target_os =
//! ...)]`-gated per-platform submodules underneath.
//!
//! Every "find a window" operation takes an optional app-name filter and an
//! optional title-substring filter (either `None`/empty meaning "don't
//! filter on this") and re-resolves the match fresh on every call — nothing
//! here keeps a window handle across calls, mirroring `macros::runner`'s
//! existing `close_app()`, which re-derives its process match from a plain
//! name every time rather than remembering a handle from launch.

/// The process/app name owning the currently focused window (e.g.
/// `"firefox"`, `"org.kde.konsole"`), or `None` if it can't be determined
/// (no window focused, unsupported desktop session, platform not yet
/// supported).
pub fn focused_application() -> Option<String> {
    imp::focused_application()
}

/// The title of the currently focused window, or `None` (same conditions as
/// `focused_application`).
pub fn focused_window_title() -> Option<String> {
    imp::focused_window_title()
}

/// Whether a process matching `app` (case-insensitive substring of its
/// executable/app name) is currently running. Doesn't require a window —
/// a tray-only or headless process still counts. Built on `sysinfo`
/// (cross-platform process enumeration), unlike every other function here —
/// process listing needs no window-manager API, so it isn't gated behind
/// the per-platform `imp` module the window-focused functions below use.
pub fn is_app_running(app: &str) -> bool {
    process::is_app_running(app)
}

/// Whether any window matches both filters (a blank filter always matches).
pub fn window_exists(app: Option<&str>, title_contains: Option<&str>) -> bool {
    imp::window_exists(app, title_contains)
}

/// Focuses the first window matching both filters. `Ok(false)` means no
/// matching window was found (not an error — the block just does nothing).
pub fn focus_window(app: Option<&str>, title_contains: Option<&str>) -> Result<bool, String> {
    imp::focus_window(app, title_contains)
}

/// Minimizes the first window matching both filters.
pub fn minimize_window(app: Option<&str>, title_contains: Option<&str>) -> Result<bool, String> {
    imp::minimize_window(app, title_contains)
}

/// Maximizes the first window matching both filters.
pub fn maximize_window(app: Option<&str>, title_contains: Option<&str>) -> Result<bool, String> {
    imp::maximize_window(app, title_contains)
}

/// Moves the first window matching both filters so its top-left corner is at
/// `(x, y)`, keeping its current size.
pub fn move_window(app: Option<&str>, title_contains: Option<&str>, x: i32, y: i32) -> Result<bool, String> {
    imp::move_window(app, title_contains, x, y)
}

/// A real window's true on-screen position/size and identity, as the window
/// manager/compositor itself sees it — see `window_at_point`.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct WindowAtPoint {
    pub app: String,
    pub title: String,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

/// Native title assigned to Blockwork's full-screen picker surface. Point
/// hit-testing must ignore this surface so it can inspect the window below
/// it while still receiving the user's mouse input itself.
pub const ELEMENT_PICKER_OVERLAY_TITLE: &str = "Blockwork Element Picker Overlay";

/// Finds the topmost normal window whose frame contains screen point
/// `(x, y)`, with its true geometry. Used by the accessibility element
/// picker: a native Wayland client can't know its own on-screen position
/// (Wayland deliberately never tells clients this), so AT-SPI's own
/// "screen coordinate" queries are unusable for finding *which* window is
/// under an arbitrary point — this is the trustworthy source for that,
/// combined with AT-SPI's window-*relative* coordinates (which stay
/// self-consistent) to hit-test the specific element — see
/// `crate::accessibility::linux::element_at_point`.
pub fn window_at_point(x: i32, y: i32) -> Option<WindowAtPoint> {
    imp::window_at_point(x, y, None)
}

/// As [`window_at_point`], but skips a window whose title exactly matches
/// `excluded_title` (case-insensitively). This is crate-visible for the
/// accessibility picker, whose own always-on-top overlay would otherwise
/// be the hit for every screen coordinate.
pub(crate) fn window_at_point_excluding(x: i32, y: i32, excluded_title: &str) -> Option<WindowAtPoint> {
    imp::window_at_point(x, y, Some(excluded_title))
}

/// Sets/clears "always keep above other windows" on the first window
/// matching both filters — see `window_at_point`'s doc comment for the
/// related Wayland limitation this works around for the element picker's
/// overlay window (best-effort: `Err`/`Ok(false)` should be swallowed by
/// the caller, same as every other "no matching window" outcome here).
pub fn set_keep_above(app: Option<&str>, title_contains: Option<&str>, keep_above: bool) -> Result<bool, String> {
    imp::set_keep_above(app, title_contains, keep_above)
}

/// Forces the first window matching both filters to occupy exactly
/// `(x, y, width, height)`. Used by the element picker to make its own
/// overlay actually span every monitor: on Wayland, a client's requested
/// window position/initial size is only advisory (see
/// `linux::kwin_dbus::set_window_geometry`'s doc comment for why), so this
/// goes through the same compositor-scripting workaround as `set_keep_above`.
pub fn set_window_geometry(app: Option<&str>, title_contains: Option<&str>, x: i32, y: i32, width: i32, height: i32) -> Result<bool, String> {
    imp::set_window_geometry(app, title_contains, x, y, width, height)
}

/// Sends `key` (with `modifiers` — a `crate::hotkey_types::MOD_*` bitmask —
/// held) to the first window matching both filters, via a synthetic input
/// event delivered directly to that window: never moves real keyboard focus,
/// never touches the real keyboard, and never affects any other window.
/// `Ok(false)` means no matching window was reachable this way — not
/// necessarily "doesn't exist": on Linux this can only ever reach a genuine
/// X11 client window (a plain X11 session, or an XWayland-backed app under a
/// Wayland session) — a native Wayland toplevel has no such window to target
/// at all, and this deliberately never falls back to global input for one.
/// See `linux::x11_keys`'s module doc comment.
pub fn send_key_to_window(app: Option<&str>, title_contains: Option<&str>, key: &crate::input::types::MacroKey, modifiers: u8) -> Result<bool, String> {
    imp::send_key_to_window(app, title_contains, key, modifiers)
}

/// As [`send_key_to_window`], but types `text` character-by-character into
/// the matched window instead of a single keypress.
pub fn type_text_in_window(app: Option<&str>, title_contains: Option<&str>, text: &str) -> Result<bool, String> {
    imp::type_text_in_window(app, title_contains, text)
}

mod process;

#[cfg(target_os = "linux")]
#[path = "linux/mod.rs"]
mod imp;

#[cfg(windows)]
mod imp {
    //! Not yet implemented — every call reports "nothing found"/"not
    //! supported" rather than panicking, same spirit as
    //! `backend::create_backend()`'s unsupported-platform fallback.
    pub fn focused_application() -> Option<String> {
        None
    }
    pub fn focused_window_title() -> Option<String> {
        None
    }
    pub fn window_exists(_app: Option<&str>, _title_contains: Option<&str>) -> bool {
        false
    }
    pub fn focus_window(_app: Option<&str>, _title_contains: Option<&str>) -> Result<bool, String> {
        Err("window sensing is not yet supported on Windows".to_string())
    }
    pub fn minimize_window(_app: Option<&str>, _title_contains: Option<&str>) -> Result<bool, String> {
        Err("window sensing is not yet supported on Windows".to_string())
    }
    pub fn maximize_window(_app: Option<&str>, _title_contains: Option<&str>) -> Result<bool, String> {
        Err("window sensing is not yet supported on Windows".to_string())
    }
    pub fn move_window(_app: Option<&str>, _title_contains: Option<&str>, _x: i32, _y: i32) -> Result<bool, String> {
        Err("window sensing is not yet supported on Windows".to_string())
    }
    pub fn window_at_point(_x: i32, _y: i32, _excluded_title: Option<&str>) -> Option<super::WindowAtPoint> {
        None
    }
    pub fn set_keep_above(_app: Option<&str>, _title_contains: Option<&str>, _keep_above: bool) -> Result<bool, String> {
        Ok(false)
    }
    pub fn set_window_geometry(_app: Option<&str>, _title_contains: Option<&str>, _x: i32, _y: i32, _width: i32, _height: i32) -> Result<bool, String> {
        Ok(false)
    }
    pub fn send_key_to_window(_app: Option<&str>, _title_contains: Option<&str>, _key: &crate::input::types::MacroKey, _modifiers: u8) -> Result<bool, String> {
        Err("background key injection is not yet supported on Windows".to_string())
    }
    pub fn type_text_in_window(_app: Option<&str>, _title_contains: Option<&str>, _text: &str) -> Result<bool, String> {
        Err("background key injection is not yet supported on Windows".to_string())
    }
}

#[cfg(target_os = "macos")]
mod imp {
    //! Not yet implemented — see the `windows` stub above for the rationale.
    pub fn focused_application() -> Option<String> {
        None
    }
    pub fn focused_window_title() -> Option<String> {
        None
    }
    pub fn window_exists(_app: Option<&str>, _title_contains: Option<&str>) -> bool {
        false
    }
    pub fn focus_window(_app: Option<&str>, _title_contains: Option<&str>) -> Result<bool, String> {
        Err("window sensing is not yet supported on macOS".to_string())
    }
    pub fn minimize_window(_app: Option<&str>, _title_contains: Option<&str>) -> Result<bool, String> {
        Err("window sensing is not yet supported on macOS".to_string())
    }
    pub fn maximize_window(_app: Option<&str>, _title_contains: Option<&str>) -> Result<bool, String> {
        Err("window sensing is not yet supported on macOS".to_string())
    }
    pub fn move_window(_app: Option<&str>, _title_contains: Option<&str>, _x: i32, _y: i32) -> Result<bool, String> {
        Err("window sensing is not yet supported on macOS".to_string())
    }
    pub fn window_at_point(_x: i32, _y: i32, _excluded_title: Option<&str>) -> Option<super::WindowAtPoint> {
        None
    }
    pub fn set_keep_above(_app: Option<&str>, _title_contains: Option<&str>, _keep_above: bool) -> Result<bool, String> {
        Ok(false)
    }
    pub fn set_window_geometry(_app: Option<&str>, _title_contains: Option<&str>, _x: i32, _y: i32, _width: i32, _height: i32) -> Result<bool, String> {
        Ok(false)
    }
    pub fn send_key_to_window(_app: Option<&str>, _title_contains: Option<&str>, _key: &crate::input::types::MacroKey, _modifiers: u8) -> Result<bool, String> {
        Err("background key injection is not yet supported on macOS".to_string())
    }
    pub fn type_text_in_window(_app: Option<&str>, _title_contains: Option<&str>, _text: &str) -> Result<bool, String> {
        Err("background key injection is not yet supported on macOS".to_string())
    }
}

#[cfg(not(any(target_os = "linux", windows, target_os = "macos")))]
mod imp {
    pub fn focused_application() -> Option<String> {
        None
    }
    pub fn focused_window_title() -> Option<String> {
        None
    }
    pub fn window_exists(_app: Option<&str>, _title_contains: Option<&str>) -> bool {
        false
    }
    pub fn focus_window(_app: Option<&str>, _title_contains: Option<&str>) -> Result<bool, String> {
        Err("window sensing is not supported on this platform".to_string())
    }
    pub fn minimize_window(_app: Option<&str>, _title_contains: Option<&str>) -> Result<bool, String> {
        Err("window sensing is not supported on this platform".to_string())
    }
    pub fn maximize_window(_app: Option<&str>, _title_contains: Option<&str>) -> Result<bool, String> {
        Err("window sensing is not supported on this platform".to_string())
    }
    pub fn move_window(_app: Option<&str>, _title_contains: Option<&str>, _x: i32, _y: i32) -> Result<bool, String> {
        Err("window sensing is not supported on this platform".to_string())
    }
    pub fn window_at_point(_x: i32, _y: i32, _excluded_title: Option<&str>) -> Option<super::WindowAtPoint> {
        None
    }
    pub fn set_keep_above(_app: Option<&str>, _title_contains: Option<&str>, _keep_above: bool) -> Result<bool, String> {
        Ok(false)
    }
    pub fn set_window_geometry(_app: Option<&str>, _title_contains: Option<&str>, _x: i32, _y: i32, _width: i32, _height: i32) -> Result<bool, String> {
        Ok(false)
    }
    pub fn send_key_to_window(_app: Option<&str>, _title_contains: Option<&str>, _key: &crate::input::types::MacroKey, _modifiers: u8) -> Result<bool, String> {
        Err("background key injection is not supported on this platform".to_string())
    }
    pub fn type_text_in_window(_app: Option<&str>, _title_contains: Option<&str>, _text: &str) -> Result<bool, String> {
        Err("background key injection is not supported on this platform".to_string())
    }
}
