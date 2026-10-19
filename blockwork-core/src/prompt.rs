//! Dialogs a macro shows: ask for text, yes/no, pick from a list, a message,
//! and permission requests. The host draws them; without one, asking fails.

use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Text,
    Confirm,
    Choice,
    Message,
    Permission,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    pub kind: Kind,
    pub title: String,
    pub message: String,
    #[serde(default)]
    pub default: String,
    #[serde(default)]
    pub options: Vec<String>,
    /// Permission questions: the macro's name and the ability's key, so
    /// the dialog can word the question in the UI language.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub subject: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub ability: String,
    /// The UI language from Settings; empty means the system's.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub language: String,
}

impl Request {
    pub fn new(kind: Kind, title: &str, message: impl Into<String>) -> Self {
        Request {
            kind,
            title: title.to_string(),
            message: message.into(),
            default: String::new(),
            options: Vec::new(),
            subject: String::new(),
            ability: String::new(),
            language: String::new(),
        }
    }
}

/// `answer` is the button: `ok`, `cancel`, `yes`, `no`, `deny`, `once` or
/// `always`. Closing the window counts as `cancel`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Reply {
    pub answer: String,
    #[serde(default)]
    pub text: String,
}

/// Shows a dialog and blocks until it's answered, or fails once `stopped`
/// turns true.
pub type Handler = dyn Fn(&Request, &dyn Fn() -> bool) -> Result<Reply, String> + Send + Sync;

static HANDLER: RwLock<Option<Arc<Handler>>> = RwLock::new(None);
/// One dialog at a time, so parallel strands don't stack windows.
static SHOWING: Mutex<()> = Mutex::new(());

pub fn set_handler(handler: Arc<Handler>) {
    if let Ok(mut slot) = HANDLER.write() {
        *slot = Some(handler);
    }
}

pub fn ask(request: &Request, stopped: &dyn Fn() -> bool) -> Result<Reply, String> {
    let handler = HANDLER
        .read()
        .ok()
        .and_then(|h| h.clone())
        .ok_or_else(|| "dialogs aren't available here".to_string())?;
    let _turn = loop {
        match SHOWING.try_lock() {
            Ok(guard) => break guard,
            Err(std::sync::TryLockError::Poisoned(p)) => break p.into_inner(),
            Err(std::sync::TryLockError::WouldBlock) => {
                if stopped() {
                    return Err("stopped".into());
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    };
    handler(request, stopped)
}
