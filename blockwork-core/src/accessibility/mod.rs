//! Bounded native UI queries. Selectors are resolved again for every operation.
pub use crate::input::ui::{UiAction, UiScope, UiSelector};
use serde::Serialize;
#[derive(Debug, Clone, Serialize)]
pub struct UiNode {
    pub app: String,
    pub window: String,
    pub name: String,
    pub role: String,
    pub id: String,
    pub enabled: bool,
    pub selector: UiSelector,
    /// `selector` as block text.
    pub text: String,
    /// How many elements `selector` matches; 1 means a block can use it.
    pub matches: usize,
}
/// What is under a screen point, for the element picker.
#[derive(Debug, Clone, Serialize)]
pub struct UiHit {
    pub app: String,
    pub window: String,
    pub name: String,
    pub role: String,
    pub enabled: bool,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    /// The window doesn't expose an element there, only itself.
    pub window_only: bool,
}
/// A clicked element and the selector that finds it again.
#[derive(Debug, Clone, Serialize)]
pub struct UiPick {
    pub hit: UiHit,
    /// None when only the window was found.
    pub node: Option<UiNode>,
}
#[derive(Clone, Copy)]
pub enum Query {
    Exists,
    Enabled,
    Text,
    Value,
}
pub fn enabled() -> Result<(), String> {
    if crate::config::load_settings()
        .ui_automation_enabled
        .unwrap_or(false)
    {
        Ok(())
    } else {
        Err("Native UI automation is disabled in Settings".into())
    }
}
pub fn query(
    selector: &UiSelector,
    query: Query,
) -> Result<crate::input::value::Evaluated, String> {
    enabled()?;
    selector.require_target()?;
    imp::query(selector, query)
}
pub fn action(selector: &UiSelector, action: UiAction, value: &str) -> Result<(), String> {
    enabled()?;
    selector.require_target()?;
    if value.len() > 65536 {
        return Err("Native UI text exceeds 64 KiB".into());
    }
    imp::action(selector, action, value)
}
pub fn event_revision() -> Result<u64, String> {
    enabled()?;
    imp::revision()
}
pub fn inspect(selector: &UiSelector) -> Result<Vec<UiNode>, String> {
    enabled()?;
    imp::inspect(selector)
}
pub fn applications() -> Result<Vec<String>, String> {
    enabled()?;
    imp::applications()
}
/// The element under screen point (`x`, `y`), skipping windows titled
/// `ignore` (the picker's own overlay).
pub fn element_at(x: f64, y: f64, ignore: &str) -> Result<Option<UiHit>, String> {
    enabled()?;
    imp::element_at(x, y, ignore)
}
/// Like `element_at`, plus the shortest selector that finds it alone.
pub fn pick(x: f64, y: f64, ignore: &str) -> Result<Option<UiPick>, String> {
    enabled()?;
    imp::pick(x, y, ignore)
}
#[cfg(target_os = "linux")]
#[path = "linux.rs"]
mod imp;
#[cfg(not(target_os = "linux"))]
mod imp {
    use super::*;
    pub fn revision() -> Result<u64, String> {
        Err(unsupported())
    }
    fn unsupported() -> String {
        "Native UI automation currently requires Linux AT-SPI; Windows UI Automation and macOS accessibility adapters are not available".into()
    }
    pub fn query(_: &UiSelector, _: Query) -> Result<crate::input::value::Evaluated, String> {
        Err(unsupported())
    }
    pub fn action(_: &UiSelector, _: UiAction, _: &str) -> Result<(), String> {
        Err(unsupported())
    }
    pub fn inspect(_: &UiSelector) -> Result<Vec<UiNode>, String> {
        Err(unsupported())
    }
    pub fn applications() -> Result<Vec<String>, String> {
        Err(unsupported())
    }
    pub fn element_at(_: f64, _: f64, _: &str) -> Result<Option<UiHit>, String> {
        Err(unsupported())
    }
    pub fn pick(_: f64, _: f64, _: &str) -> Result<Option<UiPick>, String> {
        Err(unsupported())
    }
}
