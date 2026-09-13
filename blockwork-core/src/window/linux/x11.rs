//! Window sensing/control on a plain X11 session via EWMH
//! (`_NET_CLIENT_LIST`, `_NET_ACTIVE_WINDOW`, `_NET_WM_STATE`, ICCCM
//! `WM_CHANGE_STATE`), extending the `x11rb` dependency already used
//! read-only by `macros::backend::x11_cursor` for cursor-position queries.
//!
//! Deliberately not attempted on a Wayland session (see `linux/mod.rs`'s
//! `session()`) — an XWayland connection there would only ever see rootless
//! XWayland clients, silently missing every native Wayland toplevel, which
//! is worse than reporting nothing.

use std::sync::OnceLock;
use x11rb::connection::Connection as _;
use x11rb::protocol::xproto::{Atom, AtomEnum, ClientMessageEvent, ConnectionExt, EventMask, Window};
use x11rb::rust_connection::RustConnection;

pub(super) struct Ewmh {
    pub(super) conn: RustConnection,
    pub(super) screen: usize,
    net_client_list: Atom,
    net_client_list_stacking: Atom,
    net_active_window: Atom,
    net_wm_name: Atom,
    net_wm_state: Atom,
    net_wm_state_hidden: Atom,
    net_wm_state_maximized_vert: Atom,
    net_wm_state_maximized_horz: Atom,
    net_moveresize_window: Atom,
    wm_change_state: Atom,
    wm_class: Atom,
    utf8_string: Atom,
}

pub(super) fn ewmh() -> Option<&'static Ewmh> {
    static EWMH: OnceLock<Option<Ewmh>> = OnceLock::new();
    EWMH.get_or_init(|| {
        let (conn, screen) = x11rb::connect(None).ok()?;
        let intern = |name: &str| -> Option<Atom> { Some(conn.intern_atom(false, name.as_bytes()).ok()?.reply().ok()?.atom) };
        Some(Ewmh {
            net_client_list: intern("_NET_CLIENT_LIST")?,
            net_client_list_stacking: intern("_NET_CLIENT_LIST_STACKING")?,
            net_active_window: intern("_NET_ACTIVE_WINDOW")?,
            net_wm_name: intern("_NET_WM_NAME")?,
            net_wm_state: intern("_NET_WM_STATE")?,
            net_wm_state_hidden: intern("_NET_WM_STATE_HIDDEN")?,
            net_wm_state_maximized_vert: intern("_NET_WM_STATE_MAXIMIZED_VERT")?,
            net_wm_state_maximized_horz: intern("_NET_WM_STATE_MAXIMIZED_HORZ")?,
            net_moveresize_window: intern("_NET_MOVERESIZE_WINDOW")?,
            wm_change_state: intern("WM_CHANGE_STATE")?,
            wm_class: intern("WM_CLASS")?,
            utf8_string: intern("UTF8_STRING")?,
            conn,
            screen,
        })
    })
    .as_ref()
}

pub(super) fn root(e: &Ewmh) -> Window {
    e.conn.setup().roots[e.screen].root
}

/// A window's app identity (`WM_CLASS`'s instance part, e.g. `"firefox"`)
/// and title (`_NET_WM_NAME`, falling back to `WM_NAME`).
pub(super) struct WindowInfo {
    pub(super) id: Window,
    app: String,
    title: String,
}

fn window_title(e: &Ewmh, id: Window) -> String {
    if let Ok(Ok(reply)) = e.conn.get_property(false, id, e.net_wm_name, e.utf8_string, 0, u32::MAX).map(|c| c.reply())
        && let Ok(text) = String::from_utf8(reply.value)
        && !text.is_empty()
    {
        return text;
    }
    e.conn
        .get_property(false, id, AtomEnum::WM_NAME, AtomEnum::STRING, 0, u32::MAX)
        .ok()
        .and_then(|c| c.reply().ok())
        .and_then(|reply| String::from_utf8(reply.value).ok())
        .unwrap_or_default()
}

/// `WM_CLASS` is two null-terminated strings back to back (instance, class);
/// the instance name is the closer match to a launch/executable name.
fn window_app(e: &Ewmh, id: Window) -> String {
    let Ok(Ok(reply)) = e.conn.get_property(false, id, e.wm_class, AtomEnum::STRING, 0, u32::MAX).map(|c| c.reply()) else {
        return String::new();
    };
    reply.value.split(|&b| b == 0).next().map(|s| String::from_utf8_lossy(s).to_string()).unwrap_or_default()
}

fn list_windows(e: &Ewmh) -> Vec<WindowInfo> {
    let Ok(Ok(reply)) = e.conn.get_property(false, root(e), e.net_client_list, AtomEnum::WINDOW, 0, u32::MAX).map(|c| c.reply()) else {
        return Vec::new();
    };
    reply
        .value32()
        .into_iter()
        .flatten()
        .map(|id| WindowInfo { id, app: window_app(e, id), title: window_title(e, id) })
        .collect()
}

/// `_NET_CLIENT_LIST_STACKING` is bottom-to-top, unlike `_NET_CLIENT_LIST`
/// (creation order) — needed so `window_at_point` picks the topmost window
/// on overlap, matching `kwin_dbus.rs`'s `window_at_point` z-order handling.
fn list_windows_stacking(e: &Ewmh) -> Vec<WindowInfo> {
    let Ok(Ok(reply)) = e.conn.get_property(false, root(e), e.net_client_list_stacking, AtomEnum::WINDOW, 0, u32::MAX).map(|c| c.reply()) else {
        return Vec::new();
    };
    reply
        .value32()
        .into_iter()
        .flatten()
        .map(|id| WindowInfo { id, app: window_app(e, id), title: window_title(e, id) })
        .collect()
}

/// A window's true absolute screen position/size. `get_geometry` alone
/// reports position relative to the window's immediate parent (typically
/// the window manager's decoration frame, not the root), so the position
/// half needs `translate_coordinates` into root-relative coordinates —
/// standard X11 technique for this.
fn window_geometry(e: &Ewmh, id: Window) -> Option<(i32, i32, u16, u16)> {
    let geom = e.conn.get_geometry(id).ok()?.reply().ok()?;
    let root_id = root(e);
    let translated = e.conn.translate_coordinates(id, root_id, 0, 0).ok()?.reply().ok()?;
    Some((translated.dst_x as i32, translated.dst_y as i32, geom.width, geom.height))
}

fn active_window(e: &Ewmh) -> Option<WindowInfo> {
    let reply = e.conn.get_property(false, root(e), e.net_active_window, AtomEnum::WINDOW, 0, 1).ok()?.reply().ok()?;
    let id = reply.value32()?.next()?;
    if id == 0 {
        return None;
    }
    Some(WindowInfo { id, app: window_app(e, id), title: window_title(e, id) })
}

fn matches(info: &WindowInfo, app: Option<&str>, title_contains: Option<&str>) -> bool {
    let filter_ok = |needle: Option<&str>, haystack: &str| match needle {
        Some(n) if !n.is_empty() => haystack.to_lowercase().contains(&n.to_lowercase()),
        _ => true,
    };
    filter_ok(app, &info.app) && filter_ok(title_contains, &info.title)
}

pub(super) fn find_window(e: &Ewmh, app: Option<&str>, title_contains: Option<&str>) -> Option<WindowInfo> {
    list_windows(e).into_iter().find(|w| matches(w, app, title_contains))
}

/// Sends an EWMH-style root-window `ClientMessage` — the standard way a
/// pager/external tool asks a compliant window manager to act on a window it
/// doesn't own, rather than poking the window's properties directly.
fn send_root_client_message(e: &Ewmh, window: Window, message_type: Atom, data: [u32; 5]) {
    let event = ClientMessageEvent::new(32, window, message_type, data);
    let _ = e.conn.send_event(false, root(e), EventMask::SUBSTRUCTURE_NOTIFY | EventMask::SUBSTRUCTURE_REDIRECT, event);
    let _ = e.conn.flush();
}

pub fn focused_application() -> Option<String> {
    let e = ewmh()?;
    active_window(e).map(|w| w.app).filter(|s| !s.is_empty())
}

pub fn focused_window_title() -> Option<String> {
    let e = ewmh()?;
    active_window(e).map(|w| w.title).filter(|s| !s.is_empty())
}

pub fn window_exists(app: Option<&str>, title_contains: Option<&str>) -> bool {
    let Some(e) = ewmh() else { return false };
    find_window(e, app, title_contains).is_some()
}

pub fn focus_window(app: Option<&str>, title_contains: Option<&str>) -> Result<bool, String> {
    let e = ewmh().ok_or_else(no_x11_error)?;
    let Some(target) = find_window(e, app, title_contains) else { return Ok(false) };
    // _NET_ACTIVE_WINDOW request: source indication 1 (normal application), timestamp 0 (unknown).
    send_root_client_message(e, target.id, e.net_active_window, [1, 0, 0, 0, 0]);
    Ok(true)
}

pub fn minimize_window(app: Option<&str>, title_contains: Option<&str>) -> Result<bool, String> {
    let e = ewmh().ok_or_else(no_x11_error)?;
    let Some(target) = find_window(e, app, title_contains) else { return Ok(false) };
    // ICCCM WM_CHANGE_STATE request with IconicState (3) — the standard way
    // an external client asks the WM to iconify/minimize a window it
    // doesn't own (there's no EWMH `_NET_WM_STATE` atom for "minimized").
    send_root_client_message(e, target.id, e.wm_change_state, [3, 0, 0, 0, 0]);
    Ok(true)
}

pub fn maximize_window(app: Option<&str>, title_contains: Option<&str>) -> Result<bool, String> {
    let e = ewmh().ok_or_else(no_x11_error)?;
    let Some(target) = find_window(e, app, title_contains) else { return Ok(false) };
    // _NET_WM_STATE request, action 1 = _NET_WM_STATE_ADD, both maximize axes at once.
    send_root_client_message(e, target.id, e.net_wm_state, [1, e.net_wm_state_maximized_vert, e.net_wm_state_maximized_horz, 1, 0]);
    Ok(true)
}

pub fn move_window(app: Option<&str>, title_contains: Option<&str>, x: i32, y: i32) -> Result<bool, String> {
    let e = ewmh().ok_or_else(no_x11_error)?;
    let Some(target) = find_window(e, app, title_contains) else { return Ok(false) };
    // _NET_MOVERESIZE_WINDOW: gravity (0 = default) | flags for x,y present
    // (1<<8 | 1<<9) in the low bits, then x, y, width, height (0 = unchanged).
    let flags = (1 << 8) | (1 << 9);
    send_root_client_message(e, target.id, e.net_moveresize_window, [flags, x as u32, y as u32, 0, 0]);
    Ok(true)
}

/// Whether the window is minimized (iconified): EWMH-compliant WMs set
/// `_NET_WM_STATE_HIDDEN` on `_NET_WM_STATE` for those. A minimized window
/// keeps a stale geometry but isn't visible — `window_at_point` must skip
/// it, same as the KWin backend skips `w.minimized`.
fn is_minimized(e: &Ewmh, id: Window) -> bool {
    let Ok(Ok(reply)) = e
        .conn
        .get_property(false, id, e.net_wm_state, AtomEnum::ATOM, 0, u32::MAX)
        .map(|c| c.reply())
    else {
        return false;
    };
    let Some(mut states) = reply.value32() else { return false };
    states.any(|s| s == e.net_wm_state_hidden)
}

/// Finds the topmost window whose true screen geometry contains
/// `(x, y)` — see `crate::window::window_at_point`'s doc comment. Unlike a
/// native Wayland client, an X11 window's own reported geometry is already
/// trustworthy (X11 has no equivalent of Wayland's position-hiding), so
/// this needs no AT-SPI-vs-compositor reconciliation the Wayland/KWin path
/// does — it's a plain top-down geometry scan.
pub fn window_at_point(x: i32, y: i32, excluded_title: Option<&str>) -> Option<crate::window::WindowAtPoint> {
    let e = ewmh()?;
    for info in list_windows_stacking(e).into_iter().rev() {
        if excluded_title.is_some_and(|title| info.title.eq_ignore_ascii_case(title)) {
            continue;
        }
        if is_minimized(e, info.id) {
            continue;
        }
        let Some((wx, wy, ww, wh)) = window_geometry(e, info.id) else { continue };
        if x >= wx && x < wx + ww as i32 && y >= wy && y < wy + wh as i32 {
            return Some(crate::window::WindowAtPoint {
                app: info.app,
                title: info.title,
                x: wx,
                y: wy,
                width: ww as i32,
                height: wh as i32,
            });
        }
    }
    None
}

fn no_x11_error() -> String {
    "no X11 display reachable".to_string()
}
