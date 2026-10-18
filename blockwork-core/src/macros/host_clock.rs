//! Lockstep virtual clock for hosts that embed the engine and own the
//! timeline (a game's physics step). `Wait`s park on virtual time instead of
//! the wall clock and input goes to a queue instead of the OS, so playback is
//! exact to the host's own ticks. The host calls [`step`] once per tick: it
//! advances time, lets the run settle on its next `Wait`, and hands back what
//! the run emitted.

use super::backend::InputBackend;
use crate::input::key_names::key_name;
use crate::input::types::{Axis, Direction, MacroButton, MacroKey};
use std::sync::{Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant};

/// Slack for deadlines built by summing many float `Wait`s.
const EPSILON_MS: f64 = 1e-6;
/// How long `step` waits for the run to settle before giving up on it.
const SETTLE_TIMEOUT: Duration = Duration::from_millis(250);

#[derive(Debug, Clone, PartialEq)]
pub struct HostInput {
    pub name: String,
    pub is_button: bool,
    pub down: bool,
}

#[derive(Default)]
struct State {
    enabled: bool,
    now_ms: f64,
    /// Set from `begin` until the run has registered its strands.
    starting: bool,
    alive: usize,
    /// Deadlines of strands currently parked in a `Wait`.
    waiting: Vec<f64>,
    events: Vec<HostInput>,
}

struct Clock {
    state: Mutex<State>,
    cv: Condvar,
}

fn clock() -> &'static Clock {
    static CLOCK: OnceLock<Clock> = OnceLock::new();
    CLOCK.get_or_init(|| Clock { state: Mutex::new(State::default()), cv: Condvar::new() })
}

/// Whether a lockstep run is active.
pub fn enabled() -> bool {
    clock().state.lock().map_or(false, |s| s.enabled)
}

/// Resets to virtual time 0 and arms lockstep mode for the next run.
pub fn begin() {
    let c = clock();
    if let Ok(mut s) = c.state.lock() {
        *s = State { enabled: true, starting: true, ..State::default() };
    }
    c.cv.notify_all();
}

/// Disarms lockstep mode once the run thread has returned.
pub fn finish() {
    let c = clock();
    if let Ok(mut s) = c.state.lock() {
        s.enabled = false;
        s.starting = false;
        s.alive = 0;
        s.waiting.clear();
    }
    c.cv.notify_all();
}

pub(crate) fn attach(strands: usize) {
    let c = clock();
    if let Ok(mut s) = c.state.lock() {
        s.alive += strands;
        s.starting = false;
    }
    c.cv.notify_all();
}

pub(crate) fn detach() {
    let c = clock();
    if let Ok(mut s) = c.state.lock() {
        s.alive = s.alive.saturating_sub(1);
    }
    c.cv.notify_all();
}

/// Parks until virtual time reaches `target_ms`, or `should_stop` says so.
/// Returns false if stopped.
pub(crate) fn wait_until(target_ms: f64, should_stop: &dyn Fn() -> bool) -> bool {
    let c = clock();
    let Ok(mut s) = c.state.lock() else { return false };
    if s.now_ms + EPSILON_MS >= target_ms {
        return !should_stop();
    }
    s.waiting.push(target_ms);
    c.cv.notify_all();
    let stopped = loop {
        if should_stop() {
            break true;
        }
        if s.now_ms + EPSILON_MS >= target_ms || !s.enabled {
            break false;
        }
        s = match c.cv.wait_timeout(s, Duration::from_millis(5)) {
            Ok((guard, _)) => guard,
            Err(_) => return false,
        };
    };
    if let Some(pos) = s.waiting.iter().position(|d| *d == target_ms) {
        s.waiting.remove(pos);
    }
    c.cv.notify_all();
    !stopped
}

/// Advances to `time_ms`, waits for the run to settle (every strand parked
/// on a future deadline, or finished), and drains what it emitted.
pub fn step(time_ms: f64) -> Vec<HostInput> {
    let c = clock();
    let Ok(mut s) = c.state.lock() else { return Vec::new() };
    if !s.enabled {
        return Vec::new();
    }
    s.now_ms = time_ms;
    c.cv.notify_all();
    let started = Instant::now();
    loop {
        let settled = !s.starting
            && s.waiting.len() >= s.alive
            && s.waiting.iter().all(|d| *d > time_ms + EPSILON_MS);
        if settled {
            break;
        }
        if started.elapsed() > SETTLE_TIMEOUT {
            tracing::warn!("host clock: run did not settle at {time_ms:.3}ms");
            break;
        }
        s = match c.cv.wait_timeout(s, Duration::from_millis(1)) {
            Ok((guard, _)) => guard,
            Err(_) => return Vec::new(),
        };
    }
    std::mem::take(&mut s.events)
}

fn push(name: String, is_button: bool, dir: Direction) {
    let c = clock();
    let Ok(mut s) = c.state.lock() else { return };
    let mut add = |down| s.events.push(HostInput { name: name.clone(), is_button, down });
    match dir {
        Direction::Press => add(true),
        Direction::Release => add(false),
        Direction::Click => {
            add(true);
            add(false);
        }
    }
}

/// Input backend that queues for [`step`] to hand to the host.
pub struct HostBackend;

impl InputBackend for HostBackend {
    fn key(&mut self, key: MacroKey, dir: Direction) -> Result<(), String> {
        push(key_name(&key), false, dir);
        Ok(())
    }
    fn raw_keycode(&mut self, keycode: u16, dir: Direction) -> Result<(), String> {
        push(format!("Raw{keycode}"), false, dir);
        Ok(())
    }
    fn button(&mut self, button: MacroButton, dir: Direction) -> Result<(), String> {
        push(format!("{button:?}"), true, dir);
        Ok(())
    }
    fn move_mouse_rel(&mut self, _dx: i32, _dy: i32) -> Result<(), String> {
        Ok(())
    }
    fn move_mouse_abs(&mut self, _x: i32, _y: i32) -> Result<(), String> {
        Ok(())
    }
    fn scroll(&mut self, _amount: i32, _axis: Axis) -> Result<(), String> {
        Ok(())
    }
    fn text(&mut self, _s: &str) -> Result<(), String> {
        Ok(())
    }
    fn cursor_pos(&self) -> Option<(i32, i32)> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn waits_follow_virtual_time_and_events_land_on_their_step() {
        begin();
        let run = std::thread::spawn(|| {
            attach(1);
            let stop = || false;
            let mut backend = HostBackend;
            let mut t = 0.0;
            for _ in 0..3 {
                t += 100.0 / 3.0;
                assert!(wait_until(t, &stop));
                backend.button(MacroButton::Left, Direction::Click).unwrap();
            }
            detach();
        });
        let mut counts = Vec::new();
        for n in 0..=40 {
            counts.push(step(n as f64 * 2.5).len());
        }
        run.join().unwrap();
        finish();
        // Clicks at 33.3, 66.6 and 100ms land on the first step at or past each.
        let hits: Vec<usize> = counts.iter().enumerate().filter(|(_, c)| **c > 0).map(|(i, _)| i).collect();
        assert_eq!(hits, vec![14, 27, 40]);
    }
}
