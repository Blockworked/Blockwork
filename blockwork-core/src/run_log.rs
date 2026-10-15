//! Process-wide log of what macro runs did: starts, finishes, `log` blocks
//! and errors. Hosts poll [`revision`] and show [`snapshot`].

use serde::Serialize;
use std::collections::VecDeque;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

/// Oldest entries drop off past this many.
const CAPACITY: usize = 300;
/// Longer messages (a whole HTTP body, say) are cut to this many chars.
const MAX_MESSAGE_CHARS: usize = 500;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Level {
    Info,
    Error,
}

/// Run lifecycle markers, so hosts can tell which entries belong to the
/// latest run and whether a manual run is still going.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum RunEvent {
    /// A manual run began.
    Started,
    /// A trigger fired a strand.
    Fired,
    /// A manual run finished or was stopped.
    Ended,
}

/// Who wrote an entry: a macro, or the app itself (empty `macro_id`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Source {
    pub macro_id: String,
    pub name: String,
}

impl Source {
    pub fn app(name: &str) -> Source {
        Source { macro_id: String::new(), name: name.to_string() }
    }

    pub fn for_macro(macro_id: &str, name: &str) -> Source {
        Source { macro_id: macro_id.to_string(), name: name.to_string() }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Entry {
    pub seq: u64,
    /// Unix time in milliseconds.
    pub time_ms: i64,
    /// Display name of whatever wrote it.
    pub source: String,
    /// Empty for app-level entries.
    pub macro_id: String,
    /// Instruction id of the block that failed, when known.
    pub block_id: String,
    pub level: Level,
    pub event: Option<RunEvent>,
    pub message: String,
}

/// Serializes tests that assert on the shared log.
#[cfg(test)]
pub(crate) static TEST_LOCK: Mutex<()> = Mutex::new(());

static ENTRIES: Mutex<VecDeque<Entry>> = Mutex::new(VecDeque::new());
/// Bumped on every push and clear, so pollers can tell when to refresh.
static REVISION: AtomicU64 = AtomicU64::new(0);

fn push(source: &Source, level: Level, block_id: &str, event: Option<RunEvent>, message: impl Into<String>) {
    let mut message = crate::secrets::redact(&message.into());
    if let Some((cut, _)) = message.char_indices().nth(MAX_MESSAGE_CHARS) {
        message.truncate(cut);
        message.push('…');
    }
    let Ok(mut entries) = ENTRIES.lock() else { return };
    let seq = REVISION.fetch_add(1, Ordering::SeqCst) + 1;
    if entries.len() >= CAPACITY {
        entries.pop_front();
    }
    entries.push_back(Entry {
        seq,
        time_ms: chrono::Utc::now().timestamp_millis(),
        source: source.name.clone(),
        macro_id: source.macro_id.clone(),
        block_id: block_id.to_string(),
        level,
        event,
        message,
    });
}

pub fn info(source: &Source, message: impl Into<String>) {
    push(source, Level::Info, "", None, message);
}

pub fn error(source: &Source, message: impl Into<String>) {
    push(source, Level::Error, "", None, message);
}

/// An error a specific block caused, so the editor can mark that block.
pub fn block_error(source: &Source, block_id: &str, message: impl Into<String>) {
    push(source, Level::Error, block_id, None, message);
}

pub fn run_event(source: &Source, event: RunEvent, message: impl Into<String>) {
    push(source, Level::Info, "", Some(event), message);
}

pub fn revision() -> u64 {
    REVISION.load(Ordering::SeqCst)
}

/// Every retained entry, oldest first.
pub fn snapshot() -> Vec<Entry> {
    ENTRIES.lock().map(|e| e.iter().cloned().collect()).unwrap_or_default()
}

/// One line per entry for compact lists like the tray menu.
pub fn summary(entry: &Entry, max_chars: usize) -> String {
    let time = chrono::DateTime::from_timestamp_millis(entry.time_ms)
        .map(|t| t.with_timezone(&chrono::Local).format("%H:%M").to_string())
        .unwrap_or_default();
    let mark = if entry.level == Level::Error { "Error: " } else { "" };
    let line = format!("{time}  {}: {mark}{}", entry.source, entry.message.lines().next().unwrap_or_default());
    match line.char_indices().nth(max_chars) {
        Some((cut, _)) => format!("{}…", &line[..cut]),
        None => line,
    }
}

pub fn clear() {
    if let Ok(mut entries) = ENTRIES.lock() {
        entries.clear();
        REVISION.fetch_add(1, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // One test: the log is process-global, so separate tests would race.
    #[test]
    fn keeps_recent_entries_and_truncates_long_messages() {
        let _log = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        clear();
        let before = revision();
        let source = Source::for_macro("m1", "test");
        for i in 0..CAPACITY + 5 {
            info(&source, format!("entry {i}"));
        }
        block_error(&source, "b1", "x".repeat(MAX_MESSAGE_CHARS + 50));
        let entries = snapshot();
        // Other tests may log meanwhile, so only check this test's entries.
        let entries: Vec<_> = entries.into_iter().filter(|e| e.macro_id == "m1").collect();
        assert!(entries.len() <= CAPACITY);
        assert!(!entries.iter().any(|e| e.message == "entry 0"));
        let last = entries.last().unwrap();
        assert_eq!(last.level, Level::Error);
        assert_eq!(last.message.chars().count(), MAX_MESSAGE_CHARS + 1);
        assert_eq!((last.macro_id.as_str(), last.block_id.as_str()), ("m1", "b1"));
        assert!(summary(last, 20).ends_with('…'));
        assert!(revision() > before);
        clear();
        assert!(!snapshot().iter().any(|e| e.macro_id == "m1"));
    }
}
