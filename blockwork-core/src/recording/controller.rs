//! Controller recording: physical controller changes become virtual
//! controller blocks on the recording's timeline.
use super::{elapsed_at, queue_at, record_at};
use crate::input::controllers::{self, Controller};
use crate::input::gamepad::{GamepadAxis, GamepadStick};
use crate::input::types::Direction;
use crate::input::value::Value;
use crate::macros::{Instruction, InstructionKind};
use std::sync::Mutex;
use std::sync::atomic::Ordering;
use std::time::Duration;

/// Stick and trigger values are kept to two decimals; smaller moves are noise.
const STEP: f64 = 0.01;

fn quantize(value: f64, rest_zone: f64) -> f64 {
    if !value.is_finite() || value.abs() < rest_zone {
        return 0.0;
    }
    ((value.clamp(-1.0, 1.0) / STEP).round() * STEP * 100.0).round() / 100.0
}

/// Converts snapshots of one controller into the blocks that reproduce them.
#[derive(Debug, Default)]
pub struct ControllerRecorder {
    profile: Option<String>,
    last: Option<Controller>,
    /// Every controller's last state until one is picked.
    seen: Vec<Controller>,
    /// Something isn't at rest, so the recording ends with a reset.
    pub dirty: bool,
    rest_zone: f64,
}

impl ControllerRecorder {
    pub fn new(rest_zone: f64) -> Self {
        Self {
            rest_zone: if rest_zone.is_finite() {
                rest_zone.clamp(0.0, 0.95)
            } else {
                0.0
            },
            ..Self::default()
        }
    }

    /// Takes the controllers as they are now and returns the blocks for
    /// what changed. The first controller to change is the one recorded;
    /// the virtual controller is a single device, so others are ignored.
    pub fn update(&mut self, controllers: &[Controller]) -> Vec<InstructionKind> {
        let Some(profile) = self.profile.clone() else {
            // Without a recorded controller yet, any change picks one.
            let changed = controllers.iter().find(|now| {
                match self.seen.iter().find(|seen| seen.profile == now.profile) {
                    Some(seen) => self.differs(seen, now),
                    None => !self.at_rest(now),
                }
            });
            let Some(now) = changed else {
                self.seen = controllers.to_vec();
                return Vec::new();
            };
            self.profile = Some(now.profile.clone());
            self.last = Some(
                self.seen
                    .iter()
                    .find(|seen| seen.profile == now.profile)
                    .cloned()
                    .unwrap_or_else(|| Self::resting(now)),
            );
            self.seen.clear();
            return self.update(controllers);
        };
        let Some(now) = controllers.iter().find(|c| c.profile == profile) else {
            return Vec::new();
        };
        let previous = self
            .last
            .replace(now.clone())
            .unwrap_or_else(|| Self::resting(now));
        let mut out = Vec::new();
        for (button, pressed) in &now.buttons {
            if previous.buttons.get(button).copied().unwrap_or(false) != *pressed {
                out.push(InstructionKind::GamepadButton {
                    button: *button,
                    direction: if *pressed {
                        Direction::Press
                    } else {
                        Direction::Release
                    },
                });
            }
        }
        for (stick, x, y) in [
            (GamepadStick::Left, GamepadAxis::LeftX, GamepadAxis::LeftY),
            (
                GamepadStick::Right,
                GamepadAxis::RightX,
                GamepadAxis::RightY,
            ),
        ] {
            let (ox, oy) = (self.axis(&previous, x), self.axis(&previous, y));
            let (nx, ny) = (self.axis(now, x), self.axis(now, y));
            if ox != nx || oy != ny {
                out.push(InstructionKind::GamepadStick {
                    stick,
                    x: Value::number(nx),
                    y: Value::number(ny),
                });
            }
        }
        for (left, axis) in [
            (true, GamepadAxis::LeftTrigger),
            (false, GamepadAxis::RightTrigger),
        ] {
            let (old, new) = (self.axis(&previous, axis), self.axis(now, axis));
            if old != new {
                out.push(InstructionKind::GamepadTrigger {
                    left,
                    amount: Value::number(new.max(0.0)),
                });
            }
        }
        self.dirty = !self.at_rest(now);
        out
    }

    fn axis(&self, controller: &Controller, axis: GamepadAxis) -> f64 {
        quantize(
            controller.axes.get(&axis).copied().unwrap_or(0.0),
            self.rest_zone,
        )
    }

    fn differs(&self, a: &Controller, b: &Controller) -> bool {
        a.buttons != b.buttons
            || crate::input::gamepad::AXES
                .iter()
                .any(|axis| self.axis(a, *axis) != self.axis(b, *axis))
    }

    fn at_rest(&self, controller: &Controller) -> bool {
        !controller.buttons.values().any(|p| *p)
            && crate::input::gamepad::AXES
                .iter()
                .all(|axis| self.axis(controller, *axis) == 0.0)
    }

    fn resting(like: &Controller) -> Controller {
        Controller {
            buttons: like.buttons.keys().map(|b| (*b, false)).collect(),
            axes: like.axes.keys().map(|a| (*a, 0.0)).collect(),
            ..like.clone()
        }
    }
}

static DIRTY: Mutex<Option<Duration>> = Mutex::new(None);

pub(super) fn start() {
    let zone = crate::config::load_settings().stick_deadzone();
    let (baseline, subscription) = match controllers::subscribe() {
        Ok(found) => found,
        Err(error) => {
            crate::run_log::error(
                &crate::run_log::Source::app("Controller recording"),
                format!("Controllers aren't recorded: {error}"),
            );
            return;
        }
    };
    *DIRTY.lock().unwrap_or_else(|p| p.into_inner()) = None;
    let _ = std::thread::Builder::new()
        .name("controller-recording".into())
        .spawn(move || {
            let mut recorder = ControllerRecorder::new(zone);
            recorder.update(&baseline);
            while super::RECORDING_ACTIVE.load(Ordering::Relaxed) {
                if subscription.overflowed() {
                    crate::run_log::error(
                        &crate::run_log::Source::app("Controller recording"),
                        "Controller events arrived faster than they could be recorded; some were skipped",
                    );
                }
                let Ok(event) = subscription.try_recv() else {
                    std::thread::sleep(Duration::from_millis(4));
                    continue;
                };
                let at = elapsed_at(event.at);
                for kind in recorder.update(&event.controllers) {
                    record_at(Instruction::new(kind), at);
                }
                *DIRTY.lock().unwrap_or_else(|p| p.into_inner()) =
                    recorder.dirty.then_some(at);
            }
        });
}

/// Ends a recording that left buttons held or sticks off center with a reset,
/// so its playback never leaves the virtual controller pressed.
pub(super) fn finish() {
    if let Some(at) = DIRTY.lock().unwrap_or_else(|p| p.into_inner()).take() {
        queue_at(Instruction::new(InstructionKind::ResetGamepad), at);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::gamepad::GamepadButton;
    use std::collections::BTreeMap;

    fn pad(profile: &str, south: bool, lx: f64, lt: f64) -> Controller {
        Controller {
            profile: profile.into(),
            instance: 0,
            name: "Pad".into(),
            buttons: BTreeMap::from([(GamepadButton::South, south)]),
            axes: BTreeMap::from([
                (GamepadAxis::LeftX, lx),
                (GamepadAxis::LeftY, 0.0),
                (GamepadAxis::LeftTrigger, lt),
            ]),
        }
    }

    #[test]
    fn changes_become_blocks_and_noise_is_ignored() {
        let mut recorder = ControllerRecorder::new(0.1);
        assert!(recorder.update(&[pad("a", false, 0.0, 0.0)]).is_empty());
        // Stick noise inside the rest zone doesn't pick a controller.
        assert!(recorder.update(&[pad("a", false, 0.05, 0.0)]).is_empty());
        let blocks = recorder.update(&[pad("a", true, 0.0, 0.0)]);
        assert!(matches!(
            blocks.as_slice(),
            [InstructionKind::GamepadButton {
                button: GamepadButton::South,
                direction: Direction::Press
            }]
        ));
        assert!(recorder.dirty);
        let blocks = recorder.update(&[pad("a", true, 0.504, 0.25)]);
        assert_eq!(blocks.len(), 2);
        match &blocks[0] {
            InstructionKind::GamepadStick { x, .. } => assert_eq!(*x, Value::number(0.5)),
            other => panic!("{other:?}"),
        }
        let blocks = recorder.update(&[pad("a", false, 0.0, 0.0)]);
        assert_eq!(blocks.len(), 3);
        assert!(!recorder.dirty);
    }

    #[test]
    fn only_the_first_controller_to_change_is_recorded() {
        let mut recorder = ControllerRecorder::new(0.1);
        recorder.update(&[pad("a", false, 0.0, 0.0), pad("b", false, 0.0, 0.0)]);
        let blocks = recorder.update(&[pad("a", false, 0.0, 0.0), pad("b", true, 0.0, 0.0)]);
        assert_eq!(blocks.len(), 1);
        let blocks = recorder.update(&[pad("a", true, 0.0, 0.0), pad("b", true, 0.0, 0.0)]);
        assert!(blocks.is_empty());
        // A controller already held when recording starts is recorded from rest.
        let mut recorder = ControllerRecorder::new(0.1);
        let blocks = recorder.update(&[pad("c", true, 0.0, 0.0)]);
        assert_eq!(blocks.len(), 1);
        // A disconnected recorded controller records nothing until it returns.
        assert!(recorder.update(&[]).is_empty());
        assert!(quantize(f64::NAN, 0.0) == 0.0);
    }
}
