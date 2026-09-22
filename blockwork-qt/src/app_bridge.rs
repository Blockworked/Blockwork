use crate::daemon_client;
use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;
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
        type AppBridge = super::AppBridgeRust;

        #[qinvokable]
        fn start(self: Pin<&mut AppBridge>);

        #[qinvokable]
        fn poll(self: Pin<&mut AppBridge>);

        #[qinvokable]
        #[cxx_name = "invokeCommand"]
        fn invoke_command(self: Pin<&mut AppBridge>, command: &QString, arguments: &QString);
    }
}

pub struct AppBridgeRust {
    state_json: QString,
    last_error: QString,
    connected: bool,
    should_quit: bool,
    focus_serial: i32,
    started: bool,
    request_tx: Option<tokio::sync::mpsc::UnboundedSender<daemon_client::Request>>,
    event_rx: Option<mpsc::Receiver<daemon_client::Event>>,
}

impl Default for AppBridgeRust {
    fn default() -> Self {
        Self {
            state_json: QString::from("{}"),
            last_error: QString::default(),
            connected: false,
            should_quit: false,
            focus_serial: 0,
            started: false,
            request_tx: None,
            event_rx: None,
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
}
