//! The UI's connection to `blockwork-daemon`, which owns all app state (see
//! `blockwork-protocol`). Every frontend command is forwarded to it; state
//! updates, "focus" and "quit" come back the other way.

use blockwork_protocol::{ClientMessage, Connection, DaemonMessage, DaemonMessageKind, encode};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::{AsyncWriteExt, BufReader, ReadHalf, WriteHalf};
use tokio::sync::{mpsc, oneshot};

type Reader = BufReader<ReadHalf<Connection>>;
type PendingReply = oneshot::Sender<Result<Value, String>>;

/// How long to wait for a freshly spawned daemon to start listening.
const SPAWN_TIMEOUT: Duration = Duration::from_secs(10);

pub(crate) struct Daemon {
    outgoing: mpsc::Sender<String>,
    pending: Mutex<HashMap<u64, PendingReply>>,
    next_id: AtomicU64,
}

struct PendingCall<'a> {
    daemon: &'a Daemon,
    id: u64,
}
impl Drop for PendingCall<'_> {
    fn drop(&mut self) {
        self.daemon.pending.lock().unwrap().remove(&self.id);
    }
}

/// Result of [`connect`].
pub(crate) enum Connected {
    /// This is the app's UI; `reader` must be handed to [`pump`].
    Ui(std::sync::Arc<Daemon>, Reader),
    /// Another UI is already open and has been brought to the front.
    AlreadyOpen,
}

/// Connects to the running daemon, starting one first if there isn't any.
pub(crate) async fn connect() -> std::io::Result<Connected> {
    let connection = match blockwork_protocol::connect().await {
        Ok(connection) => connection,
        Err(e)
            if matches!(
                e.kind(),
                std::io::ErrorKind::NotFound | std::io::ErrorKind::ConnectionRefused
            ) =>
        {
            spawn_daemon()?;
            let deadline = Instant::now() + SPAWN_TIMEOUT;
            loop {
                match tokio::time::timeout(
                    deadline.saturating_duration_since(Instant::now()),
                    blockwork_protocol::connect(),
                )
                .await?
                {
                    Ok(connection) => break connection,
                    Err(e) if Instant::now() >= deadline => return Err(e),
                    Err(e)
                        if !matches!(
                            e.kind(),
                            std::io::ErrorKind::NotFound | std::io::ErrorKind::ConnectionRefused
                        ) =>
                    {
                        return Err(e);
                    }
                    Err(_) => tokio::time::sleep(Duration::from_millis(50)).await,
                }
            }
        }
        Err(e) => return Err(e),
    };

    let (reader, writer) = tokio::io::split(connection);
    let mut reader = BufReader::new(reader);
    let (outgoing, rx) = mpsc::channel(32);
    tokio::spawn(write_loop(writer, rx));

    let _ = outgoing.try_send(encode(&ClientMessage::Hello));
    let line = tokio::time::timeout(
        blockwork_protocol::HANDSHAKE_TIMEOUT,
        blockwork_protocol::read_line(&mut reader),
    )
    .await??
    .ok_or_else(|| blockwork_protocol::protocol_error("daemon closed the connection"))?;
    let hello: DaemonMessage = serde_json::from_str(&line).map_err(std::io::Error::other)?;
    if hello.kind != DaemonMessageKind::Hello {
        return Err(blockwork_protocol::protocol_error(
            "expected hello from daemon",
        ));
    }
    if hello.accepted != Some(true) {
        return Ok(Connected::AlreadyOpen);
    }

    let daemon = Daemon {
        outgoing,
        pending: Mutex::new(HashMap::new()),
        next_id: AtomicU64::new(1),
    };
    Ok(Connected::Ui(std::sync::Arc::new(daemon), reader))
}

async fn write_loop(mut writer: WriteHalf<Connection>, mut rx: mpsc::Receiver<String>) {
    while let Some(line) = rx.recv().await {
        if !matches!(
            tokio::time::timeout(
                blockwork_protocol::WRITE_TIMEOUT,
                writer.write_all(line.as_bytes())
            )
            .await,
            Ok(Ok(()))
        ) {
            return;
        }
    }
}

impl Daemon {
    /// Runs backend command `cmd` in the daemon and waits for its result.
    pub(crate) async fn call(&self, cmd: String, args: Value) -> Result<Value, String> {
        self.send_call(cmd, args)
            .await
            .map_err(|_| "lost connection to the Blockwork daemon".to_string())?
    }

    /// Like [`Daemon::call`], but blocks the calling thread for at most
    /// `timeout`. For callbacks that must answer synchronously.
    #[cfg_attr(not(windows), allow(dead_code))]
    pub(crate) fn call_blocking(
        &self,
        cmd: String,
        args: Value,
        timeout: Duration,
    ) -> Option<Value> {
        let (id, rx) = self.register_call(cmd, args)?;
        let _pending = PendingCall { daemon: self, id };
        let result = tauri::async_runtime::block_on(async {
            tokio::time::timeout(timeout, rx).await.ok()?.ok()?.ok()
        });
        result
    }

    fn register_call(
        &self,
        cmd: String,
        args: Value,
    ) -> Option<(u64, oneshot::Receiver<Result<Value, String>>)> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();
        let mut pending = self.pending.lock().unwrap();
        if pending.len() >= 64 {
            return None;
        }
        let line = encode(&ClientMessage::Call { id, cmd, args });
        if line.len() > blockwork_protocol::MAX_MESSAGE_BYTES {
            return None;
        }
        pending.insert(id, tx);
        drop(pending);
        if self.outgoing.try_send(line).is_err() {
            self.pending.lock().unwrap().remove(&id);
            return None;
        }
        Some((id, rx))
    }

    async fn send_call(&self, cmd: String, args: Value) -> Result<Result<Value, String>, ()> {
        let (id, rx) = self.register_call(cmd, args).ok_or(())?;
        let _pending = PendingCall { daemon: self, id };
        let result = tokio::time::timeout(Duration::from_secs(60), rx).await;
        match result {
            Ok(reply) => reply.map_err(|_| ()),
            Err(_) => Ok(Err("daemon request timed out".into())),
        }
    }
}

/// Reads daemon messages for the life of the UI. The UI can't do anything
/// without the daemon, so losing it closes the app.
pub(crate) async fn pump(app: AppHandle, daemon: std::sync::Arc<Daemon>, mut reader: Reader) {
    while let Ok(Some(line)) = blockwork_protocol::read_line(&mut reader).await {
        let message: DaemonMessage = match serde_json::from_str(&line) {
            Ok(message) => message,
            Err(e) => {
                tracing::warn!("Ignoring malformed message from the daemon: {e}");
                continue;
            }
        };
        match message.kind {
            DaemonMessageKind::Reply => {
                let Some(id) = message.id else { continue };
                if let Some(tx) = daemon.pending.lock().unwrap().remove(&id) {
                    let result = match message.error {
                        Some(error) => Err(error),
                        None => Ok(message.data.unwrap_or(Value::Null)),
                    };
                    let _ = tx.send(result);
                }
            }
            DaemonMessageKind::State => {
                if let Some(payload) = message.payload {
                    let _ = app.emit("state-updated", payload);
                }
            }
            DaemonMessageKind::Focus => {
                let app = app.clone();
                let _ = app.clone().run_on_main_thread(move || {
                    if let Some(window) = app.webview_windows().into_values().next() {
                        let _ = window.unminimize();
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                });
            }
            DaemonMessageKind::Quit => break,
            DaemonMessageKind::Hello => {}
        }
    }
    daemon.pending.lock().unwrap().clear();
    app.exit(0);
}

/// Argument that makes `blockwork` run the daemon instead of the UI.
pub(crate) const DAEMON_ARG: &str = "--daemon";

fn daemon_exe() -> std::io::Result<std::path::PathBuf> {
    Ok(std::env::current_exe()?
        .with_file_name(format!("blockwork-daemon{}", std::env::consts::EXE_SUFFIX)))
}

/// Replaces this process with the `blockwork-daemon` sitting next to it.
///
/// Only needed inside an AppImage: its files are only mounted while the
/// process that launched the AppImage runs, so a daemon started straight from
/// the mount would lose its own binary once the UI exits. Starting it as
/// `$APPIMAGE --daemon` instead gives it a mount of its own.
pub(crate) fn exec_daemon() -> ! {
    let error = match daemon_exe() {
        #[cfg(unix)]
        Ok(exe) => {
            use std::os::unix::process::CommandExt;
            std::process::Command::new(&exe)
                .args(std::env::args_os().skip(2))
                .exec()
        }
        #[cfg(not(unix))]
        Ok(exe) => match std::process::Command::new(&exe)
            .args(std::env::args_os().skip(2))
            .status()
        {
            Ok(status) => std::process::exit(status.code().unwrap_or(1)),
            Err(e) => e,
        },
        Err(e) => e,
    };
    eprintln!("Failed to start blockwork-daemon: {error}");
    std::process::exit(1);
}

/// Starts `blockwork-daemon` from next to this executable, detached so it
/// outlives the UI (and the terminal it may have been started from).
fn spawn_daemon() -> std::io::Result<()> {
    let exe = std::env::current_exe()?;
    let appimage = std::env::var_os("APPIMAGE");
    let daemon_exe = daemon_exe()?;
    let mut command = match &appimage {
        Some(appimage) => {
            let mut command = std::process::Command::new(appimage);
            command.arg(DAEMON_ARG);
            command
        }
        None => std::process::Command::new(&daemon_exe),
    };

    // Tells the daemon how to relaunch this UI from the tray: the AppImage
    // itself when running from one, and the Flatpak wrapper sets this to
    // itself, since the bare binary skips zypak.
    let ui_exe = std::env::var_os("BLOCKWORK_UI_EXE")
        .or(appimage)
        .unwrap_or_else(|| exe.into_os_string());
    command
        .env("BLOCKWORK_UI_EXE", ui_exe)
        .stdin(std::process::Stdio::null());

    // Debug builds keep logging to the launching terminal; release builds
    // log to a file, since the daemon outlives whatever launched it.
    if !cfg!(debug_assertions) {
        if let Some(log) = daemon_log_file() {
            command.stdout(log.try_clone()?).stderr(log);
        } else {
            command
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null());
        }
    }

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // SAFETY: `setsid` is async-signal-safe and touches no Rust state.
        unsafe {
            command.pre_exec(|| {
                libc::setsid();
                Ok(())
            });
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        command.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
    }

    let mut child = command.spawn().map_err(|e| {
        std::io::Error::new(
            e.kind(),
            format!("failed to start {}: {e}", daemon_exe.display()),
        )
    })?;
    // Reap it if it exits while this UI is still running.
    std::thread::spawn(move || child.wait());
    Ok(())
}

fn daemon_log_file() -> Option<std::fs::File> {
    let dir = dirs::cache_dir()?.join("blockwork");
    std::fs::create_dir_all(&dir).ok()?;
    std::fs::File::create(dir.join("daemon.log")).ok()
}
