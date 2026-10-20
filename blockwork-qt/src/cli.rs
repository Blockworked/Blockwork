//! `blockwork list|run|stop`: drives macros through the daemon without
//! opening a window, for scripts and launchers.

use crate::daemon_client::connect_or_spawn;
use blockwork_protocol::{ClientMessage, DaemonMessage, DaemonMessageKind, encode};
use serde_json::{Value, json};
use std::io;
use std::time::Duration;
use tokio::io::{AsyncWriteExt, BufReader};

const USAGE: &str = "\
Usage:
  blockwork                    Open the editor
  blockwork list               List macros (* = running)
  blockwork run <name> [--wait]  Run a macro; --wait returns when it ends
  blockwork stop <name>        Stop every run of a macro";

/// The command line's subcommand, if it has one: the window isn't opened then.
pub fn requested() -> Option<Vec<String>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let first = args.first()?;
    matches!(first.as_str(), "list" | "run" | "stop" | "help" | "--help" | "-h").then_some(args)
}

/// Runs the subcommand and returns the process exit code.
pub fn main(args: Vec<String>) -> i32 {
    attach_console();
    let runtime = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
        Ok(runtime) => runtime,
        Err(e) => {
            eprintln!("blockwork: {e}");
            return 1;
        }
    };
    match runtime.block_on(run(&args)) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("blockwork: {e}");
            1
        }
    }
}

async fn run(args: &[String]) -> io::Result<()> {
    let name = || {
        let name = args[1..].iter().filter(|a| !a.starts_with("--")).cloned().collect::<Vec<_>>().join(" ");
        if name.is_empty() { Err(io::Error::other(format!("missing macro name\n{USAGE}"))) } else { Ok(name) }
    };
    match args[0].as_str() {
        "list" => {
            let mut daemon = Daemon::connect().await?;
            for mac in daemon.list().await? {
                let running = mac["running"].as_bool().unwrap_or(false);
                println!("{} {}", if running { "*" } else { " " }, mac["name"].as_str().unwrap_or_default());
            }
        }
        "run" => {
            let name = name()?;
            let mut daemon = Daemon::connect().await?;
            daemon.call("run_macro_named", json!({ "name": name })).await?;
            if args.iter().any(|a| a == "--wait") {
                // Give the run a moment to register before polling for its end.
                tokio::time::sleep(Duration::from_millis(100)).await;
                while daemon.list().await?.iter().any(|m| {
                    m["running"] == true && m["name"].as_str().is_some_and(|n| n.eq_ignore_ascii_case(name.trim()))
                }) {
                    tokio::time::sleep(Duration::from_millis(200)).await;
                }
            }
        }
        "stop" => {
            let name = name()?;
            let stopped = Daemon::connect().await?.call("stop_macro_named", json!({ "name": name })).await?;
            if stopped.as_u64() == Some(0) {
                println!("\"{name}\" wasn't running");
            }
        }
        _ => println!("{USAGE}"),
    }
    Ok(())
}

struct Daemon {
    reader: BufReader<tokio::io::ReadHalf<blockwork_protocol::Connection>>,
    writer: tokio::io::WriteHalf<blockwork_protocol::Connection>,
    next_id: u64,
}

impl Daemon {
    async fn connect() -> io::Result<Daemon> {
        let (reader, mut writer) = tokio::io::split(connect_or_spawn(true).await?);
        writer.write_all(encode(&ClientMessage::Script).as_bytes()).await?;
        let mut daemon = Daemon { reader: BufReader::new(reader), writer, next_id: 1 };
        let hello = daemon.read().await?;
        let hello: DaemonMessage = serde_json::from_str(&hello).map_err(io::Error::other)?;
        if hello.kind != DaemonMessageKind::Hello || hello.accepted != Some(true) {
            return Err(io::Error::other("the Blockwork daemon refused the connection"));
        }
        Ok(daemon)
    }

    async fn read(&mut self) -> io::Result<String> {
        tokio::time::timeout(blockwork_protocol::HANDSHAKE_TIMEOUT, blockwork_protocol::read_line(&mut self.reader))
            .await
            .map_err(|_| io::Error::other("the Blockwork daemon didn't answer"))??
            .ok_or_else(|| io::Error::other("the Blockwork daemon hung up"))
    }

    async fn call(&mut self, cmd: &str, args: Value) -> io::Result<Value> {
        let id = self.next_id;
        self.next_id += 1;
        let line = encode(&ClientMessage::Call { id, cmd: cmd.to_string(), args });
        self.writer.write_all(line.as_bytes()).await?;
        loop {
            let line = self.read().await?;
            let reply: DaemonMessage = serde_json::from_str(&line).map_err(io::Error::other)?;
            if reply.kind == DaemonMessageKind::Reply && reply.id == Some(id) {
                return match reply.error {
                    Some(error) => Err(io::Error::other(error)),
                    None => Ok(reply.data.unwrap_or(Value::Null)),
                };
            }
        }
    }

    async fn list(&mut self) -> io::Result<Vec<Value>> {
        match self.call("list_macros", json!({})).await? {
            Value::Array(macros) => Ok(macros),
            _ => Ok(Vec::new()),
        }
    }
}

/// A Windows build has no console of its own; print to the one it
/// was started from.
fn attach_console() {
    #[cfg(windows)]
    // SAFETY: plain Win32 call with a constant argument.
    unsafe {
        windows_sys::Win32::System::Console::AttachConsole(windows_sys::Win32::System::Console::ATTACH_PARENT_PROCESS);
    }
}
