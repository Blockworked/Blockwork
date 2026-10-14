use blockwork_protocol::{ClientMessage, DaemonMessage, DaemonMessageKind, encode};
use serde_json::Value;
use std::io;
use std::sync::mpsc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncWriteExt, BufReader};

const SPAWN_TIMEOUT: Duration = Duration::from_secs(10);
const DAEMON_ARG: &str = "--daemon";

pub enum Request {
    Invoke { command: String, args: Value },
    FetchApps,
    /// Sample-evaluates a value block; answered by [`Event::Preview`] with the same `key`.
    Preview { key: String, value: Value },
}

pub enum Event {
    Connected,
    State(String),
    Error(String),
    Focus,
    Quit,
    Apps(String),
    AppsError(String),
    Preview { key: String, text: String, error: bool },
    /// JSON list of the blocks running right now.
    Trace(String),
}

pub fn spawn(requests: tokio::sync::mpsc::UnboundedReceiver<Request>, events: mpsc::Sender<Event>) {
    std::thread::Builder::new()
        .name("blockwork-qt-ipc".into())
        .spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("failed to start UI IPC runtime");
            if let Err(error) = runtime.block_on(run(requests, &events)) {
                let _ = events.send(Event::Error(error.to_string()));
                let _ = events.send(Event::Quit);
            }
        })
        .expect("failed to start UI IPC thread");
}

async fn run(
    mut requests: tokio::sync::mpsc::UnboundedReceiver<Request>,
    events: &mpsc::Sender<Event>,
) -> io::Result<()> {
    let connection = connect_or_spawn(false).await?;
    let (reader, mut writer) = tokio::io::split(connection);
    let mut reader = BufReader::new(reader);
    writer
        .write_all(encode(&ClientMessage::Hello).as_bytes())
        .await?;

    let line = tokio::time::timeout(
        blockwork_protocol::HANDSHAKE_TIMEOUT,
        blockwork_protocol::read_line(&mut reader),
    )
    .await??
    .ok_or_else(|| io::Error::other("daemon closed during handshake"))?;
    let hello: DaemonMessage = serde_json::from_str(&line).map_err(io::Error::other)?;
    if hello.kind != DaemonMessageKind::Hello || hello.accepted != Some(true) {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "another Blockwork window is already open",
        ));
    }
    let _ = events.send(Event::Connected);
    let mut next_id = 1_u64;
    let mut pending_apps_id: Option<u64> = None;
    let mut pending_previews: std::collections::HashMap<u64, String> = Default::default();

    loop {
        tokio::select! {
            request = requests.recv() => {
                let Some(request) = request else { return Ok(()) };
                match request {
                    Request::Invoke { command, args } => {
                        let line = encode(&ClientMessage::Call { id: next_id, cmd: command, args });
                        next_id = next_id.wrapping_add(1);
                        if line.len() > blockwork_protocol::MAX_MESSAGE_BYTES {
                            let _ = events.send(Event::Error("command is too large".into()));
                            continue;
                        }
                        writer.write_all(line.as_bytes()).await?;
                    }
                    Request::Preview { key, value } => {
                        let id = next_id;
                        next_id = next_id.wrapping_add(1);
                        let line = encode(&ClientMessage::Call {
                            id,
                            cmd: "preview_value".to_owned(),
                            args: serde_json::json!({ "value": value }),
                        });
                        if line.len() > blockwork_protocol::MAX_MESSAGE_BYTES {
                            let _ = events.send(Event::Preview { key, text: "too large".into(), error: true });
                            continue;
                        }
                        pending_previews.insert(id, key);
                        writer.write_all(line.as_bytes()).await?;
                    }
                    Request::FetchApps => {
                        let id = next_id;
                        next_id = next_id.wrapping_add(1);
                        let line = encode(&ClientMessage::Call {
                            id,
                            cmd: "list_installed_apps".to_owned(),
                            args: serde_json::Value::Object(Default::default()),
                        });
                        if line.len() > blockwork_protocol::MAX_MESSAGE_BYTES {
                            let _ = events.send(Event::AppsError("command is too large".into()));
                            continue;
                        }
                        pending_apps_id = Some(id);
                        eprintln!("[blockwork-qt] requesting installed apps (id {id})");
                        writer.write_all(line.as_bytes()).await?;
                    }
                }
            }
            line = blockwork_protocol::read_line(&mut reader) => {
                let Some(line) = line? else { return Err(io::Error::other("daemon disconnected")) };
                let message: DaemonMessage = match serde_json::from_str(&line) {
                    Ok(message) => message,
                    Err(error) => {
                        let _ = events.send(Event::Error(format!("invalid daemon message: {error}")));
                        continue;
                    }
                };
                match message.kind {
                    DaemonMessageKind::State => {
                        if let Some(payload) = message.payload {
                            let _ = events.send(Event::State(payload.get().to_owned()));
                        }
                    }
                    DaemonMessageKind::Reply => {
                        if let Some(key) = message.id.and_then(|id| pending_previews.remove(&id)) {
                            let event = match (message.error, message.data) {
                                (Some(error), _) => Event::Preview { key, text: error, error: true },
                                (None, Some(Value::String(text))) => Event::Preview { key, text, error: false },
                                (None, Some(other)) => Event::Preview { key, text: other.to_string(), error: false },
                                (None, None) => Event::Preview { key, text: String::new(), error: false },
                            };
                            let _ = events.send(event);
                        } else if pending_apps_id.is_some_and(|pending| message.id == Some(pending)) {
                            pending_apps_id = None;
                            if let Some(error) = message.error {
                                eprintln!("[blockwork-qt] installed apps request failed: {error}");
                                let _ = events.send(Event::AppsError(error));
                            } else if let Some(data) = message.data {
                                let json = data.to_string();
                                eprintln!("[blockwork-qt] installed apps reply: {} bytes", json.len());
                                let _ = events.send(Event::Apps(json));
                            } else {
                                eprintln!("[blockwork-qt] installed apps reply had no data");
                                let _ = events.send(Event::Apps("[]".to_owned()));
                            }
                        } else if let Some(error) = message.error {
                            let _ = events.send(Event::Error(error));
                        }
                    }
                    DaemonMessageKind::Focus => { let _ = events.send(Event::Focus); }
                    DaemonMessageKind::Trace => {
                        let json = message.data.map(|d| d.to_string()).unwrap_or_else(|| "[]".to_owned());
                        let _ = events.send(Event::Trace(json));
                    }
                    DaemonMessageKind::Quit => { let _ = events.send(Event::Quit); return Ok(()); }
                    DaemonMessageKind::Hello => {}
                }
            }
        }
    }
}

/// Connects to the daemon, starting it first if needed. `background` starts
/// it like a login autostart, so it stays up with no window open.
pub(crate) async fn connect_or_spawn(background: bool) -> io::Result<blockwork_protocol::Connection> {
    match blockwork_protocol::connect().await {
        Ok(connection) => Ok(connection),
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused
            ) =>
        {
            spawn_daemon(background)?;
            let deadline = Instant::now() + SPAWN_TIMEOUT;
            loop {
                match blockwork_protocol::connect().await {
                    Ok(connection) => return Ok(connection),
                    Err(error) if Instant::now() >= deadline => return Err(error),
                    Err(error)
                        if !matches!(
                            error.kind(),
                            io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused
                        ) =>
                    {
                        return Err(error);
                    }
                    Err(_) => tokio::time::sleep(Duration::from_millis(50)).await,
                }
            }
        }
        Err(error) => Err(error),
    }
}

fn daemon_exe() -> io::Result<std::path::PathBuf> {
    Ok(std::env::current_exe()?
        .with_file_name(format!("blockwork-daemon{}", std::env::consts::EXE_SUFFIX)))
}

fn spawn_daemon(background: bool) -> io::Result<()> {
    let ui_exe = std::env::current_exe()?;
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
    if background {
        // Started from the command line: keep its logs out of the terminal.
        command
            .arg("--background")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
    }
    command
        .env(
            "BLOCKWORK_UI_EXE",
            std::env::var_os("BLOCKWORK_UI_EXE")
                .or(appimage)
                .unwrap_or_else(|| ui_exe.into_os_string()),
        )
        .stdin(std::process::Stdio::null());

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
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
        command.creation_flags(0x0000_0008 | 0x0000_0200);
    }
    let mut child = command.spawn().map_err(|error| {
        io::Error::new(
            error.kind(),
            format!("failed to start {}: {error}", daemon_exe.display()),
        )
    })?;
    std::thread::spawn(move || child.wait());
    Ok(())
}
