//! Bounded native UI queries. Selectors are resolved again for every operation.
pub use crate::input::ui::{UiAction, UiScope, UiSelector};
use serde::Serialize;

/// One element a selector found, as the picker lists it.
#[derive(Debug, Clone, Serialize)]
pub struct UiNode {
    pub app: String,
    pub window: String,
    pub name: String,
    pub role: String,
    pub id: String,
    pub enabled: bool,
    pub selector: UiSelector,
}

/// What a native UI reporter reads from the element.
#[derive(Clone, Copy)]
pub enum Query {
    Exists,
    Enabled,
    Text,
    Value,
}

/// Native UI automation is opt-in; every entry point checks this first.
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

/// Reads one property of the element `selector` names.
pub fn query(
    selector: &UiSelector,
    query: Query,
) -> Result<crate::input::value::Evaluated, String> {
    enabled()?;
    selector.require_target()?;
    imp::query(selector, query)
}

/// Invokes, focuses or writes text or a value to the element.
pub fn action(selector: &UiSelector, action: UiAction, value: &str) -> Result<(), String> {
    enabled()?;
    selector.require_target()?;
    if value.len() > 65536 {
        return Err("Native UI text exceeds 64 KiB".into());
    }
    imp::action(selector, action, value)
}

/// A counter that moves when the accessibility tree reports a change.
pub fn event_revision() -> Result<u64, String> {
    enabled()?;
    imp::revision()
}

/// Every element `selector` matches, for the picker.
pub fn inspect(selector: &UiSelector) -> Result<Vec<UiNode>, String> {
    enabled()?;
    imp::inspect(selector)
}

/// Names of the apps on the accessibility bus.
pub fn applications() -> Result<Vec<String>, String> {
    enabled()?;
    imp::applications()
}

// AT-SPI on Linux; elsewhere every call reports the missing adapter.
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
}
