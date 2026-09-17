//! Loopback TCP control interface so external processes (e.g. a game running
//! under Proton) can trigger recording/playback at a
//! precise moment without a physical hotkey press. Commands translate 1:1
//! onto the existing hotkey `QueueSignal` pipeline.

use crate::hotkey_types::HotkeyAction;
use crate::recording::{self, QueueSignal};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::watch;
use tracing::{info, warn};

#[derive(Deserialize)]
struct IpcRequest {
    cmd: String,
    id: Option<String>,
}

#[derive(Serialize)]
struct IpcResponse {
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pong: Option<bool>,
}

impl IpcResponse {
    fn ok() -> Self {
        IpcResponse { ok: true, error: None, pong: None }
    }

    fn pong() -> Self {
        IpcResponse { ok: true, error: None, pong: Some(true) }
    }

    fn error(message: impl Into<String>) -> Self {
        IpcResponse { ok: false, error: Some(message.into()), pong: None }
    }

    fn to_line(&self) -> String {
        let mut line = serde_json::to_string(self).unwrap_or_else(|_| "{\"ok\":false}".to_string());
        line.push('\n');
        line
    }
}

/// Binds a loopback-only listener and serves connections until `shutdown` is
/// set to `true`. 127.0.0.1 only — this interface trusts any local process,
/// same as the global-hotkey grab. `shutdown` is forwarded into each
/// per-connection task too, not just the accept loop, so already-connected
/// clients get closed rather than serviced forever.
pub async fn run_server(port: u16, mut shutdown: watch::Receiver<bool>) {
    let listener = match TcpListener::bind(("127.0.0.1", port)).await {
        Ok(listener) => listener,
        Err(err) => {
            warn!("IPC: failed to bind 127.0.0.1:{port}: {err}");
            return;
        }
    };
    info!("IPC: listening on 127.0.0.1:{port}");

    loop {
        tokio::select! {
            accept_result = listener.accept() => {
                match accept_result {
                    Ok((socket, _addr)) => {
                        // Commands are small and latency-sensitive (a game
                        // triggers recording at an exact frame); Nagle's
                        // algorithm otherwise causes intermittent tens-of-ms
                        // stalls buffering them against delayed ACKs.
                        if let Err(err) = socket.set_nodelay(true) {
                            warn!("IPC: failed to set TCP_NODELAY: {err}");
                        }
                        tokio::spawn(handle_connection(socket, shutdown.clone()));
                    }
                    Err(err) => warn!("IPC: accept error: {err}"),
                }
            }
            _ = shutdown.changed() => {
                if *shutdown.borrow() {
                    info!("IPC: server stopping");
                    return;
                }
            }
        }
    }
}

async fn handle_connection(socket: TcpStream, mut shutdown: watch::Receiver<bool>) {
    let (read_half, mut write_half) = socket.into_split();
    let mut lines = BufReader::new(read_half).lines();

    loop {
        tokio::select! {
            line_result = lines.next_line() => {
                let line = match line_result {
                    Ok(Some(line)) => line,
                    Ok(None) => return,
                    Err(err) => {
                        warn!("IPC: read error: {err}");
                        return;
                    }
                };
                if line.trim().is_empty() {
                    continue;
                }

                let response = handle_request(&line);
                if write_half.write_all(response.to_line().as_bytes()).await.is_err() {
                    return;
                }
            }
            _ = shutdown.changed() => {
                if *shutdown.borrow() {
                    return;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncReadExt;

    /// Regression test: "Stop Server" used to leave already-connected clients
    /// running forever, since aborting the accept loop doesn't touch the
    /// separately spawned per-connection tasks.
    #[tokio::test]
    async fn shutdown_closes_already_connected_clients() {
        let port = 58231;
        let (tx, rx) = watch::channel(false);
        let server = tokio::spawn(run_server(port, rx));
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        let mut stream = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        stream.write_all(b"{\"cmd\":\"ping\"}\n").await.unwrap();
        let mut buf = [0u8; 64];
        let n = stream.read(&mut buf).await.unwrap();
        assert!(n > 0, "expected a pong response before shutdown");

        tx.send(true).unwrap();

        let mut buf2 = [0u8; 64];
        let read = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            stream.read(&mut buf2),
        )
        .await
        .expect("connection was not closed after shutdown")
        .unwrap();
        assert_eq!(read, 0, "connection should be closed (EOF) after shutdown");

        server.abort();
    }
}

fn handle_request(line: &str) -> IpcResponse {
    let request: IpcRequest = match serde_json::from_str(line) {
        Ok(request) => request,
        Err(err) => return IpcResponse::error(format!("invalid json: {err}")),
    };

    match request.cmd.as_str() {
        "start_recording" => {
            recording::push_queue_signal(QueueSignal::Hotkey(HotkeyAction::StartRecordingImmediate));
            IpcResponse::ok()
        }
        "stop_recording" => {
            recording::push_queue_signal(QueueSignal::Stop);
            IpcResponse::ok()
        }
        "run_macro" => {
            let action = match request.id {
                Some(id) => HotkeyAction::RunSpecificMacro(id),
                None => HotkeyAction::RunMacro,
            };
            recording::push_queue_signal(QueueSignal::Hotkey(action));
            IpcResponse::ok()
        }
        "stop_loop" => {
            recording::push_queue_signal(QueueSignal::Hotkey(HotkeyAction::StopLoop));
            IpcResponse::ok()
        }
        "ping" => IpcResponse::pong(),
        other => IpcResponse::error(format!("unknown command: {other}")),
    }
}
