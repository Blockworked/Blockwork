//! The background watchers that start trigger blocks (`When...`), one per
//! kind of event. Each `start` runs on its own thread for the life of the
//! daemon, finds the strands its trigger heads in every live macro
//! (`scheduled_run::live_macros`), and runs them when the event happens.
//!
//! The rules they share live here: what a fired strand carries ([`Target`]),
//! when a polled condition fires ([`fires_on_change`], [`changed`]) and how a
//! failing system query is reported ([`ReportOnce`]).

pub(crate) mod battery;
pub(crate) mod clipboard;
pub(crate) mod file;
pub(crate) mod key;
pub(crate) mod mouse;
pub(crate) mod system;
pub(crate) mod time;
pub(crate) mod typed;
pub(crate) mod window;

use crate::AppHandle;
use crate::scheduled_run;
use crate::state::SharedState;
use blockstitch_core::graph::ListDef;
use blockwork_core::input::value::Evaluated;
use blockwork_core::macros::backend::InputBackend;
use blockwork_core::macros::trigger::fired_message;
use blockwork_core::macros::{Instruction, Macro, Strand, VariableDef};
use blockwork_core::run_log;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Which strand a watcher remembers state for, by (macro id, strand id).
pub(crate) type StrandKey = (String, String);

/// A trigger strand, copied out of the state lock so it can fire after the
/// lock is released.
pub(crate) struct Target {
    pub(crate) key: StrandKey,
    message: String,
    body: Vec<Instruction>,
    variables: Vec<VariableDef>,
    lists: Vec<ListDef>,
}

impl Target {
    /// `strand`'s body (everything under its header) in `mac`.
    pub(crate) fn new(mac: &Macro, strand: &Strand) -> Self {
        Self::with_body(mac, strand, strand.instructions[1..].to_vec())
    }

    /// Like [`Target::new`], with a body the watcher built (text expansion
    /// erases the abbreviation first).
    pub(crate) fn with_body(mac: &Macro, strand: &Strand, body: Vec<Instruction>) -> Self {
        Self {
            key: (mac.id.clone(), strand.id.clone()),
            message: fired_message(strand.instructions.first().map(|i| &i.kind)),
            body,
            variables: mac.variables.clone(),
            lists: mac.lists.clone(),
        }
    }

    /// Runs the body on its own thread (`scheduled_run::fire`).
    pub(crate) fn fire(&self, emulator: &Arc<Mutex<dyn InputBackend>>, speed: f64, shared_state: &SharedState, app: &AppHandle) {
        scheduled_run::fire(
            self.key.0.clone(),
            self.message.clone(),
            self.body.clone(),
            Arc::clone(emulator),
            speed,
            &self.variables,
            &self.lists,
            Arc::clone(shared_state),
            app.clone(),
        );
    }
}

/// The macro's saved variable values, for resolving a header's slots.
pub(crate) fn variable_env(mac: &Macro) -> HashMap<String, Evaluated> {
    mac.variables.iter().map(|v| (v.name.clone(), v.value.clone())).collect()
}

/// Whether a polled yes/no condition fires: it changed since the last poll
/// and is now `fire_on`. The first poll (`previous` is `None`) only records
/// a baseline, so opening a macro doesn't fire on a state that was already so.
pub(crate) fn fires_on_change(previous: Option<bool>, now: bool, fire_on: bool) -> bool {
    previous.is_some_and(|previous| previous != now) && now == fire_on
}

/// Whether a polled value fires: any difference from the last poll, with the
/// first poll only recording a baseline.
pub(crate) fn changed<T: PartialEq>(previous: Option<&T>, now: &T) -> bool {
    previous.is_some_and(|previous| previous != now)
}

/// Logs each distinct failure of a system query once, so a missing service
/// doesn't flood the run log every poll.
#[derive(Default)]
pub(crate) struct ReportOnce(HashMap<&'static str, String>);

impl ReportOnce {
    /// The value, or `None` after logging `what`'s error if it is new.
    pub(crate) fn check<T>(&mut self, what: &'static str, result: Result<T, String>) -> Option<T> {
        match result {
            Ok(value) => {
                self.0.remove(what);
                Some(value)
            }
            Err(e) => {
                if self.0.get(what) != Some(&e) {
                    run_log::error(&run_log::Source::app(what), e.clone());
                    self.0.insert(what, e);
                }
                None
            }
        }
    }

    #[cfg(test)]
    fn reported(&self, what: &str) -> Option<&str> {
        self.0.get(what).map(String::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_condition_fires_only_when_it_becomes_what_the_header_waits_for() {
        // The first poll is a baseline either way.
        assert!(!fires_on_change(None, true, true));
        assert!(!fires_on_change(None, false, false));
        // A change towards `fire_on` fires; a change away from it doesn't.
        assert!(fires_on_change(Some(false), true, true));
        assert!(!fires_on_change(Some(true), false, true));
        assert!(fires_on_change(Some(true), false, false));
        // No change, no fire.
        assert!(!fires_on_change(Some(true), true, true));
        assert!(!fires_on_change(Some(false), false, false));
    }

    #[test]
    fn a_polled_value_fires_on_any_difference_after_the_baseline() {
        assert!(!changed(None, &"copied"));
        assert!(!changed(Some(&"copied"), &"copied"));
        assert!(changed(Some(&"copied"), &"pasted"));
        assert!(changed(Some(&None), &Some(3)), "a file that appears counts");
        assert!(changed(Some(&Some(3)), &None), "and one that goes away");
    }

    #[test]
    fn a_failure_is_reported_once_until_it_recovers() {
        let mut report = ReportOnce::default();
        assert_eq!(report.check("Idle trigger", Err::<u8, _>("no idle service".into())), None);
        assert_eq!(report.reported("Idle trigger"), Some("no idle service"));
        assert_eq!(report.check("Idle trigger", Err::<u8, _>("no idle service".into())), None);
        assert_eq!(report.check("Idle trigger", Ok(4)), Some(4));
        assert_eq!(report.reported("Idle trigger"), None, "recovering forgets it, so a later failure logs again");
    }

    #[test]
    fn a_target_fires_the_body_under_its_header() {
        use blockwork_core::macros::InstructionKind;
        let mut mac = Macro::new("Watched".into(), String::new(), Vec::new());
        mac.id = "m".into();
        let mut strand = Strand::with_instructions(
            0,
            0,
            vec![Instruction::new(InstructionKind::WhenClipboardChanged), Instruction::new(InstructionKind::EscapeLoop)],
        );
        strand.id = "s".into();
        let target = Target::new(&mac, &strand);
        assert_eq!(target.key, ("m".to_string(), "s".to_string()));
        assert_eq!(target.body.len(), 1);
        assert!(matches!(target.body[0].kind, InstructionKind::EscapeLoop));
    }
}
