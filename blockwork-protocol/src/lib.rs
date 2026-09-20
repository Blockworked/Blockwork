//! Wire protocol between `blockwork-daemon` (owns all state, hotkeys and
//! playback) and the `blockwork` CEF UI process.
//!
//! Transport is a per-user local socket - a Unix domain socket, or a named
//! pipe on Windows - carrying newline-delimited JSON, one message per line.
//! The daemon owning the socket doubles as the single-instance lock.

use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;
use std::io;
use tokio::io::{AsyncRead, AsyncWrite};

// ─── Messages ──────────────────────────────────────────────────────────────

/// Command the UI calls for each key event CEF saw in its focused window
/// (Windows only; see `tauri_runtime_cef::set_focused_key_hook`). Args:
/// `{"vk": u16, "pressed": bool}`; replies whether to suppress the key.
pub const FOCUSED_KEY_EVENT: &str = "focused_key_event";

/// A message from the UI to the daemon.
#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    /// First message on every connection. The daemon answers with
    /// [`DaemonMessage::hello`]; `accepted: false` means another UI is
    /// already open (and has been asked to come to the front), so this one
    /// should exit. Carries the UI's `--ozone-platform` choice so the daemon
    /// can reuse it when relaunching the UI from the tray.
    Hello {
        #[serde(default)]
        ozone_platform: Option<String>,
    },
    /// Runs a backend command; answered by a reply with the same `id`.
    Call {
        id: u64,
        cmd: String,
        #[serde(default)]
        args: serde_json::Value,
    },
}

/// CLI switch the UI accepts to force an Ozone platform (currently only
/// `x11` is acted on). The daemon remembers it from the UI's `Hello` and
/// replays it on tray relaunches.
pub const OZONE_PLATFORM_ARG: &str = "--ozone-platform";

/// Parses `--ozone-platform=x11` (or `--ozone-platform x11`) out of an
/// argument list. The last occurrence wins, matching Chromium.
pub fn parse_ozone_platform<I>(args: I) -> Option<String>
where
    I: IntoIterator<Item = String>,
{
    let args: Vec<String> = args.into_iter().collect();
    let mut result = None;
    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        if let Some(value) = arg.strip_prefix(&format!("{OZONE_PLATFORM_ARG}=")) {
            if !value.is_empty() {
                result = Some(value.to_string());
            }
        } else if arg == OZONE_PLATFORM_ARG {
            if let Some(next) = args.get(i + 1) {
                if !next.starts_with('-') && !next.is_empty() {
                    result = Some(next.clone());
                    i += 1;
                }
            }
        }
        i += 1;
    }
    result
}

/// Reads the current process's `--ozone-platform` choice, if any.
pub fn current_ozone_platform() -> Option<String> {
    parse_ozone_platform(std::env::args().skip(1))
}

/// Formats an `--ozone-platform` argument for a child process.
pub fn ozone_platform_arg(value: &str) -> String {
    format!("{OZONE_PLATFORM_ARG}={value}")
}

/// A message from the daemon to the UI. Kept as one flat struct rather than
/// a tagged enum so `payload` can stay a borrowed [`RawValue`]: the state
/// snapshot is large and is only ever forwarded to the webview as-is.
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
    /// Close the UI; the whole app is shutting down.
    Quit,
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
