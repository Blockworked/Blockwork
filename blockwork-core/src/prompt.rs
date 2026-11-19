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
    /// Pick a file; the answer's text is its path.
    File,
    /// Pick a folder.
    Folder,
    /// A small box of text that closes by itself; nothing waits for it.
    Tooltip,
    /// `options` as a menu at the mouse.
    Menu,
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
    /// The UI animation speed from Settings; `None` means normal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub animation_speed: Option<f64>,
    /// Where a tooltip or menu goes, in screen coordinates.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub y: Option<f64>,
    /// How long a tooltip stays, in ms.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<f64>,
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
            animation_speed: None,
            x: None,
            y: None,
            duration_ms: None,
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

/// Counts tooltips shown, so a newer one (or hiding) closes the one before.
static TOOLTIP: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// How long after showing a tooltip to look for its window to place it.
const TOOLTIP_SETTLE: Duration = Duration::from_millis(250);

/// Shows a tooltip without waiting for it, replacing any shown before.
/// `place` puts it once its window is up and its size is known; empty
/// text only hides the last one.
pub fn show_tooltip(
    mut request: Request,
    place: impl FnOnce(&str) + Send + 'static,
) -> Result<(), String> {
    let generation = TOOLTIP.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
    if request.message.trim().is_empty() {
        return Ok(());
    }
    let handler = HANDLER
        .read()
        .ok()
        .and_then(|h| h.clone())
        .ok_or_else(|| "dialogs aren't available here".to_string())?;
    // A title no other window has, so the window can be found to move it.
    request.title = format!("Blockwork tooltip {}-{generation}", std::process::id());
    let title = request.title.clone();
    let limit = request.duration_ms.unwrap_or(3000.0).max(0.0) / 1000.0 + 5.0;
    let shown = std::time::Instant::now();
    std::thread::Builder::new()
        .name("tooltip".into())
        .spawn(move || {
            let replaced = || {
                TOOLTIP.load(std::sync::atomic::Ordering::SeqCst) != generation
                    || shown.elapsed().as_secs_f64() > limit
            };
            let _ = handler(&request, &replaced);
        })
        .map_err(|e| e.to_string())?;
    std::thread::Builder::new()
        .name("tooltip-place".into())
        .spawn(move || {
            std::thread::sleep(TOOLTIP_SETTLE);
            place(&title);
        })
        .map(|_| ())
        .map_err(|e| e.to_string())
}
