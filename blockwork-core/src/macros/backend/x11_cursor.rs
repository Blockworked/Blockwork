//! Reads the cursor position on an X11 session via `XQueryPointer`, since a
//! grabbed `uinput` mouse only sees relative deltas. Not used on Wayland,
//! where XWayland freezes it off X11 surfaces (see `cursor_track`).

use std::sync::OnceLock;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::ConnectionExt;
use x11rb::rust_connection::RustConnection;

fn connection() -> Option<&'static (RustConnection, usize)> {
    static CONN: OnceLock<Option<(RustConnection, usize)>> = OnceLock::new();
    CONN.get_or_init(|| x11rb::connect(None).ok()).as_ref()
}

/// Whether an X server is reachable for cursor queries.
pub fn is_available() -> bool {
    connection().is_some()
}

pub fn query_cursor_pos() -> Option<(i32, i32)> {
    let (conn, screen_num) = connection()?;
    let root = conn.setup().roots.get(*screen_num)?.root;
    let reply = conn.query_pointer(root).ok()?.reply().ok()?;
    Some((reply.root_x as i32, reply.root_y as i32))
}
