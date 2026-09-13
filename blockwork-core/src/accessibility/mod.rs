//! Cross-platform accessibility-tree (AT) querying/control — backs
//! `Op::ElementText`/`ElementExists`/`ElementEnabled` and the
//! `ClickElement`/`SetElementValue` instructions. Structured like
//! `crate::window`: one cross-platform dispatch function per capability,
//! `#[cfg(target_os = ...)]`-gated per-platform submodules underneath.
//!
//! Every operation takes the same [`ElementLocator`] — a [`ElementScope`]
//! (search whatever's focused, a specific app, or a specific window by
//! title) plus an element-name filter and a role filter (blank/`None`
//! meaning "don't filter on this"),
//! and re-resolves the match fresh on every call. Nothing here keeps a
//! handle across calls, same "no handles" philosophy as `crate::window`
//! (see that module's doc comment for the rationale): a `wait until element
//! is enabled` block is then just `WaitUntil` re-polling `Op::ElementEnabled`
//! with the same locator until it holds, rather than needing a separate
//! "find element" block to bind a reusable reference first.

pub use crate::input::types::ElementScope;

/// The role field is matched against the AT-SPI role *name* the target
/// platform reports for a node (e.g. `"push button"`, `"label"`, `"check
/// box"` on Linux/AT-SPI) rather than a fixed enum — the same free-text
/// spirit as `app`/`title_contains`, and the only representation that stays
/// meaningful across AT-SPI's, UI Automation's and AXUIElement's very
/// different role vocabularies once Windows/macOS are implemented.
pub struct ElementLocator<'a> {
    pub scope: ElementScope,
    /// Only consulted when `scope` is [`ElementScope::App`] — ignored
    /// (whether blank or not) for [`ElementScope::Focused`] and
    /// [`ElementScope::Window`].
    pub app: Option<&'a str>,
    /// Only consulted when `scope` is [`ElementScope::Window`] — ignored for
    /// [`ElementScope::Focused`] (the currently focused window's own title is
    /// used instead) and [`ElementScope::App`] (any of the app's windows do).
    pub title_contains: Option<&'a str>,
    pub name: Option<&'a str>,
    pub role: Option<&'a str>,
}

/// Whether any element matches the locator. Never errors — an unreachable
/// accessibility bus/API is indistinguishable from "nothing matched" here,
/// same as `window::window_exists`.
pub fn element_exists(locator: &ElementLocator) -> bool {
    imp::element_exists(locator)
}

/// Whether a matching element exists *and* is currently enabled. `false`
/// for "doesn't exist yet", "exists but disabled" and "AT backend
/// unreachable" alike — deliberately infallible so `wait until element is
/// enabled` can poll it the same way it polls `element_exists`, without
/// needing to first prove the element exists.
pub fn element_enabled(locator: &ElementLocator) -> bool {
    imp::element_enabled(locator)
}

/// Best-effort textual content of the first matching element: its text
/// contents if it exposes any (a label, a text field, ...), else its
/// current numeric value formatted as text (a slider, a progress bar, ...),
/// else its accessible name. `None` if nothing matched.
pub fn element_text(locator: &ElementLocator) -> Option<String> {
    imp::element_text(locator)
}

/// Invokes the first matching element's primary action (its "click"/
/// "press"/"activate" action if named as such, else whichever action the
/// platform lists first). `Ok(false)` means no matching element was found —
/// not an error, same spirit as `window::focus_window`.
pub fn click_element(locator: &ElementLocator) -> Result<bool, String> {
    imp::click_element(locator)
}

/// Sets the first matching element's text contents (for an editable text
/// field) or, if it isn't an editable-text element but does expose a
/// numeric value (a slider, a spin button, ...), its numeric value parsed
/// from `value`. `Ok(false)` means no matching element was found.
pub fn set_element_value(locator: &ElementLocator, value: &str) -> Result<bool, String> {
    imp::set_element_value(locator, value)
}

/// The element found at a screen point by the "pick from screen" element
/// picker, plus the window it was found in (so the caller can cache it —
/// see `element_at_point`) and that element's own screen-space bounding box
/// (for drawing a highlight over it).
#[derive(Debug, Clone)]
pub struct PickedElement {
    pub app: String,
    pub window_title: String,
    pub name: String,
    pub role: String,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub window: crate::window::WindowAtPoint,
}

/// Finds the accessibility element at screen point `(x, y)`, for the
/// element-picker feature. `None` only if no window is under the point at
/// all (or the window backend itself is unreachable — same never-errors
/// spirit as `element_exists`). When a window *is* found but no AT-SPI
/// element is (toolkits without an accessibility bridge, video surfaces,
/// gaps between controls), this returns a window-level pick instead: the
/// compositor window's app/title, empty name/role, and the window's own
/// bounds — so the overlay can still highlight something and a click still
/// fills the block's window fields rather than silently cancelling.
///
/// `known_window` is an optional cache hint: the caller (the picker's hover
/// loop) passes back the `window` field from the previous call's result,
/// and this skips re-resolving *which* window is under the cursor (the
/// expensive half of this call — see `crate::window::window_at_point`'s doc
/// comment) as long as `(x, y)` still falls inside it, only re-running that
/// lookup once the cursor actually leaves the last-known window.
pub fn element_at_point(x: i32, y: i32, known_window: Option<crate::window::WindowAtPoint>) -> Option<PickedElement> {
    imp::element_at_point(x, y, known_window)
}

#[cfg(target_os = "linux")]
#[path = "linux.rs"]
mod imp;

#[cfg(windows)]
mod imp {
    //! Not yet implemented — every call reports "nothing found"/"not
    //! supported" rather than panicking, same spirit as
    //! `backend::create_backend()`'s unsupported-platform fallback.
    use super::ElementLocator;

    pub fn element_exists(_locator: &ElementLocator) -> bool {
        false
    }
    pub fn element_enabled(_locator: &ElementLocator) -> bool {
        false
    }
    pub fn element_text(_locator: &ElementLocator) -> Option<String> {
        None
    }
    pub fn click_element(_locator: &ElementLocator) -> Result<bool, String> {
        Err("accessibility-tree automation is not yet supported on Windows".to_string())
    }
    pub fn set_element_value(_locator: &ElementLocator, _value: &str) -> Result<bool, String> {
        Err("accessibility-tree automation is not yet supported on Windows".to_string())
    }
    pub fn element_at_point(_x: i32, _y: i32, _known_window: Option<crate::window::WindowAtPoint>) -> Option<super::PickedElement> {
        None
    }
}

#[cfg(target_os = "macos")]
mod imp {
    //! Not yet implemented — see the `windows` stub above for the rationale.
    use super::ElementLocator;

    pub fn element_exists(_locator: &ElementLocator) -> bool {
        false
    }
    pub fn element_enabled(_locator: &ElementLocator) -> bool {
        false
    }
    pub fn element_text(_locator: &ElementLocator) -> Option<String> {
        None
    }
    pub fn click_element(_locator: &ElementLocator) -> Result<bool, String> {
        Err("accessibility-tree automation is not yet supported on macOS".to_string())
    }
    pub fn set_element_value(_locator: &ElementLocator, _value: &str) -> Result<bool, String> {
        Err("accessibility-tree automation is not yet supported on macOS".to_string())
    }
    pub fn element_at_point(_x: i32, _y: i32, _known_window: Option<crate::window::WindowAtPoint>) -> Option<super::PickedElement> {
        None
    }
}

#[cfg(not(any(target_os = "linux", windows, target_os = "macos")))]
mod imp {
    use super::ElementLocator;

    pub fn element_exists(_locator: &ElementLocator) -> bool {
        false
    }
    pub fn element_enabled(_locator: &ElementLocator) -> bool {
        false
    }
    pub fn element_text(_locator: &ElementLocator) -> Option<String> {
        None
    }
    pub fn click_element(_locator: &ElementLocator) -> Result<bool, String> {
        Err("accessibility-tree automation is not supported on this platform".to_string())
    }
    pub fn set_element_value(_locator: &ElementLocator, _value: &str) -> Result<bool, String> {
        Err("accessibility-tree automation is not supported on this platform".to_string())
    }
    pub fn element_at_point(_x: i32, _y: i32, _known_window: Option<crate::window::WindowAtPoint>) -> Option<super::PickedElement> {
        None
    }
}
