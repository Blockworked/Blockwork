//! Settings for the browser bridge in blockwork-core: where the Blockwork
//! browser extension connects to run the browser blocks.

use crate::state::{SharedState, emit_state_updated};
use crate::AppHandle;
use blockwork_core::browser;
use blockwork_core::config::update_settings;

pub(crate) fn start(state: SharedState, app: AppHandle) {
    browser::start(move || {
        if let Ok(s) = state.lock() {
            emit_state_updated(&app, &s);
        }
    });
}

pub(crate) fn configure(state: &SharedState, app: &AppHandle, enabled: bool, port: u16) -> Result<(), String> {
    if port == 0 {
        return Err("pick a port from 1 to 65535".into());
    }
    update_settings(|s| {
        s.browser_enabled = Some(enabled);
        s.browser_port = Some(port);
    });
    // Makes the key now, so it can be copied as soon as the switch is on.
    browser::key();
    browser::restart();
    publish(state, app);
    Ok(())
}

/// A new key unpairs every browser until it's pasted into the extension again.
pub(crate) fn new_key(state: &SharedState, app: &AppHandle) -> Result<(), String> {
    update_settings(|s| s.browser_key = Some(browser::fresh_key()));
    browser::restart();
    publish(state, app);
    Ok(())
}

fn publish(state: &SharedState, app: &AppHandle) {
    if let Ok(s) = state.lock() {
        emit_state_updated(app, &s);
    }
}
