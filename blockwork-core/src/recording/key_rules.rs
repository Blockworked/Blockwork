//! Keys and mouse buttons that triggers keep from reaching other apps, and
//! keys turned into other keys. The capture callback asks this module about
//! every physical event before passing it on, so the rule table is read
//! without waiting: the app rebuilds it when macros change, and arms each
//! rule from its conditions on another thread.

use crate::input::key_names::{ComboKey, combo_held, copilot_modifiers};
use crate::input::types::{MacroButton, MacroKey};
use crate::macros::backend::{CaptureDecision, CaptureEvent};
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex, OnceLock, RwLock};

/// What a rule listens for.
#[derive(Debug, Clone)]
pub enum RuleInput {
    /// A key combo; `extra_keys` lets other keys be held too.
    Keys {
        combo: Vec<ComboKey>,
        extra_keys: bool,
    },
    /// A mouse button or scroll direction, with exactly these modifier bits
    /// held, or any when `None`.
    Button {
        button: MacroButton,
        modifiers: Option<u8>,
    },
}

/// What a rule does with a matching event.
#[derive(Debug, Clone, PartialEq)]
pub enum RuleAction {
    /// Keep it (and its release) from other apps.
    Block,
    /// Keep it from other apps and press these keys in its place, until it
    /// is released.
    Remap(Vec<MacroKey>),
}

#[derive(Debug, Clone)]
pub struct Rule {
    pub input: RuleInput,
    pub action: RuleAction,
    /// Whether the rule's conditions hold right now. Kept up to date by the
    /// app, since the capture callback can't wait for the window system.
    pub armed: Arc<AtomicBool>,
    /// Modifiers the key holds down on its own before it arrives (the
    /// Copilot key's Shift and Meta), taken back when the rule fires.
    lift: Vec<ComboKey>,
}

impl Rule {
    pub fn new(input: RuleInput, action: RuleAction) -> Self {
        let lift = match &input {
            RuleInput::Keys { combo, .. } => copilot_modifiers(combo),
            RuleInput::Button { .. } => Vec::new(),
        };
        Self {
            input,
            action,
            armed: Arc::new(AtomicBool::new(true)),
            lift,
        }
    }

    /// Leaves held modifiers alone, for when nothing could release them.
    pub fn without_lift(mut self) -> Self {
        self.lift.clear();
        self
    }
}

/// What a remap asks the app to press or release in a key's place.
#[derive(Debug, Clone, PartialEq)]
pub enum RemapEvent {
    Press(Vec<MacroKey>),
    Release(Vec<MacroKey>),
    /// Release these keys, which other apps saw go down, without letting a
    /// lone Meta release open the start menu or launcher.
    Lift(Vec<MacroKey>),
}

static RULES: OnceLock<RwLock<Vec<Rule>>> = OnceLock::new();
static REMAPS: OnceLock<Mutex<Option<Sender<RemapEvent>>>> = OnceLock::new();
static STATE: OnceLock<Mutex<State>> = OnceLock::new();

fn rules() -> &'static RwLock<Vec<Rule>> {
    RULES.get_or_init(|| RwLock::new(Vec::new()))
}

/// Replaces the rule table. Keys already held keep what their press decided,
/// so a blocked key's release stays blocked and a remap is always released.
pub fn set_rules(new: Vec<Rule>) {
    if let Ok(mut table) = rules().write() {
        *table = new;
    }
}

/// Receives the keys remaps press and release. There is one receiver: a new
/// call replaces the last one.
pub fn subscribe_remaps() -> Receiver<RemapEvent> {
    let (tx, rx) = channel();
    if let Ok(mut slot) = REMAPS.get_or_init(|| Mutex::new(None)).lock() {
        *slot = Some(tx);
    }
    rx
}

fn send_remap(event: RemapEvent) {
    if let Some(Ok(slot)) = REMAPS.get().map(|m| m.lock())
        && let Some(tx) = slot.as_ref()
    {
        let _ = tx.send(event);
    }
}

/// How many strands are inside a "with input blocked" block.
static BLOCKING_ALL: AtomicUsize = AtomicUsize::new(0);

/// Keeps the physical keyboard and mouse from every app until dropped.
/// Keys already held still release, so nothing sticks.
pub struct InputBlocked(());

pub fn block_all_input() -> InputBlocked {
    BLOCKING_ALL.fetch_add(1, Ordering::SeqCst);
    InputBlocked(())
}

impl Drop for InputBlocked {
    fn drop(&mut self) {
        BLOCKING_ALL.fetch_sub(1, Ordering::SeqCst);
    }
}

pub fn input_blocked() -> bool {
    BLOCKING_ALL.load(Ordering::Relaxed) > 0
}

/// Physical keys and buttons held now, and which of them a rule took.
#[derive(Default)]
struct State {
    keys: Vec<MacroKey>,
    buttons: HashSet<MacroButton>,
    blocked_keys: HashSet<MacroKey>,
    remapped: Vec<(MacroKey, Vec<MacroKey>)>,
    blocked_buttons: HashSet<MacroButton>,
}

fn state() -> std::sync::MutexGuard<'static, State> {
    STATE
        .get_or_init(|| Mutex::new(State::default()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

/// The physical keys held right now, as the capture callback saw them.
pub fn held_keys() -> Vec<MacroKey> {
    state().keys.clone()
}

/// Whether the keys of `combo` are all held now (others may be too).
pub fn combo_is_held(combo: &[ComboKey]) -> bool {
    combo_held(combo, &state().keys, true)
}

/// Forgets every held key, as when the desktop switches and releases are lost.
pub fn reset_held() {
    let mut s = state();
    s.keys.clear();
    s.buttons.clear();
}

/// Records a physical event in the held state. Returns `Suppress` for the
/// release of a key or button whose press a rule took, whatever the rules
/// say now.
pub(super) fn observe(event: &CaptureEvent) -> Option<CaptureDecision> {
    let mut s = state();
    match event {
        CaptureEvent::KeyPress(key) => {
            if !s.keys.contains(key) {
                s.keys.push(key.clone());
            }
            None
        }
        CaptureEvent::KeyRelease(key) => {
            s.keys.retain(|k| k != key);
            if s.blocked_keys.remove(key) {
                return Some(CaptureDecision::Suppress);
            }
            if let Some(at) = s.remapped.iter().position(|(k, _)| k == key) {
                let (_, to) = s.remapped.remove(at);
                drop(s);
                send_remap(RemapEvent::Release(to));
                return Some(CaptureDecision::Suppress);
            }
            None
        }
        CaptureEvent::ButtonPress(button) => {
            s.buttons.insert(button.clone());
            None
        }
        CaptureEvent::ButtonRelease(button) => {
            s.buttons.remove(button);
            s.blocked_buttons
                .remove(button)
                .then_some(CaptureDecision::Suppress)
        }
        _ => None,
    }
}

/// The scroll "button" a wheel event counts as, like `WhenMouseButton` sees it.
fn scroll_button(h: i32, v: i32) -> Option<MacroButton> {
    match (h.signum(), v.signum()) {
        (_, 1) => Some(MacroButton::ScrollUp),
        (_, -1) => Some(MacroButton::ScrollDown),
        (1, _) => Some(MacroButton::ScrollRight),
        (-1, _) => Some(MacroButton::ScrollLeft),
        _ => None,
    }
}

/// Whether an armed rule takes this press or scroll. `mods` is the modifier
/// bits held, for button rules. Call [`observe`] first.
pub(super) fn decide(event: &CaptureEvent, mods: u8) -> CaptureDecision {
    if input_blocked() {
        let mut s = state();
        match event {
            CaptureEvent::KeyPress(key) => {
                s.blocked_keys.insert(key.clone());
            }
            CaptureEvent::ButtonPress(button) => {
                s.blocked_buttons.insert(button.clone());
            }
            // Releases of keys pressed before the block began go through.
            CaptureEvent::KeyRelease(_) | CaptureEvent::ButtonRelease(_) => {
                return CaptureDecision::Passthrough;
            }
            _ => {}
        }
        return CaptureDecision::Suppress;
    }
    let Ok(table) = rules().try_read() else {
        return CaptureDecision::Passthrough;
    };
    if table.is_empty() {
        return CaptureDecision::Passthrough;
    }
    let armed = |rule: &&Rule| rule.armed.load(Ordering::Relaxed);
    match event {
        CaptureEvent::KeyPress(key) => {
            let mut s = state();
            let Some(rule) = table.iter().filter(armed).find(|rule| match &rule.input {
                RuleInput::Keys { combo, extra_keys } => {
                    combo.iter().any(|c| c.matches(key))
                        && combo_held(
                            combo,
                            &s.keys,
                            *extra_keys || matches!(rule.action, RuleAction::Remap(_)),
                        )
                }
                RuleInput::Button { .. } => false,
            }) else {
                return CaptureDecision::Passthrough;
            };
            // Their physical releases come later and are kept back too.
            let lift: Vec<MacroKey> = s
                .keys
                .iter()
                .filter(|k| *k != key && rule.lift.iter().any(|c| c.matches(k)))
                .cloned()
                .collect();
            s.blocked_keys.extend(lift.iter().cloned());
            match &rule.action {
                RuleAction::Block => {
                    s.blocked_keys.insert(key.clone());
                    drop(s);
                }
                RuleAction::Remap(to) => {
                    s.remapped.push((key.clone(), to.clone()));
                    drop(s);
                }
            }
            if !lift.is_empty() {
                send_remap(RemapEvent::Lift(lift));
            }
            if let RuleAction::Remap(to) = &rule.action {
                send_remap(RemapEvent::Press(to.clone()));
            }
            CaptureDecision::Suppress
        }
        CaptureEvent::ButtonPress(_) | CaptureEvent::Scroll(..) => {
            let (button, paired) = match event {
                CaptureEvent::ButtonPress(b) => (b.clone(), true),
                CaptureEvent::Scroll(h, v) => match scroll_button(*h, *v) {
                    Some(b) => (b, false),
                    None => return CaptureDecision::Passthrough,
                },
                _ => unreachable!(),
            };
            let hit = table.iter().filter(armed).any(|rule| match &rule.input {
                RuleInput::Button {
                    button: b,
                    modifiers,
                } => *b == button && modifiers.is_none_or(|m| m == mods),
                RuleInput::Keys { .. } => false,
            });
            if !hit {
                return CaptureDecision::Passthrough;
            }
            if paired {
                state().blocked_buttons.insert(button);
            }
            CaptureDecision::Suppress
        }
        _ => CaptureDecision::Passthrough,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::key_names::parse_trigger_combo;

    // The tables are process-wide, so one test walks every case in order.
    #[test]
    fn rules_block_remap_and_pair_their_releases() {
        let caps = MacroKey::CapsLock;
        let k = MacroKey::Unicode('k');
        let remaps = subscribe_remaps();
        let block_ctrl_k = Rule::new(
            RuleInput::Keys {
                combo: parse_trigger_combo("Control+k"),
                extra_keys: false,
            },
            RuleAction::Block,
        );
        let caps_to_escape = Rule::new(
            RuleInput::Keys {
                combo: parse_trigger_combo("CapsLock"),
                extra_keys: false,
            },
            RuleAction::Remap(vec![MacroKey::Escape]),
        );
        let block_back = Rule::new(
            RuleInput::Button {
                button: MacroButton::Back,
                modifiers: None,
            },
            RuleAction::Block,
        );
        set_rules(vec![block_ctrl_k.clone(), caps_to_escape, block_back]);
        let step = |event: CaptureEvent| {
            observe(&event).unwrap_or_else(|| match event {
                CaptureEvent::KeyRelease(_) | CaptureEvent::ButtonRelease(_) => {
                    CaptureDecision::Passthrough
                }
                _ => decide(&event, 0),
            })
        };
        let pass = |d: CaptureDecision| matches!(d, CaptureDecision::Passthrough);

        // k alone isn't the combo.
        assert!(pass(step(CaptureEvent::KeyPress(k.clone()))));
        assert!(pass(step(CaptureEvent::KeyRelease(k.clone()))));
        // Control passes, Control+k is blocked, and so is k's release.
        assert!(pass(step(CaptureEvent::KeyPress(MacroKey::LControl))));
        assert!(!pass(step(CaptureEvent::KeyPress(k.clone()))));
        assert!(!pass(step(CaptureEvent::KeyRelease(k.clone()))));
        assert!(pass(step(CaptureEvent::KeyRelease(MacroKey::LControl))));

        // Shift+CapsLock still remaps: a remap lets other keys be held.
        assert!(pass(step(CaptureEvent::KeyPress(MacroKey::LShift))));
        assert!(!pass(step(CaptureEvent::KeyPress(caps.clone()))));
        assert_eq!(
            remaps.try_recv(),
            Ok(RemapEvent::Press(vec![MacroKey::Escape]))
        );
        // The release is still the remap's after the rules go away.
        set_rules(Vec::new());
        assert!(!pass(step(CaptureEvent::KeyRelease(caps.clone()))));
        assert_eq!(
            remaps.try_recv(),
            Ok(RemapEvent::Release(vec![MacroKey::Escape]))
        );
        assert!(pass(step(CaptureEvent::KeyRelease(MacroKey::LShift))));

        // A disarmed rule does nothing.
        block_ctrl_k.armed.store(false, Ordering::Relaxed);
        set_rules(vec![block_ctrl_k]);
        assert!(pass(step(CaptureEvent::KeyPress(MacroKey::LControl))));
        assert!(pass(step(CaptureEvent::KeyPress(k.clone()))));
        assert!(combo_is_held(&parse_trigger_combo("Control+k")));
        assert!(pass(step(CaptureEvent::KeyRelease(k.clone()))));
        assert!(pass(step(CaptureEvent::KeyRelease(MacroKey::LControl))));
        assert!(held_keys().is_empty());

        // The Copilot key holds Shift and Meta before its F23. A remap of
        // it takes them back and keeps their releases from other apps.
        set_rules(vec![Rule::new(
            RuleInput::Keys {
                combo: parse_trigger_combo("Copilot"),
                extra_keys: false,
            },
            RuleAction::Remap(vec![MacroKey::RControl]),
        )]);
        assert!(pass(step(CaptureEvent::KeyPress(MacroKey::LShift))));
        assert!(pass(step(CaptureEvent::KeyPress(MacroKey::Meta))));
        assert!(!pass(step(CaptureEvent::KeyPress(MacroKey::F23))));
        assert_eq!(
            remaps.try_recv(),
            Ok(RemapEvent::Lift(vec![MacroKey::LShift, MacroKey::Meta]))
        );
        assert_eq!(
            remaps.try_recv(),
            Ok(RemapEvent::Press(vec![MacroKey::RControl]))
        );
        assert!(!pass(step(CaptureEvent::KeyRelease(MacroKey::F23))));
        assert_eq!(
            remaps.try_recv(),
            Ok(RemapEvent::Release(vec![MacroKey::RControl]))
        );
        assert!(!pass(step(CaptureEvent::KeyRelease(MacroKey::Meta))));
        assert!(!pass(step(CaptureEvent::KeyRelease(MacroKey::LShift))));
        // Shift+Meta+F23 typed into a key field is the same key.
        assert!(
            !Rule::new(
                RuleInput::Keys {
                    combo: parse_trigger_combo("Shift+Meta+F23"),
                    extra_keys: false,
                },
                RuleAction::Block,
            )
            .lift
            .is_empty()
        );
        set_rules(Vec::new());

        // Blocking all input takes every press and move, but lets keys
        // held from before release, and keeps a taken press's release.
        assert!(pass(step(CaptureEvent::KeyPress(MacroKey::LShift))));
        let blocked = block_all_input();
        assert!(!pass(step(CaptureEvent::KeyPress(k.clone()))));
        assert!(!pass(step(CaptureEvent::MouseMoveRel(3, 4))));
        assert!(!pass(step(CaptureEvent::Scroll(0, 1))));
        assert!(pass(step(CaptureEvent::KeyRelease(MacroKey::LShift))));
        drop(blocked);
        assert!(!input_blocked());
        assert!(!pass(step(CaptureEvent::KeyRelease(k.clone()))));
        assert!(pass(step(CaptureEvent::MouseMoveRel(3, 4))));

        // Buttons pair their release; scrolls have none.
        set_rules(vec![
            Rule::new(
                RuleInput::Button {
                    button: MacroButton::Back,
                    modifiers: None,
                },
                RuleAction::Block,
            ),
            Rule::new(
                RuleInput::Button {
                    button: MacroButton::ScrollDown,
                    modifiers: Some(crate::hotkey_types::MOD_CTRL),
                },
                RuleAction::Block,
            ),
        ]);
        assert!(!pass(step(CaptureEvent::ButtonPress(MacroButton::Back))));
        set_rules(Vec::new());
        assert!(!pass(step(CaptureEvent::ButtonRelease(MacroButton::Back))));
        set_rules(vec![Rule::new(
            RuleInput::Button {
                button: MacroButton::ScrollDown,
                modifiers: Some(crate::hotkey_types::MOD_CTRL),
            },
            RuleAction::Block,
        )]);
        assert!(pass(decide(&CaptureEvent::Scroll(0, -1), 0)));
        assert!(!pass(decide(
            &CaptureEvent::Scroll(0, -1),
            crate::hotkey_types::MOD_CTRL
        )));
        set_rules(Vec::new());
    }
}
