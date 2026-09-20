//! Reads the real cursor position via XWayland's X11 socket (`XQueryPointer`),
//! since a grabbed `uinput` mouse only sees relative deltas. Wayland sessions
//! convert the pixel-sized root to the XDG Output logical desktop.

use std::sync::OnceLock;
use x11rb::connection::Connection;
use x11rb::protocol::randr::ConnectionExt as RandrConnectionExt;
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
    let screen = conn.setup().roots.get(*screen_num)?;
    let root = screen.root;
    let reply = conn.query_pointer(root).ok()?.reply().ok()?;
    let x = reply.root_x as i32;
    let y = reply.root_y as i32;

    if let Some(monitor) = find_xwayland_monitor(conn, root, x, y) {
        return Some(map_to_logical_desktop(
            x - monitor.left,
            y - monitor.top,
            monitor.width,
            monitor.height,
            monitor.bounds,
        ));
    }

    let Some(bounds) = super::wayland_display::logical_desktop_bounds() else {
        return Some((x, y));
    };
    let root_width = i32::from(screen.width_in_pixels);
    let root_height = i32::from(screen.height_in_pixels);
    if root_width <= 0 || root_height <= 0 {
        return Some((x, y));
    }

    Some(map_to_logical_desktop(
        x,
        y,
        root_width,
        root_height,
        bounds,
    ))
}

/// The physical -> logical pixel ratio for the monitor under the pointer,
/// falling back to the combined-desktop ratio, then to 1:1. Converts a raw
/// evdev delta into the same logical space `query_cursor_pos` reports.
pub fn logical_delta_scale() -> (f64, f64) {
    (|| {
        let (conn, screen_num) = connection()?;
        let screen = conn.setup().roots.get(*screen_num)?;
        let root = screen.root;
        let reply = conn.query_pointer(root).ok()?.reply().ok()?;
        let (x, y) = (reply.root_x as i32, reply.root_y as i32);

        if let Some(monitor) = find_xwayland_monitor(conn, root, x, y) {
            return Some((
                f64::from(monitor.bounds.width) / f64::from(monitor.width),
                f64::from(monitor.bounds.height) / f64::from(monitor.height),
            ));
        }

        let bounds = super::wayland_display::logical_desktop_bounds()?;
        let root_width = i32::from(screen.width_in_pixels);
        let root_height = i32::from(screen.height_in_pixels);
        if root_width <= 0 || root_height <= 0 {
            return None;
        }
        Some((
            f64::from(bounds.width) / f64::from(root_width),
            f64::from(bounds.height) / f64::from(root_height),
        ))
    })()
    .unwrap_or((1.0, 1.0))
}

/// The XRandR monitor (in XWayland root-pixel space) containing `(x, y)`,
/// paired with the matching Wayland output's logical bounds.
struct XwaylandMonitor {
    left: i32,
    top: i32,
    width: i32,
    height: i32,
    bounds: super::wayland_display::DesktopBounds,
}

fn find_xwayland_monitor(
    conn: &RustConnection,
    root: u32,
    x: i32,
    y: i32,
) -> Option<XwaylandMonitor> {
    let outputs = super::wayland_display::logical_outputs()?;
    let monitors = conn.randr_get_monitors(root, true).ok()?.reply().ok()?;
    for monitor in monitors.monitors {
        let left = i32::from(monitor.x);
        let top = i32::from(monitor.y);
        let width = i32::from(monitor.width);
        let height = i32::from(monitor.height);
        if width <= 0
            || height <= 0
            || x < left
            || y < top
            || x >= left + width
            || y >= top + height
        {
            continue;
        }
        let name =
            String::from_utf8(conn.get_atom_name(monitor.name).ok()?.reply().ok()?.name).ok()?;
        let output = outputs.iter().find(|output| output.name == name)?;
        return Some(XwaylandMonitor {
            left,
            top,
            width,
            height,
            bounds: output.bounds,
        });
    }
    None
}

fn map_to_logical_desktop(
    x: i32,
    y: i32,
    root_width: i32,
    root_height: i32,
    bounds: super::wayland_display::DesktopBounds,
) -> (i32, i32) {
    (
        bounds.x + x * bounds.width / root_width,
        bounds.y + y * bounds.height / root_height,
    )
}

#[cfg(test)]
mod tests {
    use super::{super::wayland_display::DesktopBounds, map_to_logical_desktop};

    #[test]
    fn converts_xwayland_root_pixels_to_logical_desktop_coordinates() {
        let bounds = DesktopBounds {
            x: 0,
            y: 0,
            width: 1000,
            height: 800,
        };
        assert_eq!(
            map_to_logical_desktop(2640, 2000, 5280, 2000, bounds),
            (500, 800)
        );
    }
}
