//! What is under the mouse right now, for the inspector panel: the window
//! (and the mouse position inside it), the pixel color and the native UI
//! element, so a macro author can read them all in one place.

use crate::accessibility::UiHit;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct InspectedWindow {
    pub title: String,
    pub app: String,
    /// 0 when the system doesn't say.
    pub pid: u32,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    /// The mouse, counted from the window's corner (as Use coordinates does).
    pub mouse_x: f64,
    pub mouse_y: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Inspected {
    pub x: f64,
    pub y: f64,
    pub window: Option<InspectedWindow>,
    /// `#rrggbb`, when asked for; reading it takes a screenshot.
    pub color: Option<String>,
    pub element: Option<UiHit>,
    /// Why a part is missing, by part ("window", "color", "element").
    pub notes: Vec<(String, String)>,
}

/// Inspects the point under the mouse, skipping windows titled `ignore`
/// (the inspector's own).
pub fn at_mouse(ignore: &str, color: bool) -> Result<Inspected, String> {
    let (x, y) = crate::screen::cursor()?;
    Ok(at(x.into(), y.into(), ignore, color))
}

pub fn at(x: f64, y: f64, ignore: &str, color: bool) -> Inspected {
    let mut notes = vec![];
    let window = match window_at(x, y, ignore) {
        Ok(w) => w,
        Err(e) => {
            notes.push(("window".to_string(), e));
            None
        }
    };
    let color = if color {
        crate::screen::pixel_color(x, y)
            .map_err(|e| notes.push(("color".to_string(), e)))
            .ok()
    } else {
        None
    };
    let element = if crate::accessibility::enabled().is_ok() {
        crate::accessibility::element_at(x, y, ignore)
            .map_err(|e| notes.push(("element".to_string(), e)))
            .ok()
            .flatten()
    } else {
        None
    };
    Inspected {
        x,
        y,
        window,
        color,
        element,
        notes,
    }
}

fn window_at(x: f64, y: f64, ignore: &str) -> Result<Option<InspectedWindow>, String> {
    let skip = |title: &str| !ignore.is_empty() && title.starts_with(ignore);
    let list = crate::window::list()?;
    Ok(list
        .into_iter()
        .find(|w| w.contains(x, y) && !skip(&w.title))
        .map(|w| InspectedWindow {
            mouse_x: x - w.bounds.x,
            mouse_y: y - w.bounds.y,
            title: w.title,
            app: w.app,
            pid: w.pid,
            x: w.bounds.x,
            y: w.bounds.y,
            width: w.bounds.width,
            height: w.bounds.height,
        }))
}
