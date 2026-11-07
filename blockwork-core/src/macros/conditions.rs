//! The model's per-macro conditions, checked against this machine.

pub use blockwork_model::macros::conditions::*;

use super::MacroSettings;

/// Why a trigger or hotkey can't start the macro right now, or `None` if it can.
pub fn blocked_by(settings: &MacroSettings) -> Option<String> {
    if !settings.has_conditions() {
        return None;
    }
    settings.check(
        &chrono::Local::now(),
        crate::battery::is_plugged_in,
        |pattern| {
            crate::window::active().is_ok_and(|w| w.matches(pattern, crate::window::MatchBy::Any))
        },
    )
}
