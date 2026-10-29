//! Host policy is separate from saved preferences and per-macro grants.
//! In particular, importing desktop settings must not enable desktop hooks
//! or polling services inside an Android application process.

use blockwork_core::macros::backend::InputBackend;
use serde::Serialize;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, Serialize)]
pub struct Capability {
    pub supported: bool,
    pub ready: bool,
    pub reason: String,
}

impl Capability {
    pub fn ready() -> Self {
        Self {
            supported: true,
            ready: true,
            reason: String::new(),
        }
    }

    pub fn unavailable(supported: bool, reason: impl Into<String>) -> Self {
        Self {
            supported,
            ready: false,
            reason: reason.into(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Capabilities {
    pub platform: String,
    pub services: BTreeMap<String, Capability>,
}

impl Capabilities {
    pub fn desktop() -> Self {
        Self {
            platform: std::env::consts::OS.into(),
            services: [
                "desktop_input",
                "recording",
                "process_events",
                "clipboard_watch",
                "window_management",
                "browser",
                "schedules",
            ]
            .into_iter()
            .map(|name| (name.into(), Capability::ready()))
            .collect(),
        }
    }

    /// Supported Android integrations start unready. The native host publishes
    /// observed permission/service state; a macro grant never enables them.
    pub fn android() -> Self {
        let mut services = BTreeMap::new();
        for name in [
            "desktop_input",
            "recording",
            "process_events",
            "clipboard_watch",
            "window_management",
            "browser",
        ] {
            services.insert(
                name.into(),
                Capability::unavailable(false, "Unavailable on stock Android"),
            );
        }
        for name in ["touch", "elements", "screenshots", "picker", "foreground_events", "app_launch"] {
            services.insert(
                name.into(),
                Capability::unavailable(true, "Enable the Blockwork accessibility service"),
            );
        }
        for name in [
            "schedules",
            "exact_alarms",
        ] {
            services.insert(
                name.into(),
                Capability::unavailable(
                    true,
                    "Waiting for Android alarm readiness",
                ),
            );
        }
        Self {
            platform: "android".into(),
            services,
        }
    }
}

/// Startup services. Android events and alarms are delivered by the host,
/// rather than by the desktop's polling watchers.
#[derive(Clone, Copy, Debug)]
pub struct Watchers {
    pub desktop: bool,
    pub time: bool,
    pub webhook: bool,
}

pub struct HostConfig {
    pub capabilities: Capabilities,
    pub watchers: Watchers,
    pub input: Option<Arc<Mutex<dyn InputBackend>>>,
    pub prompt: Option<Arc<blockwork_core::prompt::Handler>>,
    pub desktop_integration: bool,
}

impl HostConfig {
    pub fn desktop() -> Self {
        Self {
            capabilities: Capabilities::desktop(),
            watchers: Watchers {
                desktop: true,
                time: true,
                webhook: true,
            },
            input: None,
            prompt: None,
            desktop_integration: true,
        }
    }

    pub fn android() -> Self {
        Self {
            capabilities: Capabilities::android(),
            watchers: Watchers {
                desktop: false,
                time: false,
                webhook: false,
            },
            input: None,
            prompt: None,
            desktop_integration: false,
        }
    }
}

impl Default for HostConfig {
    fn default() -> Self {
        if cfg!(target_os = "android") {
            Self::android()
        } else {
            Self::desktop()
        }
    }
}
