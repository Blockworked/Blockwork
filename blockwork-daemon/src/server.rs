//! Serves UI connections on the daemon socket.

use crate::MainThreadEvent;
use blockwork_app::{Backend, Event};
use blockwork_protocol::{ClientMessage, DaemonMessage, FOCUSED_KEY_EVENT, Listener, encode};
use serde_json::value::RawValue;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use tao::event_loop::EventLoopProxy;
use tokio::io::{AsyncWriteExt, BufReader};
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::mpsc;

/// The currently connected UI, if any. Only one editor window is allowed at a
/// time - a second launch just brings the existing one to the front.
/// Also remembers the UI's `--ozone-platform` choice so tray relaunches keep
/// it until the daemon quits.
pub(crate) struct Clients {
    ui: Mutex<Option<(u64, mpsc::Sender<String>)>>,
    ozone_platform: Mutex<Option<String>>,
}

impl Default for Clients {
    fn default() -> Self {
        Self {
            ui: Mutex::new(None),
            ozone_platform: Mutex::new(blockwork_protocol::current_ozone_platform()),
        }
    }
}

impl Clients {
    /// Asks the connected UI to come to the front. Returns `false` if no UI is
    /// connected.
    pub(crate) fn focus_ui(&self) -> bool {
        let ui = self.ui.lock().unwrap();
        match ui.as_ref() {
            Some((_, tx)) => tx.try_send(encode(&DaemonMessage::focus())).is_ok(),
            None => false,
        }
    }

    fn set_ozone_platform(&self, ozone: Option<String>) {
        if ozone.is_some() {
            *self.ozone_platform.lock().unwrap() = ozone;
        }
    }

    fn ozone_platform(&self) -> Option<String> {
        self.ozone_platform.lock().unwrap().clone()
    }
}

pub(crate) async fn serve(
    listener: std::sync::Arc<Listener>,
    backend: Backend,
    clients: std::sync::Arc<Clients>,
    proxy: EventLoopProxy<MainThreadEvent>,
) {
    static NEXT_ID: AtomicU64 = AtomicU64::new(1);
    let slots = std::sync::Arc::new(tokio::sync::Semaphore::new(32));
    loop {
        match listener.accept().await {
            Ok(connection) => {
                let Ok(permit) = slots.clone().try_acquire_owned() else {
                    continue;
                };
                let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
                let backend = backend.clone();
                let clients = std::sync::Arc::clone(&clients);
                let proxy = proxy.clone();
                tokio::spawn(async move {
                    let _permit = permit;
                    let was_ui = handle_connection(id, connection, &backend, &clients).await;
                    if was_ui {
                        tracing::info!("UI disconnected");
                        if !backend.close_to_tray() {
                            let _ = proxy.send_event(MainThreadEvent::Quit);
                        }
                    }
                });
            }
            Err(e) => {
                tracing::warn!("Failed to accept a UI connection: {e}");
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        }
    }
}

async fn handle_connection(
    id: u64,
    connection: blockwork_protocol::Connection,
    backend: &Backend,
    clients: &Clients,
) -> bool {
    let (reader, mut writer) = tokio::io::split(connection);
    let mut reader = BufReader::new(reader);

    // Everything written to the socket goes through this one queue so replies
    // and pushed events never interleave mid-line.
    let (tx, mut rx) = mpsc::channel::<String>(32);
    let mut writer_task = tokio::spawn(async move {
        while let Some(line) = rx.recv().await {
            if line.len() > blockwork_protocol::MAX_MESSAGE_BYTES
                || !matches!(
                    tokio::time::timeout(
                        blockwork_protocol::WRITE_TIMEOUT,
                        writer.write_all(line.as_bytes())
                    )
                    .await,
                    Ok(Ok(()))
                )
            {
                break;
            }
        }
    });

    match tokio::time::timeout(
        blockwork_protocol::HANDSHAKE_TIMEOUT,
        blockwork_protocol::read_line(&mut reader),
    )
    .await
    {
        Ok(Ok(Some(line)))
            if matches!(serde_json::from_str(&line), Ok(ClientMessage::Hello { .. })) =>
        {
            if let Ok(ClientMessage::Hello { ozone_platform }) = serde_json::from_str(&line) {
                clients.set_ozone_platform(ozone_platform);
            }
        }
        _ => {
            writer_task.abort();
            return false;
        }
    }

    // Subscribe before taking the snapshot so no change can slip in between.
    let events = backend.subscribe();
    {
        let mut ui = clients.ui.lock().unwrap();
        match ui.as_ref() {
            Some((_, existing)) => {
                let _ = existing.try_send(encode(&DaemonMessage::focus()));
                let _ = tx.try_send(encode(&DaemonMessage::hello(false)));
            }
            None => *ui = Some((id, tx.clone())),
        }
    }
    if !clients
        .ui
        .lock()
        .unwrap()
        .as_ref()
        .is_some_and(|(ui_id, _)| *ui_id == id)
    {
        // Let the rejection reach the client before hanging up.
        drop(tx);
        let _ = writer_task.await;
        return false;
    }
    tracing::info!("UI connected");
    let _ = tx.try_send(encode(&DaemonMessage::hello(true)));
    send_state_snapshot(backend, &tx);
    let mut forwarder = tokio::spawn(forward_events(backend.clone(), events, tx.clone()));

    loop {
        let line = tokio::select! {
            _ = &mut writer_task => break,
            _ = &mut forwarder => break,
            line = blockwork_protocol::read_line(&mut reader) => match line {
                Ok(Some(line)) => line,
                _ => break,
            },
        };
        match serde_json::from_str::<ClientMessage>(&line) {
            // Handled one at a time, in order: commands mutate the same state
            // and the UI relies on them applying in the order it sent them.
            Ok(ClientMessage::Call { id, cmd, args }) => {
                let result = if cmd == FOCUSED_KEY_EVENT {
                    focused_key_event(&args)
                } else {
                    backend.dispatch(&cmd, args).await
                };
                if tx
                    .try_send(encode(&DaemonMessage::reply(id, result)))
                    .is_err()
                {
                    break;
                }
            }
            Ok(ClientMessage::Hello { ozone_platform }) => {
                clients.set_ozone_platform(ozone_platform);
            }
            Err(e) => tracing::warn!("Ignoring malformed message from the UI: {e}"),
        }
    }

    // The client is gone: stop writing to it and free the UI slot (which also
    // holds a sender, so the writer would otherwise never see its queue close).
    forwarder.abort();
    writer_task.abort();
    let mut ui = clients.ui.lock().unwrap();
    if ui.as_ref().is_some_and(|(ui_id, _)| *ui_id == id) {
        *ui = None;
    }
    true
}

fn focused_key_event(args: &serde_json::Value) -> Result<serde_json::Value, String> {
    let vk = args
        .get("vk")
        .and_then(|v| v.as_u64())
        .ok_or("missing vk")?;
    let pressed = args
        .get("pressed")
        .and_then(|v| v.as_bool())
        .ok_or("missing pressed")?;
    let vk = u16::try_from(vk).map_err(|e| e.to_string())?;
    Ok(blockwork_app::dispatch_from_focused_window(vk, pressed).into())
}

async fn forward_events(
    backend: Backend,
    mut events: tokio::sync::broadcast::Receiver<Event>,
    tx: mpsc::Sender<String>,
) {
    loop {
        let line = match events.recv().await {
            Ok(Event::State(json)) => match RawValue::from_string(json.to_string()) {
                Ok(raw) => encode(&DaemonMessage::state(&raw)),
                Err(_) => continue,
            },
            Ok(Event::Quit) => encode(&DaemonMessage::quit()),
            Ok(Event::CloseToTray(_)) => continue,
            // Fell behind: intermediate states are moot, send the latest.
            Err(RecvError::Lagged(_)) => {
                send_state_snapshot(&backend, &tx);
                continue;
            }
            Err(RecvError::Closed) => return,
        };
        if tx.try_send(line).is_err() {
            return;
        }
    }
}

fn send_state_snapshot(backend: &Backend, tx: &mpsc::Sender<String>) {
    match backend
        .state_json()
        .and_then(|json| RawValue::from_string(json).map_err(|e| e.to_string()))
    {
        Ok(raw) => {
            let _ = tx.try_send(encode(&DaemonMessage::state(&raw)));
        }
        Err(e) => tracing::warn!("Failed to snapshot state: {e}"),
    }
}

/// Launches the editor UI. The UI passes its own launch command down when it
/// starts the daemon (`BLOCKWORK_UI_EXE`) - inside Flatpak that's the
/// `/app/bin/blockwork` wrapper rather than the bare binary, and in a macOS
/// bundle the UI binary is named after the app. Replays the remembered
/// `--ozone-platform` choice, if any.
pub(crate) fn spawn_ui(clients: &Clients) {
    let ozone = clients.ozone_platform();
    let program = std::env::var_os("BLOCKWORK_UI_EXE")
        .map(std::path::PathBuf::from)
        .or_else(|| {
            let exe = std::env::current_exe().ok()?;
            Some(exe.with_file_name(format!("blockwork{}", std::env::consts::EXE_SUFFIX)))
        });
    let Some(program) = program else {
        tracing::warn!("Can't tell where the Blockwork UI binary is");
        return;
    };
    let mut command = std::process::Command::new(&program);
    if let Some(ozone) = ozone {
        command.arg(blockwork_protocol::ozone_platform_arg(&ozone));
    }
    match command.spawn() {
        // Reap it once it exits so it doesn't linger as a zombie.
        Ok(mut child) => {
            std::thread::spawn(move || child.wait());
        }
        Err(e) => tracing::warn!("Failed to launch the UI ({}): {e}", program.display()),
    }
}
