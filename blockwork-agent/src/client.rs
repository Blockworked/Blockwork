//! Daemon client for agents: connect/spawn, Agent handshake with version
//! check, one `call` per tool. Mirrors `blockwork-qt/src/daemon_client.rs`.

use blockwork_protocol::{AGENT_PROTOCOL, ClientMessage, DaemonMessage, DaemonMessageKind, encode};
use serde_json::Value;
use std::io;
use std::time::{Duration, Instant};
use tokio::io::{AsyncWriteExt, BufReader};

const SPAWN_TIMEOUT: Duration = Duration::from_secs(10);

pub type AgentError = String;

pub struct DaemonClient {
    reader: BufReader<tokio::io::ReadHalf<blockwork_protocol::Connection>>,
    writer: tokio::io::WriteHalf<blockwork_protocol::Connection>,
    next_id: u64,
}

impl DaemonClient {
    pub async fn connect() -> io::Result<Self> {
        let connection = Self::connect_or_spawn().await?;
        let (reader, writer) = tokio::io::split(connection);
        let mut client = Self {
            reader: BufReader::new(reader),
            writer,
            next_id: 1,
        };
        client.handshake().await?;
        Ok(client)
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
                Self::spawn_daemon()?;
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
        let daemon_exe = Self::daemon_exe()?;
        let mut command = match &appimage {
            Some(appimage) => {
                let mut c = std::process::Command::new(appimage);
                c.arg("--daemon");
                c
            }
            None => std::process::Command::new(&daemon_exe),
        };
        command
            .arg("--background")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
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

    async fn handshake(&mut self) -> io::Result<()> {
        let hello = ClientMessage::Agent {
            client: format!("blockwork-agent/{}", env!("CARGO_PKG_VERSION")),
            protocol: AGENT_PROTOCOL,
        };
        self.writer.write_all(encode(&hello).as_bytes()).await?;
        let line = blockwork_protocol::read_line(&mut self.reader)
            .await?
            .ok_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "daemon hung up"))?;
        let message: DaemonMessage<'_> = serde_json::from_str(&line)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        match (message.kind, message.accepted) {
            (DaemonMessageKind::Hello, Some(true)) => Ok(()),
            (DaemonMessageKind::Hello, Some(false)) => Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "agent channel is disabled or protocol unsupported (enable it in Settings or set BLOCKWORK_AGENT=1)",
            )),
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "unexpected daemon handshake reply",
            )),
        }
    }

    /// Calls one agent command and returns its JSON result, or the raw
    /// structured-error string.
    pub async fn call(&mut self, cmd: &str, args: Value) -> Result<Value, String> {
        let id = self.next_id;
        self.next_id += 1;
        let msg = ClientMessage::Call {
            id,
            cmd: cmd.to_string(),
            args,
        };
        self.writer
            .write_all(encode(&msg).as_bytes())
            .await
            .map_err(|e| e.to_string())?;
        loop {
            let line = tokio::time::timeout(
                blockwork_protocol::WRITE_TIMEOUT,
                blockwork_protocol::read_line(&mut self.reader),
            )
            .await
            .map_err(|_| "daemon timed out".to_string())?
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "daemon hung up".to_string())?;
            let message: DaemonMessage<'_> =
                serde_json::from_str(&line).map_err(|e| e.to_string())?;
            if message.kind == DaemonMessageKind::Reply && message.id == Some(id) {
                if let Some(error) = message.error {
                    return Err(error);
                }
                return Ok(message.data.unwrap_or(Value::Null));
            }
            // Ignore State/Trace pushes (agents get none, but be lenient).
        }
    }
}
