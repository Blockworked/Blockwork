//! What restricted macros may do without asking. Grants live in the config
//! folder, not the macro file, so a shared macro can't grant itself anything.
//! Macros with no entry (your own) are trusted.

use crate::prompt::{self, Kind, Request};
use crate::run_log::Source;
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Abilities that ask first, with what the prompt says the macro wants to do.
pub const GUARDED: [(&str, &str); 5] = [
    ("commands", "run commands and open or close apps"),
    ("network", "go online"),
    ("files_write", "write to files"),
    ("secrets", "read your saved secrets"),
    ("screen", "read the screen"),
];

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Grants {
    #[serde(default)]
    pub allowed: Vec<String>,
}

static STORE: Mutex<Option<HashMap<String, Grants>>> = Mutex::new(None);
/// "Allow this time" and "Deny" answers, until the macro's runs end.
static SESSION: Mutex<Vec<(String, String, bool)>> = Mutex::new(Vec::new());
/// One permission question at a time, so a loop doesn't queue a stack of them.
static ASKING: Mutex<()> = Mutex::new(());

#[cfg(not(test))]
const FILE_NAME: &str = "permissions.json";

fn label(kind: &str) -> &'static str {
    GUARDED.iter().find(|(k, _)| *k == kind).map_or("do that", |(_, l)| l)
}

#[cfg(not(test))]
fn load() -> HashMap<String, Grants> {
    crate::config::config_root()
        .ok()
        .and_then(|root| std::fs::read_to_string(root.join(FILE_NAME)).ok())
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

#[cfg(test)]
fn load() -> HashMap<String, Grants> {
    HashMap::new()
}

#[cfg(not(test))]
fn save(store: &HashMap<String, Grants>) {
    let Ok(root) = crate::config::config_root() else { return };
    let _ = std::fs::create_dir_all(&root);
    match serde_json::to_string_pretty(store) {
        Ok(text) => {
            if let Err(e) = std::fs::write(root.join(FILE_NAME), text) {
                tracing::warn!("Failed to save permissions: {e}");
            }
        }
        Err(e) => tracing::warn!("Failed to save permissions: {e}"),
    }
}

#[cfg(test)]
fn save(_: &HashMap<String, Grants>) {}

fn with_store<T>(f: impl FnOnce(&mut HashMap<String, Grants>) -> (T, bool)) -> T {
    let mut slot = STORE.lock().unwrap_or_else(|p| p.into_inner());
    let store = slot.get_or_insert_with(load);
    let (out, changed) = f(store);
    if changed {
        save(store);
    }
    out
}

/// `None` when the macro is trusted.
pub fn grants(macro_id: &str) -> Option<Grants> {
    with_store(|s| (s.get(macro_id).cloned(), false))
}

/// Makes `macro_id` ask before anything not in `allowed`.
pub fn restrict(macro_id: &str, allowed: Vec<String>) {
    with_store(|s| (s.insert(macro_id.to_string(), Grants { allowed }), true));
}

/// Trusts `macro_id` fully, or forgets a deleted one.
pub fn trust(macro_id: &str) {
    with_store(|s| ((), s.remove(macro_id).is_some()));
}

pub fn set_allowed(macro_id: &str, kind: &str, allowed: bool) {
    with_store(|s| {
        let Some(grants) = s.get_mut(macro_id) else { return ((), false) };
        grants.allowed.retain(|k| k != kind);
        if allowed {
            grants.allowed.push(kind.to_string());
        }
        ((), true)
    });
}

/// Gives a copy of a macro the same restrictions.
pub fn copy(from: &str, to: &str) {
    with_store(|s| match s.get(from).cloned() {
        Some(grants) => (s.insert(to.to_string(), grants), true),
        None => (None, false),
    });
}

pub(crate) fn run_ended(macro_id: &str) {
    if let Ok(mut session) = SESSION.lock() {
        session.retain(|(id, ..)| id != macro_id);
    }
}

fn session_answer(macro_id: &str, kind: &str) -> Option<bool> {
    SESSION.lock().ok()?.iter().find(|(id, k, _)| id == macro_id && k == kind).map(|(.., ok)| *ok)
}

fn remember(macro_id: &str, kind: &str, ok: bool) {
    if let Ok(mut session) = SESSION.lock() {
        session.push((macro_id.to_string(), kind.to_string(), ok));
    }
}

/// Settled without asking: `Some(Ok)` allowed, `Some(Err)` denied this run.
fn decided(macro_id: &str, kind: &str) -> Option<Result<(), String>> {
    match grants(macro_id) {
        None => return Some(Ok(())),
        Some(g) if g.allowed.iter().any(|k| k == kind) => return Some(Ok(())),
        Some(_) => {}
    }
    session_answer(macro_id, kind).map(|ok| if ok { Ok(()) } else { Err(denied(kind)) })
}

fn denied(kind: &str) -> String {
    format!("not allowed to {}. Change it in Macro settings > Permissions", label(kind))
}

/// Asks the user unless `source`'s macro already may do `kind`.
pub fn check(source: &Source, kind: &str, stopped: &dyn Fn() -> bool) -> Result<(), String> {
    if source.macro_id.is_empty() {
        return Ok(());
    }
    if let Some(done) = decided(&source.macro_id, kind) {
        return done;
    }
    let _turn = ASKING.lock().unwrap_or_else(|p| p.into_inner());
    // Another strand may have asked while this one waited.
    if let Some(done) = decided(&source.macro_id, kind) {
        return done;
    }
    let name = if source.name.is_empty() { "A macro" } else { source.name.as_str() };
    let mut request = Request::new(Kind::Permission, "Allow this macro?", format!("“{name}” wants to {}.", label(kind)));
    request.subject = name.to_string();
    request.ability = kind.to_string();
    let answer = prompt::ask(&request, stopped).map(|r| r.answer);
    match answer.as_deref() {
        Ok("always") => {
            set_allowed(&source.macro_id, kind, true);
            Ok(())
        }
        Ok("once") => {
            remember(&source.macro_id, kind, true);
            Ok(())
        }
        Ok(_) => {
            remember(&source.macro_id, kind, false);
            Err(denied(kind))
        }
        Err(e) => Err(format!("{} ({e})", denied(kind))),
    }
}

type Current = (Arc<Source>, Option<Arc<Mutex<bool>>>);

thread_local! {
    static CURRENT: RefCell<Option<Current>> = const { RefCell::new(None) };
}

/// Marks this thread as running `source`'s macro until dropped, so
/// reporters can [`require`] abilities.
pub struct Scope(Option<Current>);

pub fn enter(source: Arc<Source>, stop_flag: Option<Arc<Mutex<bool>>>) -> Scope {
    Scope(CURRENT.with(|c| c.replace(Some((source, stop_flag)))))
}

impl Drop for Scope {
    fn drop(&mut self) {
        let previous = self.0.take();
        CURRENT.with(|c| *c.borrow_mut() = previous);
    }
}

/// [`check`] for the macro running on this thread; outside a run (previews) it's allowed.
pub fn require(kind: &str) -> Result<(), String> {
    let Some((source, flag)) = CURRENT.with(|c| c.borrow().clone()) else { return Ok(()) };
    let stopped = || flag.as_ref().is_some_and(|f| f.lock().map(|r| !*r).unwrap_or(true));
    check(&source, kind, &stopped)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restricted_macros_ask_and_remember_the_answer() {
        let source = Source::for_macro("perm-test", "Test");
        assert!(check(&source, "network", &|| false).is_ok(), "trusted by default");

        restrict("perm-test", vec!["screen".into()]);
        assert!(check(&source, "screen", &|| false).is_ok());
        // No dialog handler in tests: asking fails, which denies.
        assert!(check(&source, "network", &|| false).is_err());

        remember("perm-test", "commands", true);
        assert!(check(&source, "commands", &|| false).is_ok());
        run_ended("perm-test");
        assert_eq!(session_answer("perm-test", "commands"), None);

        set_allowed("perm-test", "network", true);
        assert!(check(&source, "network", &|| false).is_ok());
        copy("perm-test", "perm-copy");
        assert_eq!(grants("perm-copy"), grants("perm-test"));
        trust("perm-test");
        trust("perm-copy");
        assert_eq!(grants("perm-test"), None);
    }
}
