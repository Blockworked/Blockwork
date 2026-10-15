//! Turning "the target is N pixels off the crosshair" into one relative
//! mouse move. Games that lock the pointer only turn the view on relative
//! moves, so this is a small proportional step, repeated with a fresh
//! detection each time.

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AimController {
    /// Mouse units per pixel of error; depends on in-game sensitivity, DPI
    /// and FOV, so it's tuned per setup.
    pub gain: f64,
    /// Largest move on each axis per step.
    pub max_step: f64,
    /// Within this many pixels of the crosshair counts as on target.
    pub deadzone: f64,
}

impl AimController {
    /// The relative move toward `target`, or `None` when it's already
    /// within the deadzone.
    pub fn step(&self, target: (f64, f64), crosshair: (f64, f64)) -> Option<(i32, i32)> {
        let (ex, ey) = (target.0 - crosshair.0, target.1 - crosshair.1);
        if ex.hypot(ey) <= self.deadzone.max(0.0) {
            return None;
        }
        let limit = self.max_step.abs();
        let axis = |e: f64| (e * self.gain).clamp(-limit, limit).round() as i32;
        let (dx, dy) = (axis(ex), axis(ey));
        // A tiny gain would round to nothing and never arrive; nudge one unit.
        if dx == 0 && dy == 0 && limit >= 1.0 && self.gain > 0.0 {
            return Some(if ex.abs() >= ey.abs() { (ex.signum() as i32, 0) } else { (0, ey.signum() as i32) });
        }
        Some((dx, dy))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const AIM: AimController = AimController { gain: 0.5, max_step: 30.0, deadzone: 4.0 };

    #[test]
    fn steps_are_proportional_to_the_error() {
        assert_eq!(AIM.step((120.0, 90.0), (100.0, 100.0)), Some((10, -5)));
    }

    #[test]
    fn steps_are_clamped_per_axis() {
        assert_eq!(AIM.step((1100.0, 0.0), (100.0, 100.0)), Some((30, -30)));
    }

    #[test]
    fn inside_the_deadzone_nothing_moves() {
        assert_eq!(AIM.step((103.0, 102.0), (100.0, 100.0)), None);
        assert_eq!(AIM.step((100.0, 100.0), (100.0, 100.0)), None);
    }

    #[test]
    fn tiny_gains_still_make_progress() {
        let slow = AimController { gain: 0.01, ..AIM };
        assert_eq!(slow.step((90.0, 103.0), (100.0, 100.0)), Some((-1, 0)));
        let off = AimController { gain: 0.0, ..AIM };
        assert_eq!(off.step((90.0, 100.0), (100.0, 100.0)), Some((0, 0)));
    }
}
