use crate::{daemon_client, native_icon, window_frame};
use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::mpsc;

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(QString, state_json, cxx_name = "stateJson")]
        #[qproperty(QString, last_error, cxx_name = "lastError")]
        #[qproperty(bool, connected)]
        #[qproperty(bool, should_quit, cxx_name = "shouldQuit")]
        #[qproperty(i32, focus_serial, cxx_name = "focusSerial")]
        #[qproperty(QString, installed_apps_json, cxx_name = "installedAppsJson")]
        #[qproperty(bool, installed_apps_loading, cxx_name = "installedAppsLoading")]
        #[qproperty(QString, installed_apps_error, cxx_name = "installedAppsError")]
        #[qproperty(i32, installed_apps_serial, cxx_name = "installedAppsSerial")]
        #[qproperty(QString, preview_key, cxx_name = "previewKey")]
        #[qproperty(QString, preview_text, cxx_name = "previewText")]
        #[qproperty(bool, preview_error, cxx_name = "previewError")]
        #[qproperty(i32, preview_serial, cxx_name = "previewSerial")]
        #[qproperty(QString, active_blocks_json, cxx_name = "activeBlocksJson")]
        type AppBridge = super::AppBridgeRust;

        #[qinvokable]
        fn start(self: Pin<&mut AppBridge>);

        #[qinvokable]
        fn poll(self: Pin<&mut AppBridge>);

        #[qinvokable]
        #[cxx_name = "invokeCommand"]
        fn invoke_command(self: Pin<&mut AppBridge>, command: &QString, arguments: &QString);

        /// Sample-evaluates a value block (JSON); the answer arrives through
        /// the `preview*` properties, tagged with `key`.
        #[qinvokable]
        #[cxx_name = "previewValue"]
        fn preview_value(self: Pin<&mut AppBridge>, key: &QString, value: &QString);

        #[qinvokable]
        #[cxx_name = "refreshInstalledApps"]
        fn refresh_installed_apps(self: Pin<&mut AppBridge>);

        /// Native file icon for an installed app's launch target as a PNG
        /// data: URI (or empty when none resolves). The backend ships icons
        /// only on some platforms; the picker and the Open/Close App blocks
        /// use this to fill the gap natively. Results are memoized.
        #[qinvokable]
        #[cxx_name = "nativeAppIcon"]
        fn native_app_icon(self: Pin<&mut AppBridge>, command: &QString) -> QString;

        /// Marks `margin` px around frameless windows as shadow, so the
        /// compositor moves and clamps the visible frame (Wayland).
        #[qinvokable]
        #[cxx_name = "setWindowFrameMargin"]
        fn set_window_frame_margin(self: &AppBridge, margin: i32);
    }
}

pub struct AppBridgeRust {
    state_json: QString,
    last_error: QString,
    connected: bool,
    should_quit: bool,
    focus_serial: i32,
    installed_apps_json: QString,
    installed_apps_loading: bool,
    installed_apps_error: QString,
    installed_apps_serial: i32,
    preview_key: QString,
    preview_text: QString,
    preview_error: bool,
    preview_serial: i32,
    active_blocks_json: QString,
    started: bool,
    request_tx: Option<tokio::sync::mpsc::UnboundedSender<daemon_client::Request>>,
    event_rx: Option<mpsc::Receiver<daemon_client::Event>>,
    native_icon_cache: HashMap<String, QString>,
}

impl Default for AppBridgeRust {
    fn default() -> Self {
        Self {
            state_json: QString::from("{}"),
            last_error: QString::default(),
            connected: false,
            should_quit: false,
            focus_serial: 0,
            installed_apps_json: QString::from("[]"),
            installed_apps_loading: false,
            installed_apps_error: QString::default(),
            installed_apps_serial: 0,
            preview_key: QString::default(),
            preview_text: QString::default(),
            preview_error: false,
            preview_serial: 0,
            active_blocks_json: QString::from("[]"),
            started: false,
            request_tx: None,
            event_rx: None,
            native_icon_cache: HashMap::new(),
        }
    }
}

impl qobject::AppBridge {
    pub fn start(mut self: Pin<&mut Self>) {
        if self.rust().started {
            return;
        }
        let (request_tx, request_rx) = tokio::sync::mpsc::unbounded_channel();
        let (event_tx, event_rx) = mpsc::channel();
        daemon_client::spawn(request_rx, event_tx);
        self.as_mut().rust_mut().started = true;
        self.as_mut().rust_mut().request_tx = Some(request_tx);
        self.as_mut().rust_mut().event_rx = Some(event_rx);
    }

    pub fn poll(mut self: Pin<&mut Self>) {
        let events: Vec<_> = self
            .as_mut()
            .rust_mut()
            .event_rx
            .as_ref()
            .map(|rx| rx.try_iter().collect())
            .unwrap_or_default();
        for event in events {
            match event {
                daemon_client::Event::Connected => self.as_mut().set_connected(true),
                daemon_client::Event::State(json) => {
                    self.as_mut().set_state_json(QString::from(&json));
                }
                daemon_client::Event::Error(error) => {
                    self.as_mut().set_last_error(QString::from(&error));
                }
                daemon_client::Event::Focus => {
                    let next = self.focus_serial().saturating_add(1);
                    self.as_mut().set_focus_serial(next);
                }
                daemon_client::Event::Quit => self.as_mut().set_should_quit(true),
                daemon_client::Event::Apps(json) => {
                    self.as_mut()
                        .set_installed_apps_json(QString::from(&json));
                    self.as_mut().set_installed_apps_loading(false);
                    self.as_mut()
                        .set_installed_apps_error(QString::default());
                    let next = self.installed_apps_serial().saturating_add(1);
                    self.as_mut().set_installed_apps_serial(next);
                }
                daemon_client::Event::Preview { key, text, error } => {
                    self.as_mut().set_preview_key(QString::from(&key));
                    self.as_mut().set_preview_text(QString::from(&text));
                    self.as_mut().set_preview_error(error);
                    let next = self.preview_serial().saturating_add(1);
                    self.as_mut().set_preview_serial(next);
                }
                daemon_client::Event::Trace(json) => {
                    self.as_mut().set_active_blocks_json(QString::from(&json));
                }
                daemon_client::Event::AppsError(error) => {
                    self.as_mut().set_installed_apps_loading(false);
                    self.as_mut()
                        .set_installed_apps_error(QString::from(&error));
                }
            }
        }
    }

    pub fn invoke_command(mut self: Pin<&mut Self>, command: &QString, arguments: &QString) {
        let command = command.to_string();
        let args_text = arguments.to_string();
        let args = match serde_json::from_str(&args_text) {
            Ok(value) => value,
            Err(error) => {
                self.as_mut().set_last_error(QString::from(&format!(
                    "Invalid command arguments: {error}"
                )));
                return;
            }
        };
        let sent = self.rust().request_tx.as_ref().is_some_and(|tx| {
            tx.send(daemon_client::Request::Invoke { command, args })
                .is_ok()
        });
        if !sent {
            self.as_mut().set_last_error(QString::from(
                "Blockwork is not connected to its background service",
            ));
        }
    }

    pub fn preview_value(mut self: Pin<&mut Self>, key: &QString, value: &QString) {
        let Ok(value) = serde_json::from_str(&value.to_string()) else {
            return;
        };
        let key = key.to_string();
        let sent = self
            .rust()
            .request_tx
            .as_ref()
            .is_some_and(|tx| tx.send(daemon_client::Request::Preview { key, value }).is_ok());
        if !sent {
            self.as_mut().set_last_error(QString::from(
                "Blockwork is not connected to its background service",
            ));
        }
    }

    pub fn refresh_installed_apps(mut self: Pin<&mut Self>) {
        self.as_mut().set_installed_apps_loading(true);
        self.as_mut()
            .set_installed_apps_error(QString::default());
        let sent = self
            .rust()
            .request_tx
            .as_ref()
            .is_some_and(|tx| tx.send(daemon_client::Request::FetchApps).is_ok());
        if !sent {
            self.as_mut().set_installed_apps_loading(false);
            self.as_mut().set_installed_apps_error(QString::from(
                "Blockwork is not connected to its background service",
            ));
        }
    }

    pub fn set_window_frame_margin(&self, margin: i32) {
        window_frame::set_margin(margin);
    }

    pub fn native_app_icon(mut self: Pin<&mut Self>, command: &QString) -> QString {
        const SIZE: i32 = 64;
        let key = command.to_string();
        if let Some(cached) = self.rust().native_icon_cache.get(&key) {
            return cached.clone();
        }
        let icon = QString::from(&native_icon::app_icon(&key, SIZE));
        self.as_mut()
            .rust_mut()
            .native_icon_cache
            .insert(key, icon.clone());
        icon
    }
}
