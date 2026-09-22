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
}

pub enum Event {
    Connected,
    State(String),
    Error(String),
    Focus,
    Quit,
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
    let connection = connect_or_spawn().await?;
    let (reader, mut writer) = tokio::io::split(connection);
    let mut reader = BufReader::new(reader);
    writer
        .write_all(
            encode(&ClientMessage::Hello {
                ozone_platform: blockwork_protocol::current_ozone_platform(),
            })
            .as_bytes(),
        )
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

    loop {
        tokio::select! {
            request = requests.recv() => {
                let Some(Request::Invoke { command, args }) = request else { return Ok(()) };
                let line = encode(&ClientMessage::Call { id: next_id, cmd: command, args });
                next_id = next_id.wrapping_add(1);
                if line.len() > blockwork_protocol::MAX_MESSAGE_BYTES {
                    let _ = events.send(Event::Error("command is too large".into()));
                    continue;
                }
                writer.write_all(line.as_bytes()).await?;
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
                        if let Some(error) = message.error {
                            let _ = events.send(Event::Error(error));
                        }
                    }
                    DaemonMessageKind::Focus => { let _ = events.send(Event::Focus); }
                    DaemonMessageKind::Quit => { let _ = events.send(Event::Quit); return Ok(()); }
                    DaemonMessageKind::Hello => {}
                }
            }
        }
    }
}

async fn connect_or_spawn() -> io::Result<blockwork_protocol::Connection> {
    match blockwork_protocol::connect().await {
        Ok(connection) => Ok(connection),
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused
            ) =>
        {
            spawn_daemon()?;
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

fn spawn_daemon() -> io::Result<()> {
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
    if let Some(ozone) = blockwork_protocol::current_ozone_platform() {
        command.arg(blockwork_protocol::ozone_platform_arg(&ozone));
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
