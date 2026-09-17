//! Best-effort query of the real, compositor-tracked cursor position via
//! XWayland's X11 socket (`XQueryPointer` on the root window). Read-only -
//! no input injection here, unlike the removed XTest *emission* backend;
//! this is core X11 protocol, not the XTest extension.
//!
//! A grabbed `uinput` mouse only ever sees relative deltas, so
//! `EvdevBackend`'s own running-sum tracking (`CURSOR_X`/`CURSOR_Y`) is
//! nothing but a counter since app launch with no relationship to real
//! screen pixels: it starts at (0, 0) regardless of where the cursor
//! actually is, and pointer acceleration curves and edge clamping mean raw
//! device deltas don't even sum linearly to screen pixels. XWayland's X
//! server tracks the compositor's real global pointer position for its own
//! clients and answers `XQueryPointer` on the root window with it
//! regardless of whether the querying process is itself an X11 client -
//! the same trick tools like `xdotool` use to get real coordinates under
//! Wayland. Returns `None` if no X11 display is reachable at all (a
//! pure-Wayland session with no XWayland), so callers must have a fallback.

use std::sync::OnceLock;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::ConnectionExt;
use x11rb::rust_connection::RustConnection;

fn connection() -> Option<&'static (RustConnection, usize)> {
    static CONN: OnceLock<Option<(RustConnection, usize)>> = OnceLock::new();
    CONN.get_or_init(|| x11rb::connect(None).ok()).as_ref()
}

/// Whether an X server is reachable for accurate global cursor queries.
pub fn is_available() -> bool {
    connection().is_some()
}

pub fn query_cursor_pos() -> Option<(i32, i32)> {
    let (conn, screen_num) = connection()?;
    let root = conn.setup().roots.get(*screen_num)?.root;
    let reply = conn.query_pointer(root).ok()?.reply().ok()?;
    Some((reply.root_x as i32, reply.root_y as i32))
}
