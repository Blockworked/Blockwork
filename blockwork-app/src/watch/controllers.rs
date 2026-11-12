//! Controller triggers consume ordered changes, including short button presses.
use super::{ReportOnce, StrandKey, Target, fires_on_change, variable_env};
use crate::{AppHandle, scheduled_run, state::SharedState};
use blockwork_core::input::{
    controllers,
    gamepad::{GamepadAxis, GamepadButton, above_threshold, deadzone},
};
use blockwork_core::macros::InstructionKind;
use std::collections::HashMap;
use std::hash::Hash;
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone, Debug, PartialEq)]
enum Condition {
    Button(GamepadButton, bool),
    Connected(bool),
    Axis(GamepadAxis, f64, bool),
}

impl Condition {
    fn fires_on(&self) -> bool {
        match *self {
            Condition::Button(_, v) | Condition::Connected(v) | Condition::Axis(_, _, v) => v,
        }
    }
}

struct Watched<K> {
    key: K,
    profile: String,
    condition: Condition,
}

/// The condition's state now. `None` means unknown: a button or axis on a
/// controller that isn't connected. Unknown never fires; the next known
/// state becomes a fresh baseline, so a disconnect doesn't read as a release
/// and a reconnect with a button held doesn't read as a press.
fn sample<K>(
    w: &Watched<K>,
    devices: &[controllers::Controller],
    old: Option<bool>,
    zone: f64,
) -> Result<Option<bool>, String> {
    let device = controllers::select(devices, &w.profile)?;
    Ok(match w.condition {
        Condition::Connected(_) => Some(device.is_some()),
        Condition::Button(button, _) => {
            device.map(|d| d.buttons.get(&button).copied().unwrap_or(false))
        }
        Condition::Axis(axis, threshold, _) => device.map(|d| {
            above_threshold(
                old.unwrap_or(false),
                deadzone(d.axes.get(&axis).copied().unwrap_or(0.0), zone),
                threshold,
            )
        }),
    })
}

/// What each watched condition was, across header edits and device changes.
struct Tracker<K> {
    last: HashMap<K, bool>,
    definitions: HashMap<K, String>,
}

impl<K: Clone + Eq + Hash> Tracker<K> {
    fn new() -> Self {
        Self {
            last: HashMap::new(),
            definitions: HashMap::new(),
        }
    }

    fn clear(&mut self) {
        self.last.clear();
    }

    /// New and edited headers take a baseline before consuming changes.
    fn sync(&mut self, watched: &[Watched<K>], devices: &[controllers::Controller], zone: f64) {
        for w in watched {
            let signature = format!("{}:{:?}", w.profile, w.condition);
            if self.definitions.get(&w.key) != Some(&signature) {
                self.last.remove(&w.key);
                self.definitions.insert(w.key.clone(), signature);
            }
            if !self.last.contains_key(&w.key)
                && let Ok(Some(value)) = sample(w, devices, None, zone)
            {
                self.last.insert(w.key.clone(), value);
            }
        }
        self.last
            .retain(|key, _| watched.iter().any(|w| &w.key == key));
        self.definitions
            .retain(|key, _| watched.iter().any(|w| &w.key == key));
    }

    /// One ordered change; returns the indices of `watched` that fire.
    fn event(
        &mut self,
        watched: &[Watched<K>],
        devices: &[controllers::Controller],
        zone: f64,
        mut report: impl FnMut(Result<(), String>),
    ) -> Vec<usize> {
        let mut fired = Vec::new();
        for (index, w) in watched.iter().enumerate() {
            let before = self.last.get(&w.key).copied();
            match sample(w, devices, before, zone) {
                Ok(Some(now)) => {
                    report(Ok(()));
                    if fires_on_change(before, now, w.condition.fires_on()) {
                        fired.push(index);
                    }
                    self.last.insert(w.key.clone(), now);
                }
                Ok(None) => {
                    report(Ok(()));
                    self.last.remove(&w.key);
                }
                Err(error) => {
                    report(Err(error));
                    self.last.remove(&w.key);
                }
            }
        }
        fired
    }
}

type Watch = (Vec<Watched<StrandKey>>, Vec<Target>);

fn watched(state: &crate::state::AppState) -> Watch {
    let mut watched = vec![];
    let mut targets = vec![];
    for mac in scheduled_run::live_macros(state) {
        if !super::permitted(mac, "input") {
            continue;
        }
        let env = variable_env(mac);
        for strand in &mac.strands {
            let Some(header) = strand.instructions.first() else {
                continue;
            };
            let (profile, condition) = match &header.kind {
                InstructionKind::WhenGamepadButton {
                    controller,
                    button,
                    pressed,
                } => (controller, Condition::Button(*button, *pressed)),
                InstructionKind::WhenGamepadConnection {
                    controller,
                    connected,
                } => (controller, Condition::Connected(*connected)),
                InstructionKind::WhenGamepadAxis {
                    controller,
                    axis,
                    threshold,
                    above,
                } => {
                    let Some(threshold) = super::header_value(threshold, &env)
                        .ok()
                        .and_then(|v| v.eval_number().ok())
                        .filter(|n| n.is_finite() && (0.05..=1.0).contains(n))
                    else {
                        continue;
                    };
                    (controller, Condition::Axis(*axis, threshold, *above))
                }
                _ => continue,
            };
            let Ok(profile) = super::header_value(profile, &env).and_then(|v| v.eval_text()) else {
                continue;
            };
            let target = Target::new(mac, strand);
            watched.push(Watched {
                key: target.key.clone(),
                profile,
                condition,
            });
            targets.push(target);
        }
    }
    (watched, targets)
}

pub(crate) fn start(state: SharedState, app: AppHandle) {
    let _ = std::thread::Builder::new()
        .name("controller-watch".into())
        .spawn(move || {
            let mut subscription = None;
            let mut devices = vec![];
            let mut tracker = Tracker::new();
            let mut report = ReportOnce::default();
            loop {
                std::thread::sleep(Duration::from_millis(10));
                let ((watched, targets), emulator, speed, zone) = {
                    let Ok(s) = state.lock() else { continue };
                    if !s.controller_input_enabled || s.triggers_paused {
                        subscription = None;
                        tracker.clear();
                        continue;
                    }
                    let Some(emulator) = s.emulator.as_ref().map(Arc::clone) else {
                        continue;
                    };
                    (
                        watched(&s),
                        emulator,
                        s.global_speed_multiplier,
                        s.controller_deadzone,
                    )
                };
                if watched.is_empty() {
                    subscription = None;
                    tracker.clear();
                    continue;
                }
                if subscription.is_none() {
                    let Some((baseline, stream)) =
                        report.check("Controller triggers", controllers::subscribe())
                    else {
                        std::thread::sleep(Duration::from_millis(250));
                        continue;
                    };
                    devices = baseline;
                    subscription = Some(stream);
                    tracker.clear();
                }
                let stream = subscription.as_ref().unwrap();
                if stream.overflowed() {
                    blockwork_core::run_log::error(
                        &blockwork_core::run_log::Source::app("Controller triggers"),
                        "Controller event queue overflowed; triggers were resynchronized without replaying stale presses",
                    );
                    while stream.try_recv().is_ok() {}
                    devices = controllers::list().unwrap_or_default();
                    tracker.clear();
                }
                tracker.sync(&watched, &devices, zone);
                for _ in 0..256 {
                    let Ok(event) = stream.try_recv() else { break };
                    devices = event.controllers;
                    let fired = tracker.event(&watched, &devices, zone, |result| {
                        report.check("Controller selector", result);
                    });
                    for index in fired {
                        targets[index].fire(&emulator, speed, &state, &app);
                    }
                }
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn pad(profile: &str, south: bool, x: f64) -> controllers::Controller {
        controllers::Controller {
            profile: profile.into(),
            instance: 0,
            name: "Pad".into(),
            buttons: BTreeMap::from([(GamepadButton::South, south)]),
            axes: BTreeMap::from([(GamepadAxis::LeftX, x)]),
        }
    }

    fn watch(key: u8, condition: Condition) -> Watched<u8> {
        Watched {
            key,
            profile: "a".into(),
            condition,
        }
    }

    fn run(
        tracker: &mut Tracker<u8>,
        w: &[Watched<u8>],
        devices: &[controllers::Controller],
    ) -> Vec<usize> {
        tracker.event(w, devices, 0.1, |_| {})
    }

    #[test]
    fn short_presses_fire_once_and_baselines_never_fire() {
        let w = [
            watch(0, Condition::Button(GamepadButton::South, true)),
            watch(1, Condition::Button(GamepadButton::South, false)),
        ];
        let mut tracker = Tracker::new();
        // Held when the strand appears: a baseline, not a press.
        tracker.sync(&w, &[pad("a", true, 0.0)], 0.1);
        assert!(run(&mut tracker, &w, &[pad("a", true, 0.0)]).is_empty());
        assert_eq!(run(&mut tracker, &w, &[pad("a", false, 0.0)]), vec![1]);
        assert_eq!(run(&mut tracker, &w, &[pad("a", true, 0.0)]), vec![0]);
    }

    #[test]
    fn disconnect_and_reconnect_never_fake_presses_or_releases() {
        let w = [
            watch(0, Condition::Button(GamepadButton::South, true)),
            watch(1, Condition::Button(GamepadButton::South, false)),
            watch(2, Condition::Connected(false)),
            watch(3, Condition::Connected(true)),
            watch(4, Condition::Axis(GamepadAxis::LeftX, 0.5, false)),
        ];
        let mut tracker = Tracker::new();
        tracker.sync(&w, &[pad("a", true, 0.9)], 0.1);
        // Unplugged with the button held and the stick pushed: only the
        // disconnect trigger fires.
        assert_eq!(run(&mut tracker, &w, &[]), vec![2]);
        // Plugged back with the button still held: only the connect trigger.
        assert_eq!(run(&mut tracker, &w, &[pad("a", true, 0.9)]), vec![3]);
        assert_eq!(run(&mut tracker, &w, &[pad("a", false, 0.0)]), vec![1, 4]);
    }

    #[test]
    fn duplicate_profiles_report_and_never_fire() {
        let w = [watch(0, Condition::Button(GamepadButton::South, true))];
        let mut tracker = Tracker::new();
        tracker.sync(&w, &[pad("a", false, 0.0)], 0.1);
        let mut errors = 0;
        let both = [pad("a", true, 0.0), pad("a", true, 0.0)];
        assert!(
            tracker
                .event(&w, &both, 0.1, |r| errors += usize::from(r.is_err()))
                .is_empty()
        );
        assert_eq!(errors, 1);
        // Back to one controller with the button held: a baseline again.
        assert!(run(&mut tracker, &w, &[pad("a", true, 0.0)]).is_empty());
    }

    #[test]
    fn editing_a_header_takes_a_new_baseline_and_removed_strands_are_forgotten() {
        let mut w = vec![watch(0, Condition::Axis(GamepadAxis::LeftX, 0.5, true))];
        let mut tracker = Tracker::new();
        tracker.sync(&w, &[pad("a", false, 0.9)], 0.1);
        w[0].condition = Condition::Axis(GamepadAxis::LeftX, 0.3, true);
        tracker.sync(&w, &[pad("a", false, 0.9)], 0.1);
        assert!(run(&mut tracker, &w, &[pad("a", false, 0.9)]).is_empty());
        tracker.sync(&[], &[], 0.1);
        assert!(tracker.last.is_empty() && tracker.definitions.is_empty());
    }

    #[test]
    fn axis_noise_around_the_threshold_fires_once() {
        let w = [watch(0, Condition::Axis(GamepadAxis::LeftX, 0.5, true))];
        let mut tracker = Tracker::new();
        tracker.sync(&w, &[pad("a", false, 0.0)], 0.0);
        let mut fired = 0;
        for x in [0.49, 0.51, 0.49, 0.52, 0.47, 0.5, 0.4, 0.55] {
            fired += tracker.event(&w, &[pad("a", false, x)], 0.0, |_| {}).len();
        }
        // Up at 0.51, down only below the band (0.4), up again at 0.55.
        assert_eq!(fired, 2);
    }
}
