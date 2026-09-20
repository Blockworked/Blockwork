//! Tracks where the cursor is on Wayland, which has no API to ask.
//!
//! `XQueryPointer` is frozen unless the pointer is over an XWayland surface,
//! so integrate the raw device deltas and pin them exact on every libei move.

use std::sync::{Mutex, OnceLock};

use super::wayland_display::{
    DesktopBounds, is_wayland_session, logical_desktop_bounds, logical_outputs,
};

/// libinput hands the compositor motion in logical pixels, so a raw evdev
/// count is roughly one of those (measured ~1.1). Exact while we drive.
const LOGICAL_PER_COUNT: f64 = 1.0;

struct Tracked {
    pos: (f64, f64),
    /// Set when we placed the cursor, cleared once a delta lands on top.
    exact: bool,
}

static TRACKED: OnceLock<Mutex<Option<Tracked>>> = OnceLock::new();

fn tracked() -> &'static Mutex<Option<Tracked>> {
    TRACKED.get_or_init(|| Mutex::new(None))
}

/// Pins the tracked position to one we just commanded through libei.
pub fn set_exact(x: f64, y: f64) {
    if let Ok(mut t) = tracked().lock() {
        *t = Some(Tracked {
            pos: (x, y),
            exact: true,
        });
    }
}

/// Records a position reached relatively, which pointer acceleration may
/// not have landed on exactly.
pub fn set_estimate(x: f64, y: f64) {
    if let Ok(mut t) = tracked().lock() {
        *t = Some(Tracked {
            pos: (x, y),
            exact: false,
        });
    }
}

/// The position to report to the rest of the app: tracked on Wayland, read
/// from the X server otherwise.
pub fn current() -> Option<(i32, i32)> {
    if is_wayland_session() {
        return position().map(|(x, y)| (x.round() as i32, y.round() as i32));
    }
    super::x11_cursor::query_cursor_pos()
}

pub fn position() -> Option<(f64, f64)> {
    tracked()
        .lock()
        .ok()
        .and_then(|t| t.as_ref().map(|t| t.pos))
}

/// Whether the tracked position is still the one we last commanded.
pub fn is_exact() -> bool {
    tracked()
        .lock()
        .ok()
        .and_then(|t| t.as_ref().map(|t| t.exact))
        .unwrap_or(false)
}

/// Integrates one raw device delta. Seeds from the desktop centre when
/// nothing is known yet; clamping corrects it again at every screen edge.
pub fn apply_device_delta(dx: i32, dy: i32) -> Option<(f64, f64)> {
    let (base_x, base_y) = match position() {
        Some(pos) => pos,
        None => desktop_center()?,
    };
    let next = clamp_to_desktop(
        base_x + f64::from(dx) * LOGICAL_PER_COUNT,
        base_y + f64::from(dy) * LOGICAL_PER_COUNT,
    );
    set_estimate(next.0, next.1);
    Some(next)
}

/// The position absolute recording should pin the cursor to when it starts.
pub fn anchor_target() -> Option<(f64, f64)> {
    position().or_else(desktop_center)
}

fn desktop_center() -> Option<(f64, f64)> {
    let bounds = logical_desktop_bounds()?;
    Some((
        f64::from(bounds.x) + f64::from(bounds.width) / 2.0,
        f64::from(bounds.y) + f64::from(bounds.height) / 2.0,
    ))
}

/// Keeps the estimate on-screen like the compositor does, snapping into the
/// nearest output when the layout leaves a gap.
fn clamp_to_desktop(x: f64, y: f64) -> (f64, f64) {
    match output_bounds() {
        Some(outputs) => clamp_within(outputs, x, y),
        None => (x, y),
    }
}

fn output_bounds() -> Option<&'static [DesktopBounds]> {
    static BOUNDS: OnceLock<Option<Vec<DesktopBounds>>> = OnceLock::new();
    BOUNDS
        .get_or_init(|| logical_outputs().map(|o| o.iter().map(|o| o.bounds).collect()))
        .as_deref()
}

fn clamp_within(outputs: &[DesktopBounds], x: f64, y: f64) -> (f64, f64) {
    if outputs.is_empty() || outputs.iter().any(|o| contains(o, x, y)) {
        return (x, y);
    }
    outputs
        .iter()
        .map(|o| clamp_into(o, x, y))
        .min_by(|a, b| {
            distance_sq(*a, (x, y))
                .partial_cmp(&distance_sq(*b, (x, y)))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .unwrap_or((x, y))
}

fn contains(output: &DesktopBounds, x: f64, y: f64) -> bool {
    x >= f64::from(output.x)
        && y >= f64::from(output.y)
        && x <= f64::from(output.x + output.width - 1)
        && y <= f64::from(output.y + output.height - 1)
}

fn clamp_into(output: &DesktopBounds, x: f64, y: f64) -> (f64, f64) {
    (
        x.clamp(f64::from(output.x), f64::from(output.x + output.width - 1)),
        y.clamp(f64::from(output.y), f64::from(output.y + output.height - 1)),
    )
}

fn distance_sq(a: (f64, f64), b: (f64, f64)) -> f64 {
    let (dx, dy) = (a.0 - b.0, a.1 - b.1);
    dx * dx + dy * dy
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bounds(x: i32, y: i32, width: i32, height: i32) -> DesktopBounds {
        DesktopBounds {
            x,
            y,
            width,
            height,
        }
    }

    #[test]
    fn leaves_positions_that_are_on_an_output_alone() {
        let outputs = [bounds(0, 13, 1707, 1067), bounds(1707, 0, 1920, 1080)];
        assert_eq!(clamp_within(&outputs, 2000.0, 700.0), (2000.0, 700.0));
    }

    #[test]
    fn clamps_past_the_desktop_edge_back_onto_the_nearest_output() {
        let outputs = [bounds(0, 13, 1707, 1067), bounds(1707, 0, 1920, 1080)];
        assert_eq!(clamp_within(&outputs, 5000.0, 700.0), (3626.0, 700.0));
        assert_eq!(clamp_within(&outputs, -50.0, 500.0), (0.0, 500.0));
    }

    #[test]
    fn snaps_into_the_nearest_output_when_the_layout_has_a_gap() {
        // The laptop panel sits 13px lower, so the top-left strip of the
        // desktop is off every output.
        let outputs = [bounds(0, 13, 1707, 1067), bounds(1707, 0, 1920, 1080)];
        assert_eq!(clamp_within(&outputs, 800.0, 5.0), (800.0, 13.0));
    }
}
