//! Wire protocol between `blockwork-daemon` (owns all state, hotkeys and
//! playback) and the `blockwork` Qt Quick UI process.
//!
//! Transport is a per-user local socket - a Unix domain socket, or a named
//! pipe on Windows - carrying newline-delimited JSON, one message per line.
//! The daemon owning the socket doubles as the single-instance lock.

use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;
use std::io;
use tokio::io::{AsyncRead, AsyncWrite};

// ─── Messages ──────────────────────────────────────────────────────────────

/// A message from the UI to the daemon.
#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    /// First message on every connection. The daemon answers with
    /// [`DaemonMessage::hello`]; `accepted: false` means another UI is
    /// already open (and has been asked to come to the front), so this one
    /// should exit.
    Hello,
    /// OS autostart notification; does not open an editor connection.
    SystemStartup,
    /// First message from a script client (the `blockwork run` command line)
    /// instead of `Hello`. It gets no state pushes, may only call
    /// [`SCRIPT_COMMANDS`], and doesn't count as an open window.
    Script,
    /// First message from an agent client (`blockwork-agent`) instead of
    /// `Hello`. Versioned handshake; see `AGENT_PROTOCOL` and
    /// `AGENT_COMMANDS`. Agent connections do not take the UI slot, get no
    /// `State` pushes, and the daemon survives their disconnect.
    Agent {
        #[serde(default)]
        client: String,
        #[serde(default)]
        protocol: u32,
    },
    /// Runs a backend command; answered by a reply with the same `id`.
    Call {
        id: u64,
        cmd: String,
        #[serde(default)]
        args: serde_json::Value,
    },
}

/// What a [`ClientMessage::Script`] connection may call.
pub const SCRIPT_COMMANDS: [&str; 5] = [
    "list_macros",
    "run_macro_named",
    "run_macro_file",
    "macro_running",
    "stop_macro_named",
];

/// The agent handshake version this build speaks. The daemon negotiates
/// against `AGENT_PROTOCOL_MIN..=AGENT_PROTOCOL_MAX`.
pub const AGENT_PROTOCOL: u32 = 1;
pub const AGENT_PROTOCOL_MIN: u32 = 1;
pub const AGENT_PROTOCOL_MAX: u32 = 1;

/// Structured agent error codes (see `AgentError`).
pub const AGENT_ERROR_CODES: [&str; 7] = [
    "NOT_FOUND",
    "CONFLICT",
    "BAD_ARGS",
    "BUSY",
    "FORBIDDEN",
    "DAEMON_UNAVAILABLE",
    "UNSUPPORTED",
];

/// A structured error for agent commands: `{code, message, hint?}`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct AgentError {
    pub code: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
}

impl AgentError {
    pub fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.to_string(),
            message: message.into(),
            hint: None,
        }
    }
    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new("NOT_FOUND", message)
    }
    pub fn conflict(message: impl Into<String>) -> Self {
        Self::new("CONFLICT", message)
    }
    pub fn bad_args(message: impl Into<String>) -> Self {
        Self::new("BAD_ARGS", message)
    }
    pub fn busy(message: impl Into<String>) -> Self {
        Self::new("BUSY", message)
    }
    pub fn forbidden(message: impl Into<String>) -> Self {
        Self::new("FORBIDDEN", message)
    }
    pub fn unsupported(message: impl Into<String>) -> Self {
        Self::new("UNSUPPORTED", message)
    }
    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }
}

impl std::fmt::Display for AgentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

/// The allowlist for [`ClientMessage::Agent`] connections. Kept in
/// `blockwork-protocol` so the daemon, the backend parity tests and the
/// `blockwork-agent` client all agree on it. Add a command in exactly three
/// places: here, `blockwork-app::commands::agent`, and
/// `blockwork-agent/tools.rs` - the parity test enforces the third.
pub const AGENT_COMMANDS: [&str; 69] = [
    // Library
    "agent_list_macros",
    "agent_get_macro",
    "agent_create_macro",
    "agent_create_from_template",
    "agent_duplicate_macro",
    "agent_delete_macro",
    "agent_rename_macro",
    "agent_get_macro_settings",
    "agent_update_macro_settings",
    "agent_get_global_settings",
    "agent_set_global_settings",
    "agent_get_hotkeys",
    "agent_set_hotkeys",
    "agent_list_templates",
    "agent_describe_blocks",
    "agent_export_macro",
    "agent_import_macro",
    "agent_confirm_import",
    // Canvas
    "agent_add_strand",
    "agent_remove_strand",
    "agent_add_block",
    "agent_edit_block",
    "agent_remove_block",
    "agent_move_block",
    "agent_find_blocks",
    "agent_add_comment",
    "agent_remove_comment",
    "agent_create_variable",
    "agent_rename_variable",
    "agent_delete_variable",
    "agent_set_variable_value",
    "agent_set_list_items",
    "agent_create_list",
    "agent_rename_list",
    "agent_delete_list",
    "agent_replace_canvas",
    "agent_undo",
    "agent_redo",
    // Run
    "agent_run_macro",
    "agent_stop_macro",
    "agent_stop_all",
    "agent_run_status",
    "agent_run_log",
    "agent_trace",
    "agent_emergency_stop",
    "agent_resume_triggers",
    // See
    "agent_crop",
    "agent_screenshot",
    "agent_ocr",
    "agent_find_image",
    "agent_pixel",
    // Act (window/input/clipboard; input gated additionally at runtime)
    "agent_move_mouse",
    "agent_move_mouse_rel",
    "agent_click",
    "agent_scroll",
    "agent_key",
    "agent_type_text",
    "agent_button",
    "agent_window_action",
    "agent_window_move",
    "agent_clipboard_get",
    "agent_clipboard_set",
    "agent_notify",
    "agent_list_apps",
    "agent_launch_app",
    "agent_windows",
    "agent_active_window",
    "agent_window_bounds",
    "agent_cursor",
];

/// A message from the daemon to the UI. Kept as one flat struct rather than
/// a tagged enum so `payload` can stay a borrowed [`RawValue`]: the state
/// snapshot is large and is only ever forwarded to the UI as-is.
#[derive(Serialize, Deserialize, Debug)]
pub struct DaemonMessage<'a> {
    #[serde(rename = "type")]
    pub kind: DaemonMessageKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accepted: Option<bool>,
    #[serde(default, borrow, skip_serializing_if = "Option::is_none")]
    pub payload: Option<&'a RawValue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DaemonMessageKind {
    Hello,
    Reply,
    /// `payload` is the full `StateDto`, sent on connect and after every change.
    State,
    /// Bring the window to the front.
    Focus,
    /// Exit the UI process.
    Quit,
    /// `data` lists the blocks running strands are on, `[{macro_id, block_id}]`.
    Trace,
}

impl<'a> DaemonMessage<'a> {
    fn new(kind: DaemonMessageKind) -> Self {
        DaemonMessage {
            kind,
            id: None,
            accepted: None,
            payload: None,
            data: None,
            error: None,
        }
    }

    pub fn hello(accepted: bool) -> Self {
        DaemonMessage {
            accepted: Some(accepted),
            ..Self::new(DaemonMessageKind::Hello)
        }
    }

    pub fn reply(id: u64, result: Result<serde_json::Value, String>) -> Self {
        let mut message = DaemonMessage {
            id: Some(id),
            ..Self::new(DaemonMessageKind::Reply)
        };
        match result {
            Ok(data) => message.data = Some(data),
            Err(error) => message.error = Some(error),
        }
        message
    }

    pub fn state(payload: &'a RawValue) -> Self {
        DaemonMessage {
            payload: Some(payload),
            ..Self::new(DaemonMessageKind::State)
        }
    }

    pub fn focus() -> Self {
        Self::new(DaemonMessageKind::Focus)
    }

    pub fn quit() -> Self {
        Self::new(DaemonMessageKind::Quit)
    }

    pub fn trace(active: serde_json::Value) -> Self {
        DaemonMessage {
            data: Some(active),
            ..Self::new(DaemonMessageKind::Trace)
        }
    }
}

/// Serializes `message` as one protocol line (including the trailing `\n`).
pub fn encode<T: Serialize>(message: &T) -> String {
    let mut line = serde_json::to_string(message).expect("protocol messages always serialize");
    line.push('\n');
    line
}

// ─── Transport ─────────────────────────────────────────────────────────────

pub trait Stream: AsyncRead + AsyncWrite + Send + Unpin {}
impl<T: AsyncRead + AsyncWrite + Send + Unpin> Stream for T {}

pub type Connection = Box<dyn Stream>;

/// Debug builds get their own socket so `cargo run` never ends up talking to
/// an installed release daemon (or the other way round).
const NAME: &str = if cfg!(debug_assertions) {
    "daemon-dev"
} else {
    "daemon"
};

#[cfg(unix)]
pub use unix::{Listener, connect, socket_path};
#[cfg(windows)]
pub use windows::{Listener, connect, pipe_name};

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

pub const CONNECT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
pub const HANDSHAKE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);
pub const WRITE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);
pub const MAX_MESSAGE_BYTES: usize = 8 * 1024 * 1024;

pub async fn read_line<R: tokio::io::AsyncBufRead + Unpin>(
    reader: &mut R,
) -> io::Result<Option<String>> {
    use tokio::io::AsyncBufReadExt;
    let mut bytes = Vec::new();
    loop {
        let available = reader.fill_buf().await?;
        if available.is_empty() {
            return if bytes.is_empty() {
                Ok(None)
            } else {
                Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "incomplete IPC message",
                ))
            };
        }
        let end = available.iter().position(|b| *b == b'\n');
        let count = end.map_or(available.len(), |i| i + 1);
        if bytes.len() + count > MAX_MESSAGE_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "IPC message too large",
            ));
        }
        bytes.extend_from_slice(&available[..count]);
        reader.consume(count);
        if end.is_some() {
            return String::from_utf8(bytes)
                .map(Some)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e));
        }
    }
}

/// Convenience for `io::Error::other` with a message.
pub fn protocol_error(message: impl Into<String>) -> io::Error {
    io::Error::other(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(line: &str) -> DaemonMessage<'_> {
        assert!(line.ends_with('\n') && !line.trim_end().contains('\n'));
        serde_json::from_str(line.trim_end()).unwrap()
    }

    #[test]
    fn calls_round_trip_with_default_args() {
        let line = encode(&ClientMessage::Call {
            id: 7,
            cmd: "undo".into(),
            args: serde_json::json!({"x": 1}),
        });
        match serde_json::from_str(line.trim_end()).unwrap() {
            ClientMessage::Call { id, cmd, args } => {
                assert_eq!((id, cmd.as_str(), args["x"].as_i64()), (7, "undo", Some(1)))
            }
            other => panic!("unexpected {other:?}"),
        }
        let bare: ClientMessage =
            serde_json::from_str(r#"{"type":"call","id":1,"cmd":"get_state"}"#).unwrap();
        assert!(matches!(
            bare,
            ClientMessage::Call {
                args: serde_json::Value::Null,
                ..
            }
        ));
    }

    #[test]
    fn replies_carry_either_data_or_error() {
        let ok = encode(&DaemonMessage::reply(3, Ok(serde_json::json!("abc"))));
        let ok = parse(&ok);
        assert_eq!(
            (ok.kind, ok.id, ok.data, ok.error),
            (
                DaemonMessageKind::Reply,
                Some(3),
                Some(serde_json::json!("abc")),
                None
            )
        );

        let err = encode(&DaemonMessage::reply(4, Err("nope".into())));
        let err = parse(&err);
        assert_eq!(
            (err.id, err.data, err.error.as_deref()),
            (Some(4), None, Some("nope"))
        );
    }

    #[test]
    fn state_payload_is_forwarded_verbatim() {
        let raw = RawValue::from_string(r#"{"page":"Main","macro_strs":["a\nb"]}"#.into()).unwrap();
        let line = encode(&DaemonMessage::state(&raw));
        let message = parse(&line);
        assert_eq!(message.kind, DaemonMessageKind::State);
        assert_eq!(message.payload.unwrap().get(), raw.get());
    }

    #[test]
    fn hello_reports_acceptance() {
        assert_eq!(
            parse(&encode(&DaemonMessage::hello(false))).accepted,
            Some(false)
        );
        assert_eq!(
            parse(&encode(&DaemonMessage::focus())).kind,
            DaemonMessageKind::Focus
        );
    }
}

#[cfg(test)]
mod framing_tests {
    use super::*;
    #[tokio::test]
    async fn rejects_oversized_and_partial_messages() {
        let input = vec![b'x'; MAX_MESSAGE_BYTES + 1];
        assert_eq!(
            read_line(&mut input.as_slice()).await.unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
        assert_eq!(
            read_line(&mut &b"partial"[..]).await.unwrap_err().kind(),
            io::ErrorKind::UnexpectedEof
        );
        assert!(read_line(&mut &b""[..]).await.unwrap().is_none());
    }
    #[tokio::test]
    async fn preserves_message_boundaries() {
        let mut input = &b"one\ntwo\n"[..];
        assert_eq!(
            read_line(&mut input).await.unwrap().as_deref(),
            Some("one\n")
        );
        assert_eq!(
            read_line(&mut input).await.unwrap().as_deref(),
            Some("two\n")
        );
    }
}
