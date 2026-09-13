//! Linux window sensing/control, dispatched by desktop session — see this
//! file's `session()` for the detection rule. Plasma Wayland (the primary
//! target: KWin has no stable public D-Bus window-control surface, so this
//! goes through its scripting API instead — see `kwin_dbus.rs`'s module doc
//! for why) and plain X11 (via the `x11rb` dependency already used
//! read-only by `macros::backend::x11_cursor`) are both fully implemented;
//! any other Wayland compositor gets no window backend for now; there's no
//! reliable window-manager-agnostic protocol to fall back to, and an
//! XWayland-only query there would silently miss every native Wayland
//! client, which is worse than reporting nothing.

mod kwin_dbus;
mod x11;
mod x11_keys;

enum Session {
    PlasmaWayland,
    X11,
    Unsupported,
}

fn session() -> Session {
    let is_wayland = std::env::var("XDG_SESSION_TYPE").map(|v| v == "wayland").unwrap_or(false)
        || std::env::var_os("WAYLAND_DISPLAY").is_some();
    if is_wayland {
        let is_plasma = std::env::var("XDG_CURRENT_DESKTOP")
            .map(|v| v.to_lowercase().contains("kde"))
            .unwrap_or(false)
            || std::env::var_os("KDE_SESSION_VERSION").is_some();
        return if is_plasma { Session::PlasmaWayland } else { Session::Unsupported };
    }
    if std::env::var_os("DISPLAY").is_some() {
        return Session::X11;
    }
    Session::Unsupported
}

pub fn focused_application() -> Option<String> {
    match session() {
        Session::PlasmaWayland => kwin_dbus::focused_application(),
        Session::X11 => x11::focused_application(),
        Session::Unsupported => None,
    }
}

pub fn focused_window_title() -> Option<String> {
    match session() {
        Session::PlasmaWayland => kwin_dbus::focused_window_title(),
        Session::X11 => x11::focused_window_title(),
        Session::Unsupported => None,
    }
}

pub fn window_exists(app: Option<&str>, title_contains: Option<&str>) -> bool {
    match session() {
        Session::PlasmaWayland => kwin_dbus::window_exists(app, title_contains),
        Session::X11 => x11::window_exists(app, title_contains),
        Session::Unsupported => false,
    }
}

pub fn focus_window(app: Option<&str>, title_contains: Option<&str>) -> Result<bool, String> {
    match session() {
        Session::PlasmaWayland => kwin_dbus::focus_window(app, title_contains),
        Session::X11 => x11::focus_window(app, title_contains),
        Session::Unsupported => Err(unsupported_session_error()),
    }
}

pub fn minimize_window(app: Option<&str>, title_contains: Option<&str>) -> Result<bool, String> {
    match session() {
        Session::PlasmaWayland => kwin_dbus::minimize_window(app, title_contains),
        Session::X11 => x11::minimize_window(app, title_contains),
        Session::Unsupported => Err(unsupported_session_error()),
    }
}

pub fn maximize_window(app: Option<&str>, title_contains: Option<&str>) -> Result<bool, String> {
    match session() {
        Session::PlasmaWayland => kwin_dbus::maximize_window(app, title_contains),
        Session::X11 => x11::maximize_window(app, title_contains),
        Session::Unsupported => Err(unsupported_session_error()),
    }
}

pub fn move_window(app: Option<&str>, title_contains: Option<&str>, x: i32, y: i32) -> Result<bool, String> {
    match session() {
        Session::PlasmaWayland => kwin_dbus::move_window(app, title_contains, x, y),
        Session::X11 => x11::move_window(app, title_contains, x, y),
        Session::Unsupported => Err(unsupported_session_error()),
    }
}

pub fn window_at_point(x: i32, y: i32, excluded_title: Option<&str>) -> Option<super::WindowAtPoint> {
    match session() {
        Session::PlasmaWayland => kwin_dbus::window_at_point(x, y, excluded_title),
        Session::X11 => x11::window_at_point(x, y, excluded_title),
        Session::Unsupported => None,
    }
}

pub fn set_keep_above(app: Option<&str>, title_contains: Option<&str>, keep_above: bool) -> Result<bool, String> {
    match session() {
        Session::PlasmaWayland => kwin_dbus::set_keep_above(app, title_contains, keep_above),
        // Winit's own `always_on_top` window config already works natively
        // on X11 (EWMH-based) — no compositor-scripting workaround needed.
        Session::X11 => Ok(true),
        Session::Unsupported => Err(unsupported_session_error()),
    }
}

pub fn set_window_geometry(app: Option<&str>, title_contains: Option<&str>, x: i32, y: i32, width: i32, height: i32) -> Result<bool, String> {
    match session() {
        Session::PlasmaWayland => kwin_dbus::set_window_geometry(app, title_contains, x, y, width, height),
        // Winit's own `position`/`inner_size` builder calls already work
        // natively on X11 (a client can set its own absolute geometry there)
        // — no compositor-scripting workaround needed, same as `set_keep_above`.
        Session::X11 => Ok(true),
        Session::Unsupported => Err(unsupported_session_error()),
    }
}

fn unsupported_session_error() -> String {
    "window sensing needs Plasma/Wayland or an X11 session".to_string()
}

/// Sends `key` (with `modifiers` held) to the first window matching both
/// filters via a synthetic X11 `SendEvent`, never touching the real
/// keyboard or window focus. Unlike every other function in this file, this
/// is **not** dispatched through `session()` — it only ever targets
/// X11-backed windows (a plain X11 session, or an XWayland-backed app under
/// Wayland) in the first place, so it always attempts the X11 connection
/// directly regardless of desktop session, and reports "not found" rather
/// than falling back to anything global. See `x11_keys`'s module doc
/// comment for the full reasoning and its Wayland-native-client limitation.
pub fn send_key_to_window(app: Option<&str>, title_contains: Option<&str>, key: &crate::input::types::MacroKey, modifiers: u8) -> Result<bool, String> {
    x11_keys::send_key_to_window(app, title_contains, key, modifiers)
}

/// As [`send_key_to_window`], but types `text` character-by-character
/// instead of a single keypress. See `x11_keys::type_text_in_window`.
pub fn type_text_in_window(app: Option<&str>, title_contains: Option<&str>, text: &str) -> Result<bool, String> {
    x11_keys::type_text_in_window(app, title_contains, text)
}
