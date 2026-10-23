//! Turning "the target is N pixels off the crosshair" into relative mouse
//! moves. Games that lock the pointer only turn the view on relative moves.
//! [`AimController::step`] is one proportional step; [`Glide`] moves
//! continuously between detections, the way a hand would.

use std::collections::VecDeque;

/// Window height that gains are given for. A first-person view turns the
/// same angle across more pixels in a taller window.
pub const REFERENCE_HEIGHT: f64 = 1000.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AimController {
    /// Mouse units per pixel of error in a [`REFERENCE_HEIGHT`] tall window;
    /// depends on in-game sensitivity and FOV, so it's tuned per setup.
    pub gain: f64,
    /// Largest move on each axis per step.
    pub max_step: f64,
    /// Within this many pixels of the crosshair counts as on target.
    pub deadzone: f64,
}

impl AimController {
    /// This controller with its gain scaled to a window `height` tall.
    pub fn for_window(self, height: f64) -> AimController {
        AimController { gain: self.gain * REFERENCE_HEIGHT / height.max(1.0), ..self }
    }

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

/// How quickly a glide closes in, in rad/s: about 200 ms to cover a flick.
const OMEGA: f64 = 24.0;
/// A little under critical damping: a slight overshoot, then a correction.
const ZETA: f64 = 0.8;
/// Mouse noise grows with speed, as a hand's does; this is its share.
const SPEED_NOISE: f64 = 0.04;
/// Goals are extrapolated at most this far, so a lost target can't drag it.
const MAX_LEAD: f64 = 0.15;
/// How much of the past is kept to answer [`Glide::emitted_at`].
const HISTORY: f64 = 1.0;

/// Smooth pointer motion toward a goal in mouse units, ticked at a high
/// rate: a damped spring, so speed rises and falls like a hand's, with
/// speed-scaled noise and a lead for goals that are moving.
#[derive(Debug, Clone)]
pub struct Glide {
    aim: AimController,
    /// Continuous position and velocity, in mouse units and units/s.
    pos: (f64, f64),
    vel: (f64, f64),
    /// Whole units sent so far.
    sent: (i64, i64),
    goal: Option<Goal>,
    history: VecDeque<(f64, (i64, i64))>,
    rng: u64,
}

#[derive(Debug, Clone, Copy)]
struct Goal {
    at: f64,
    pos: (f64, f64),
    vel: (f64, f64),
}

impl Glide {
    pub fn new(aim: AimController, seed: u64) -> Glide {
        Glide { aim, pos: (0.0, 0.0), vel: (0.0, 0.0), sent: (0, 0), goal: None, history: VecDeque::new(), rng: seed | 1 }
    }

    /// Follows a window resize; see [`AimController::for_window`].
    pub fn set_aim(&mut self, aim: AimController) {
        self.aim = aim;
    }

    /// Units sent by time `t` (seconds), for matching a frame captured then.
    pub fn emitted_at(&self, t: f64) -> (i64, i64) {
        match self.history.iter().rev().find(|(at, _)| *at <= t) {
            Some(h) => h.1,
            // Older than the history: the oldest kept is the best guess.
            None => self.history.front().map_or(self.sent, |h| h.1),
        }
    }

    /// A detection from a frame captured at `t`: the target was `error`
    /// pixels off the crosshair. Moves sent since then are already on their
    /// way, so the goal is where the pointer was plus the error.
    pub fn observe(&mut self, t: f64, error: (f64, f64)) {
        let base = self.emitted_at(t);
        let pos = (base.0 as f64 + error.0 * self.aim.gain, base.1 as f64 + error.1 * self.aim.gain);
        // A moving target shows up as a goal that keeps shifting: lead it.
        let vel = match self.goal {
            Some(g) if t > g.at && t - g.at < 0.3 => {
                let raw = ((pos.0 - g.pos.0) / (t - g.at), (pos.1 - g.pos.1) / (t - g.at));
                let limit = self.max_speed() * 0.5;
                let blend = |old: f64, new: f64| (old * 0.6 + new * 0.4).clamp(-limit, limit);
                (blend(g.vel.0, raw.0), blend(g.vel.1, raw.1))
            }
            _ => (0.0, 0.0),
        };
        self.goal = Some(Goal { at: t, pos, vel });
    }

    /// The target wasn't seen: hold the last goal instead of leading it.
    pub fn hold(&mut self, t: f64) {
        let pos = self.goal_at(t);
        if let Some(g) = &mut self.goal {
            *g = Goal { at: t, pos, vel: (0.0, 0.0) };
        }
    }

    fn goal_at(&self, t: f64) -> (f64, f64) {
        match self.goal {
            Some(g) => {
                let lead = (t - g.at).clamp(0.0, MAX_LEAD);
                (g.pos.0 + g.vel.0 * lead, g.pos.1 + g.vel.1 * lead)
            }
            None => self.pos,
        }
    }

    /// Units per second; `max_step` is the most per 10 ms.
    fn max_speed(&self) -> f64 {
        self.aim.max_step.abs() * 100.0
    }

    fn noise(&mut self) -> f64 {
        // xorshift: plenty for jitter, and repeatable in tests.
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        (self.rng >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0
    }

    /// Advances to time `t` by `dt` seconds; returns the whole units to send.
    pub fn tick(&mut self, t: f64, dt: f64) -> (i32, i32) {
        if self.goal.is_some() {
            let goal = self.goal_at(t);
            let goal_vel = self.goal.map_or((0.0, 0.0), |g| if t - g.at <= MAX_LEAD { g.vel } else { (0.0, 0.0) });
            let speed = self.vel.0.hypot(self.vel.1);
            let limit = self.max_speed();
            let noise = (self.noise(), self.noise());
            let axis = |p: f64, v: f64, g: f64, gv: f64, n: f64| {
                let acc = OMEGA * OMEGA * (g - p) + 2.0 * ZETA * OMEGA * (gv - v) + speed * SPEED_NOISE * OMEGA * n;
                (v + acc * dt).clamp(-limit, limit)
            };
            self.vel = (
                axis(self.pos.0, self.vel.0, goal.0, goal_vel.0, noise.0),
                axis(self.pos.1, self.vel.1, goal.1, goal_vel.1, noise.1),
            );
            self.pos = (self.pos.0 + self.vel.0 * dt, self.pos.1 + self.vel.1 * dt);
        }
        let next = (self.pos.0.round() as i64, self.pos.1.round() as i64);
        let step = ((next.0 - self.sent.0) as i32, (next.1 - self.sent.1) as i32);
        self.sent = next;
        self.history.push_back((t, self.sent));
        while self.history.front().is_some_and(|(at, _)| t - at > HISTORY) {
            self.history.pop_front();
        }
        step
    }

    /// How far the view had turned by time `t`, in pixels.
    pub fn turned_px(&self, t: f64) -> (f64, f64) {
        let sent = self.emitted_at(t);
        let gain = self.aim.gain.abs().max(1e-6);
        (sent.0 as f64 / gain, sent.1 as f64 / gain)
    }

    /// Where the target should be now, in pixels off the crosshair, by the
    /// latest goal and the moves sent since.
    pub fn expected_px(&self, t: f64) -> Option<(f64, f64)> {
        self.goal?;
        let g = self.goal_at(t);
        let gain = self.aim.gain.abs().max(1e-6);
        Some(((g.0 - self.sent.0 as f64) / gain, (g.1 - self.sent.1 as f64) / gain))
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
    fn gains_scale_with_window_height() {
        assert_eq!(AIM.for_window(500.0).gain, 1.0);
        assert_eq!(AIM.for_window(2000.0).gain, 0.25);
    }

    #[test]
    fn tiny_gains_still_make_progress() {
        let slow = AimController { gain: 0.01, ..AIM };
        assert_eq!(slow.step((90.0, 103.0), (100.0, 100.0)), Some((-1, 0)));
        let off = AimController { gain: 0.0, ..AIM };
        assert_eq!(off.step((90.0, 100.0), (100.0, 100.0)), Some((0, 0)));
    }

    /// A simulated view: the target's error shrinks by what's been sent,
    /// seen through frames that arrive `latency` late, every `frame` s.
    fn simulate(gain: f64, true_gain: f64, latency: f64, frame: f64, target_speed: f64) -> (f64, f64, i32) {
        let aim = AimController { gain, max_step: 60.0, deadzone: 2.0 };
        let mut glide = Glide::new(aim, 7);
        let (dt, mut t, mut next_frame) = (0.002, 0.0, 0.0);
        let start = (300.0, -120.0);
        let mut worst_step = 0;
        let mut pending: Vec<(f64, f64, (f64, f64))> = Vec::new();
        while t < 1.5 {
            // What the screen shows now: the target, minus how far the view turned.
            let target = (start.0 + target_speed * t, start.1);
            let error = (target.0 - glide.sent.0 as f64 / true_gain, target.1 - glide.sent.1 as f64 / true_gain);
            if t >= next_frame {
                pending.push((t + latency, t, error));
                next_frame += frame;
            }
            while pending.first().is_some_and(|p| p.0 <= t) {
                let (_, captured, e) = pending.remove(0);
                glide.observe(captured, e);
            }
            let (dx, dy) = glide.tick(t, dt);
            worst_step = worst_step.max(dx.abs()).max(dy.abs());
            t += dt;
        }
        let target = (start.0 + target_speed * t, start.1);
        let error = (target.0 - glide.sent.0 as f64 / true_gain, target.1 - glide.sent.1 as f64 / true_gain);
        (error.0, error.1, worst_step)
    }

    #[test]
    fn glides_onto_a_still_target_despite_slow_late_frames() {
        // 20 detections a second, each 60 ms late.
        let (ex, ey, worst) = simulate(1.0, 1.0, 0.06, 0.05, 0.0);
        assert!(ex.hypot(ey) < 2.0, "ended {ex:.1},{ey:.1} px off");
        // Small moves every tick, never one big jump.
        assert!(worst <= 20, "jumped {worst} units in one tick");
    }

    #[test]
    fn an_underestimated_gain_still_converges() {
        let (ex, ey, _) = simulate(0.6, 1.0, 0.06, 0.05, 0.0);
        assert!(ex.hypot(ey) < 3.0, "ended {ex:.1},{ey:.1} px off");
    }

    #[test]
    fn leads_a_moving_target() {
        // 200 px/s sideways: without a lead it would trail by tens of pixels.
        let (ex, ey, _) = simulate(1.0, 1.0, 0.06, 0.05, 200.0);
        assert!(ex.hypot(ey) < 8.0, "trailing by {ex:.1},{ey:.1} px");
    }

    #[test]
    fn emitted_history_answers_past_times() {
        let mut glide = Glide::new(AIM, 1);
        glide.observe(0.0, (100.0, 0.0));
        for i in 0..50 {
            glide.tick(i as f64 * 0.002, 0.002);
        }
        let mid = glide.emitted_at(0.05);
        assert!(mid.0 > 0 && mid.0 < glide.sent.0, "{mid:?} vs {:?}", glide.sent);
        assert_eq!(glide.emitted_at(-1.0), glide.history.front().unwrap().1);
    }
}
