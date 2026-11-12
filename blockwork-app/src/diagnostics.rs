//! A plain report of what input this session can see and send, for the
//! Settings page and bug reports. Reading it changes nothing.
use blockwork_core::{config, recording};
use serde_json::{Value, json};

/// Opening /dev/uinput creates nothing until a device is configured on it.
fn writable(path: &str) -> bool {
    std::fs::OpenOptions::new().write(true).open(path).is_ok()
}

fn result<T: serde::Serialize>(value: Result<T, String>) -> Value {
    match value {
        Ok(value) => json!({ "ok": true, "value": value }),
        Err(error) => json!({ "ok": false, "error": error }),
    }
}

pub(crate) fn input() -> Value {
    let settings = config::load_settings();
    let monitors = blockwork_core::screen::monitors();
    let touch_playback = if cfg!(target_os = "linux") {
        "Linux uinput (experimental, one monitor)"
    } else if cfg!(windows) {
        "Windows pointer injection"
    } else {
        "unsupported"
    };
    let controllers = if settings.controller_input_enabled.unwrap_or(false) {
        result(blockwork_core::input::controllers::list().map(|list| {
            list.into_iter()
                .map(|c| json!({ "name": c.name, "profile": c.profile }))
                .collect::<Vec<_>>()
        }))
    } else {
        json!({ "ok": false, "error": "Controller input is off in Settings" })
    };
    let ui_automation = if settings.ui_automation_enabled.unwrap_or(false) {
        result(blockwork_core::accessibility::applications().map(|apps| apps.len()))
    } else {
        json!({ "ok": false, "error": "UI automation is off in Settings" })
    };
    json!({
        "version": env!("CARGO_PKG_VERSION"),
        "os": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "wayland": blockwork_core::macros::backend::is_wayland_session(),
        "monitors": monitors.len(),
        "capture": {
            "available": !recording::grab_failed(),
            "msSinceInput": recording::time_since_input().map(|d| d.as_millis() as u64),
        },
        "output": {
            "available": !blockwork_core::macros::runner::emulator_failed(),
            "absoluteMouse": blockwork_core::macros::backend::absolute_mouse_position_available(),
            "uinputWritable": cfg!(target_os = "linux") && writable("/dev/uinput"),
        },
        "recording": {
            "mouseMovement": settings.record_mouse_movement.unwrap_or(false),
            "relativeMouse": settings.record_mouse_relative.unwrap_or(true),
            "touch": settings.record_touch.unwrap_or(false),
            "controller": settings.record_controller.unwrap_or(false),
            "review": settings.review_recordings.unwrap_or(true),
            "touchscreens": recording::touch::devices(),
        },
        "touch": {
            "playback": touch_playback,
            "playbackNeedsOneMonitor": cfg!(target_os = "linux"),
        },
        "controllers": {
            "input": settings.controller_input_enabled.unwrap_or(false),
            "deadzone": settings.controller_deadzone.unwrap_or(0.12),
            "found": controllers,
            "virtualOutput": settings.virtual_gamepad_enabled.unwrap_or(false),
            "virtualOutputSupported": cfg!(target_os = "linux"),
        },
        "uiAutomation": {
            "enabled": settings.ui_automation_enabled.unwrap_or(false),
            "supported": cfg!(target_os = "linux"),
            "applications": ui_automation,
        },
        "startup": {
            "startAtStartup": settings.start_at_startup.unwrap_or(false),
            "entryVersion": settings.startup_entry_version,
        },
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_report_has_every_section_and_never_fails() {
        let dir = std::env::temp_dir().join(format!("blockwork-diag-{}", uuid::Uuid::new_v4()));
        blockwork_core::config::set_config_dir_override(dir.clone());
        let report = super::input();
        for key in [
            "os",
            "capture",
            "output",
            "recording",
            "touch",
            "controllers",
            "uiAutomation",
            "startup",
        ] {
            assert!(report.get(key).is_some(), "{key}");
        }
        assert_eq!(report["controllers"]["found"]["ok"], false);
        let _ = std::fs::remove_dir_all(dir);
    }
}
