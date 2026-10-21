//! Global variables: shared by every macro and kept across restarts in
//! `globals.json` in the config directory.

use crate::input::value::Evaluated;
use std::collections::BTreeMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

static STORE: Mutex<Option<BTreeMap<String, Evaluated>>> = Mutex::new(None);
/// A save is pending; writes are batched so a tight loop doesn't hit the disk
/// on every change.
static DIRTY: AtomicBool = AtomicBool::new(false);
const SAVE_DELAY: Duration = Duration::from_millis(500);

fn path() -> Option<std::path::PathBuf> {
    // Tests keep their globals in memory, away from the real config dir.
    if cfg!(test) {
        return None;
    }
    crate::config::config_root().ok().map(|root| root.join("globals.json"))
}

fn load() -> BTreeMap<String, Evaluated> {
    path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

fn with_store<R>(f: impl FnOnce(&mut BTreeMap<String, Evaluated>) -> R) -> R {
    let mut guard = STORE.lock().unwrap_or_else(|e| e.into_inner());
    f(guard.get_or_insert_with(load))
}

/// The value of global `name`; 0 when it was never set, like a new variable.
pub fn get(name: &str) -> Evaluated {
    with_store(|store| store.get(name.trim()).cloned()).unwrap_or(Evaluated::Number(0.0))
}

pub fn set(name: &str, value: Evaluated) -> Result<(), String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("the global variable needs a name".into());
    }
    with_store(|store| store.insert(name.to_string(), value));
    schedule_save();
    Ok(())
}

/// Adds `by` to global `name`, treating a missing or non-number value as 0.
pub fn change(name: &str, by: f64) -> Result<(), String> {
    let current = get(name).as_number().unwrap_or(0.0);
    set(name, Evaluated::Number(current + by))
}

pub fn remove(name: &str) {
    with_store(|store| store.remove(name));
    schedule_save();
}

/// Every global, sorted by name.
pub fn all() -> Vec<(String, Evaluated)> {
    with_store(|store| store.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
}

fn schedule_save() {
    if DIRTY.swap(true, Ordering::AcqRel) {
        return;
    }
    std::thread::spawn(|| {
        std::thread::sleep(SAVE_DELAY);
        flush();
    });
}

/// The store as saved: secret values masked, like in macro files, so a
/// global that held a secret never reaches the disk in plain text.
fn without_secrets(store: &BTreeMap<String, Evaluated>) -> BTreeMap<String, Evaluated> {
    store
        .iter()
        .map(|(name, value)| {
            let value = match value {
                Evaluated::Text(text) => Evaluated::Text(crate::secrets::redact(text)),
                other => other.clone(),
            };
            (name.clone(), value)
        })
        .collect()
}

/// Writes pending changes now.
pub fn flush() {
    if !DIRTY.swap(false, Ordering::AcqRel) {
        return;
    }
    let (Some(path), Ok(json)) = (path(), with_store(|store| serde_json::to_string_pretty(&without_secrets(store)))) else {
        return;
    };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let temp = path.with_extension("json.tmp");
    if let Err(e) = std::fs::write(&temp, json).and_then(|_| std::fs::rename(&temp, &path)) {
        tracing::warn!("Failed to save global variables: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn globals_default_to_zero_and_count_up() {
        assert_eq!(get("globals-test"), Evaluated::Number(0.0));
        change("globals-test", 2.0).unwrap();
        change(" globals-test ", 3.0).unwrap();
        assert_eq!(get("globals-test"), Evaluated::Number(5.0));
        set("globals-test", Evaluated::Text("hi".into())).unwrap();
        assert_eq!(get("globals-test"), Evaluated::Text("hi".into()));
        assert!(set("  ", Evaluated::Bool(true)).is_err());
        remove("globals-test");
        assert_eq!(get("globals-test"), Evaluated::Number(0.0));
    }

    #[test]
    fn saved_globals_mask_secrets() {
        crate::secrets::set("globals-test-secret", "hunter2-globals").unwrap();
        let mut store = BTreeMap::new();
        store.insert("token".to_string(), Evaluated::Text("Bearer hunter2-globals".into()));
        store.insert("count".to_string(), Evaluated::Number(3.0));
        let saved = without_secrets(&store);
        assert_eq!(saved["token"], Evaluated::Text(format!("Bearer {}", crate::secrets::MASK)));
        assert_eq!(saved["count"], Evaluated::Number(3.0));
        let _ = crate::secrets::delete("globals-test-secret");
    }
}
