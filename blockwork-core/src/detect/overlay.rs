//! Telling a game's HUD from the world: a box that stays put on screen
//! while the view turns toward it is drawn over the game, so aiming at it
//! can never succeed. Such spots are remembered for a while and skipped.

use super::Detection;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// How long an overlay spot is skipped.
const REMEMBER: Duration = Duration::from_secs(10);
/// The view must turn at least this far (pixels), over at least `WINDOW`
/// seconds, to judge: long enough to outlast the frame lag.
const MIN_TURN: f64 = 60.0;
const WINDOW: f64 = 0.08;
/// HUD boxes hold within a pixel or two; a target in the world, even one
/// running at the view's own speed, drifts more than this.
const STILL_PX: f64 = 5.0;
/// How close in place and size a box must be to a remembered one to be skipped.
const SAME: f64 = 0.25;
/// Targets this close to the crosshair are being held, not chased.
const NEAR: f64 = 40.0;

type Px = (f64, f64);

/// Watches one aim target.
#[derive(Debug, Default)]
pub struct OverlayCheck {
    /// When it was last judged, with the error and turn then.
    anchor: Option<(f64, Px, Px)>,
}

impl OverlayCheck {
    pub fn reset(&mut self) {
        self.anchor = None;
    }

    /// A frame at time `t` saw the target `error` pixels off the crosshair
    /// after the view had turned `turned` pixels in all. True once the view
    /// has turned well toward it but it hasn't moved on screen.
    pub fn is_overlay(&mut self, t: f64, error: Px, turned: Px) -> bool {
        let Some((at, e0, turned0)) = self.anchor else {
            self.anchor = Some((t, error, turned));
            return false;
        };
        let turn = (turned.0 - turned0.0).hypot(turned.1 - turned0.1);
        if t - at < WINDOW {
            return false;
        }
        self.anchor = Some((t, error, turned));
        let moved = (error.0 - e0.0).hypot(error.1 - e0.1);
        turn >= MIN_TURN && moved < STILL_PX && error.0.hypot(error.1) >= NEAR
    }
}

/// A remembered overlay: scope, offset from the crosshair, size, and when.
type Spot = (String, Px, Px, Instant);

static SPOTS: Mutex<Vec<Spot>> = Mutex::new(Vec::new());

/// Skips boxes around `d` (relative to `crosshair`) in `scope` for a while.
pub fn remember(scope: &str, d: &Detection, crosshair: (f64, f64)) {
    let (x, y) = d.center();
    let mut spots = SPOTS.lock().unwrap_or_else(|e| e.into_inner());
    spots.push((scope.to_string(), (x - crosshair.0, y - crosshair.1), (d.w, d.h), Instant::now()));
}

/// `detections` without the ones on a remembered overlay spot.
pub fn filter(scope: &str, detections: Vec<Detection>, crosshair: (f64, f64)) -> Vec<Detection> {
    let mut spots = SPOTS.lock().unwrap_or_else(|e| e.into_inner());
    spots.retain(|s| s.3.elapsed() < REMEMBER);
    if spots.is_empty() {
        return detections;
    }
    detections
        .into_iter()
        .filter(|d| {
            let (x, y) = d.center();
            let (ox, oy) = (x - crosshair.0, y - crosshair.1);
            // Only the same box: a real target can stand right next to the HUD.
            !spots.iter().any(|(s, at, size, _)| {
                s == scope
                    && (ox - at.0).abs() <= size.0 * SAME
                    && (oy - at.1).abs() <= size.1 * SAME
                    && (d.w / size.0 - 1.0).abs() <= SAME
                    && (d.h / size.1 - 1.0).abs() <= SAME
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_box_that_ignores_the_turn_is_an_overlay() {
        let mut check = OverlayCheck::default();
        assert!(!check.is_overlay(0.0, (0.0, 209.0), (0.0, 0.0)));
        assert!(!check.is_overlay(0.05, (0.0, 209.0), (0.0, 30.0)));
        assert!(check.is_overlay(0.1, (1.0, 208.0), (0.0, 80.0)));
    }

    #[test]
    fn a_target_that_comes_closer_is_real() {
        let mut check = OverlayCheck::default();
        check.is_overlay(0.0, (0.0, 200.0), (0.0, 0.0));
        assert!(!check.is_overlay(0.3, (0.0, 60.0), (0.0, 140.0)));
    }

    #[test]
    fn a_target_running_at_the_views_speed_is_real() {
        let mut check = OverlayCheck::default();
        check.is_overlay(0.0, (107.6, 41.4), (0.0, 0.0));
        assert!(!check.is_overlay(0.1, (95.2, 34.6), (90.0, 0.0)));
    }

    #[test]
    fn a_held_moving_target_is_real() {
        // Tracking a strafing player: it stays centered while the view turns.
        let mut check = OverlayCheck::default();
        check.is_overlay(0.0, (3.0, 1.0), (0.0, 0.0));
        assert!(!check.is_overlay(0.3, (2.0, -1.0), (180.0, 0.0)));
    }

    #[test]
    fn remembered_spots_are_skipped_in_their_scope() {
        let hud = Detection { x: 380.0, y: 420.0, w: 100.0, h: 30.0, label: "player".into(), score: 0.5, id: None };
        let real = Detection { x: 400.0, y: 200.0, w: 30.0, h: 80.0, ..hud.clone() };
        // A player standing over the same spot is a different box.
        let nearby = Detection { x: 410.0, y: 360.0, w: 40.0, h: 100.0, ..hud.clone() };
        remember("test-scope", &hud, (427.0, 240.0));
        let kept = filter("test-scope", vec![hud.clone(), real.clone(), nearby.clone()], (427.0, 240.0));
        assert_eq!(kept, vec![real.clone(), nearby]);
        assert_eq!(filter("other-scope", vec![hud.clone()], (427.0, 240.0)).len(), 1);
    }
}
