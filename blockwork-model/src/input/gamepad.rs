//! Logical controls, independent of a platform's device IDs and button numbers.
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum GamepadButton {
    South,
    East,
    North,
    West,
    LeftShoulder,
    RightShoulder,
    LeftStick,
    RightStick,
    Select,
    Start,
    Mode,
    DPadUp,
    DPadDown,
    DPadLeft,
    DPadRight,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum GamepadAxis {
    LeftX,
    LeftY,
    RightX,
    RightY,
    LeftTrigger,
    RightTrigger,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GamepadStick {
    Left,
    Right,
}
pub const BUTTONS: &[GamepadButton] = &[
    GamepadButton::South,
    GamepadButton::East,
    GamepadButton::North,
    GamepadButton::West,
    GamepadButton::LeftShoulder,
    GamepadButton::RightShoulder,
    GamepadButton::LeftStick,
    GamepadButton::RightStick,
    GamepadButton::Select,
    GamepadButton::Start,
    GamepadButton::Mode,
    GamepadButton::DPadUp,
    GamepadButton::DPadDown,
    GamepadButton::DPadLeft,
    GamepadButton::DPadRight,
];
pub const AXES: &[GamepadAxis] = &[
    GamepadAxis::LeftX,
    GamepadAxis::LeftY,
    GamepadAxis::RightX,
    GamepadAxis::RightY,
    GamepadAxis::LeftTrigger,
    GamepadAxis::RightTrigger,
];
/// A deadzone with continuous output outside it, and no NaN propagation.
pub fn deadzone(value: f64, zone: f64) -> f64 {
    if !value.is_finite() || !zone.is_finite() {
        return 0.0;
    }
    let zone = zone.clamp(0.0, 0.95);
    let value = value.clamp(-1.0, 1.0);
    if value.abs() <= zone {
        0.0
    } else {
        value.signum() * (value.abs() - zone) / (1.0 - zone)
    }
}
/// Hysteresis prevents noisy axes from repeatedly firing at a threshold.
pub fn above_threshold(previous: bool, value: f64, threshold: f64) -> bool {
    value.abs()
        >= if previous {
            (threshold - 0.05).max(0.0)
        } else {
            threshold
        }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn deadzones_and_hysteresis_are_bounded_and_stable() {
        assert_eq!(deadzone(0.05, 0.12), 0.0);
        assert_eq!(deadzone(-1.0, 0.12), -1.0);
        assert_eq!(deadzone(f64::NAN, 0.12), 0.0);
        assert!(above_threshold(true, 0.48, 0.5));
        assert!(!above_threshold(false, 0.48, 0.5));
        assert!(!above_threshold(true, 0.40, 0.5));
    }
}

/// A full logical virtual-controller state. Y is positive upward; triggers are 0..1023.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct GamepadState {
    pub buttons: std::collections::BTreeSet<GamepadButton>,
    pub left_x: i16,
    pub left_y: i16,
    pub right_x: i16,
    pub right_y: i16,
    pub left_trigger: u16,
    pub right_trigger: u16,
}
impl GamepadState {
    pub fn validate(&self) -> Result<(), String> {
        if [self.left_x, self.left_y, self.right_x, self.right_y].contains(&i16::MIN)
            || self.left_trigger > 1023
            || self.right_trigger > 1023
        {
            return Err("Virtual controller state is outside its normalized range".into());
        }
        Ok(())
    }
}
