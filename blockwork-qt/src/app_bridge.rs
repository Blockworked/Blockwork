use crate::{daemon_client, native_icon, window_frame};
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};

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
        #[qproperty(QString, reply_key, cxx_name = "replyKey")]
        #[qproperty(QString, reply_json, cxx_name = "replyJson")]
        #[qproperty(QString, reply_error, cxx_name = "replyError")]
        #[qproperty(i32, reply_serial, cxx_name = "replySerial")]
        #[qproperty(i32, native_icons_serial, cxx_name = "nativeIconsSerial")]
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

        /// Runs a command whose result QML needs; it arrives through the
        /// `reply*` properties, tagged with `key`.
        #[qinvokable]
        #[cxx_name = "callCommand"]
        fn call_command(self: Pin<&mut AppBridge>, key: &QString, command: &QString, arguments: &QString);

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

        /// `nativeAppIcon` without blocking: the cached icon, or an empty
        /// string while a worker thread resolves it. `nativeIconsSerial`
        /// goes up when new icons are ready.
        #[qinvokable]
        #[cxx_name = "nativeAppIconAsync"]
        fn native_app_icon_async(self: Pin<&mut AppBridge>, command: &QString) -> QString;

        /// Marks `margin` px around frameless windows as shadow, so the
        /// compositor moves and clamps the visible frame (Wayland).
        #[qinvokable]
        #[cxx_name = "setWindowFrameMargin"]
        fn set_window_frame_margin(self: &AppBridge, margin: i32);
    }

    // The IPC thread queues `poll` here when a message arrives.
    impl cxx_qt::Threading for AppBridge {}
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
    reply_key: QString,
    reply_json: QString,
    reply_error: QString,
    reply_serial: i32,
    started: bool,
    request_tx: Option<tokio::sync::mpsc::UnboundedSender<daemon_client::Request>>,
    event_rx: Option<mpsc::Receiver<daemon_client::Event>>,
    /// Set while a `poll` is queued, so a burst of messages queues one.
    wake_pending: Arc<AtomicBool>,
    native_icon_cache: HashMap<String, QString>,
    native_icons_serial: i32,
    icon_worker: Option<IconWorker>,
}

/// One thread resolving native app icons, so a picker full of apps doesn't
/// stall the UI thread on shell calls and PNG encoding.
struct IconWorker {
    jobs: mpsc::Sender<String>,
    done: Arc<std::sync::Mutex<Vec<(String, String)>>>,
    pending: std::collections::HashSet<String>,
    wake_pending: Arc<AtomicBool>,
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
            reply_key: QString::default(),
            reply_json: QString::default(),
            reply_error: QString::default(),
            reply_serial: 0,
            started: false,
            request_tx: None,
            event_rx: None,
            wake_pending: Arc::new(AtomicBool::new(false)),
            native_icon_cache: HashMap::new(),
            native_icons_serial: 0,
            icon_worker: None,
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
        // Wake the UI thread when a message arrives instead of polling on a
        // timer: no added latency, and no wakeups while nothing happens.
        let pending = self.rust().wake_pending.clone();
        let thread = self.qt_thread();
        let wake: daemon_client::Wake = Arc::new(move || {
            if !pending.swap(true, Ordering::AcqRel) {
                let _ = thread.queue(|bridge| bridge.poll());
            }
        });
        daemon_client::spawn(request_rx, daemon_client::EventSink::new(event_tx, wake));
        self.as_mut().rust_mut().started = true;
        self.as_mut().rust_mut().request_tx = Some(request_tx);
        self.as_mut().rust_mut().event_rx = Some(event_rx);
    }

    pub fn poll(mut self: Pin<&mut Self>) {
        self.rust().wake_pending.store(false, Ordering::Release);
        let events: Vec<_> = self
            .as_mut()
            .rust_mut()
            .event_rx
            .as_ref()
            .map(|rx| rx.try_iter().collect())
            .unwrap_or_default();
        // Only the newest state and trace of a burst matter: each one is a
        // whole snapshot, and QML re-parses every state it is given.
        let last_state = events.iter().rposition(|e| matches!(e, daemon_client::Event::State(_)));
        let last_trace = events.iter().rposition(|e| matches!(e, daemon_client::Event::Trace(_)));
        for (index, event) in events.into_iter().enumerate() {
            match &event {
                daemon_client::Event::State(_) if Some(index) != last_state => continue,
                daemon_client::Event::Trace(_) if Some(index) != last_trace => continue,
                _ => {}
            }
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
                daemon_client::Event::Reply { key, json, error } => {
                    self.as_mut().set_reply_key(QString::from(&key));
                    self.as_mut().set_reply_json(QString::from(&json));
                    self.as_mut().set_reply_error(QString::from(&error));
                    let next = self.reply_serial().saturating_add(1);
                    self.as_mut().set_reply_serial(next);
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

    pub fn call_command(mut self: Pin<&mut Self>, key: &QString, command: &QString, arguments: &QString) {
        let key = key.to_string();
        let args = serde_json::from_str(&arguments.to_string()).unwrap_or(serde_json::Value::Null);
        let request = daemon_client::Request::Call { key: key.clone(), command: command.to_string(), args };
        let sent = self.rust().request_tx.as_ref().is_some_and(|tx| tx.send(request).is_ok());
        if !sent {
            self.as_mut().set_reply_key(QString::from(&key));
            self.as_mut().set_reply_json(QString::from("null"));
            self.as_mut().set_reply_error(QString::from("Blockwork is not connected to its background service"));
            let next = self.reply_serial().saturating_add(1);
            self.as_mut().set_reply_serial(next);
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

    pub fn native_app_icon_async(mut self: Pin<&mut Self>, command: &QString) -> QString {
        // Only the Windows shell lookup is slow and thread-safe; macOS asks
        // Qt, which wants the UI thread, and Linux has nothing to look up.
        if !cfg!(windows) {
            return self.native_app_icon(command);
        }
        let key = command.to_string();
        if let Some(cached) = self.rust().native_icon_cache.get(&key) {
            return cached.clone();
        }
        if self.rust().icon_worker.is_none() {
            let worker = self.as_mut().start_icon_worker();
            self.as_mut().rust_mut().icon_worker = Some(worker);
        }
        let mut rust = self.as_mut().rust_mut();
        let worker = rust.icon_worker.as_mut().unwrap();
        if worker.pending.insert(key.clone()) {
            let _ = worker.jobs.send(key);
        }
        QString::default()
    }

    fn start_icon_worker(self: Pin<&mut Self>) -> IconWorker {
        const SIZE: i32 = 64;
        let (jobs, rx) = mpsc::channel::<String>();
        let done = Arc::new(std::sync::Mutex::new(Vec::new()));
        let wake_pending = Arc::new(AtomicBool::new(false));
        let thread = self.qt_thread();
        let (out, flag) = (done.clone(), wake_pending.clone());
        std::thread::Builder::new()
            .name("blockwork-icons".into())
            .spawn(move || {
                for key in rx {
                    let icon = native_icon::app_icon(&key, SIZE);
                    out.lock().unwrap().push((key, icon));
                    if !flag.swap(true, Ordering::AcqRel) {
                        let _ = thread.queue(|bridge| bridge.take_icons());
                    }
                }
            })
            .expect("failed to start icon thread");
        IconWorker { jobs, done, pending: Default::default(), wake_pending }
    }

    fn take_icons(mut self: Pin<&mut Self>) {
        let ready: Vec<(String, String)> = {
            let mut rust = self.as_mut().rust_mut();
            let Some(worker) = rust.icon_worker.as_mut() else { return };
            worker.wake_pending.store(false, Ordering::Release);
            let ready = std::mem::take(&mut *worker.done.lock().unwrap());
            for (key, _) in &ready {
                worker.pending.remove(key);
            }
            ready
        };
        if ready.is_empty() {
            return;
        }
        for (key, icon) in ready {
            self.as_mut().rust_mut().native_icon_cache.insert(key, QString::from(&icon));
        }
        let next = self.native_icons_serial().saturating_add(1);
        self.as_mut().set_native_icons_serial(next);
    }
}
