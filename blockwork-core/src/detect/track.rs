//! Keeps detections stable across frames: each box gets an id that follows
//! the same object, with its position smoothed so aiming doesn't jitter.

use super::{Detection, Query};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// A track survives this many frames without a match, and at least
/// `LOST_AFTER`, so a fast detector doesn't drop a target on a brief miss.
pub const MAX_MISSES: u32 = 5;
const LOST_AFTER: Duration = Duration::from_millis(250);
/// Weight of the newest box in the smoothed one.
const SMOOTHING: f64 = 0.6;
/// A tracker idle this long starts over: its objects have moved on. Well
/// above one frame, since a portal capture alone can take seconds.
const STALE_AFTER: Duration = Duration::from_secs(10);

#[derive(Debug, Clone)]
struct Track {
    box_: Detection,
    misses: u32,
    seen: Instant,
}

#[derive(Debug, Default)]
pub struct Tracker {
    tracks: Vec<Track>,
    next_id: u64,
}

impl Tracker {
    /// Matches this frame's boxes to existing tracks and returns them with
    /// ids and smoothed positions. Unmatched boxes start new tracks.
    pub fn update(&mut self, found: Vec<Detection>) -> Vec<Detection> {
        self.update_at(found, Instant::now())
    }

    fn update_at(&mut self, found: Vec<Detection>, now: Instant) -> Vec<Detection> {
        let mut pairs: Vec<(f64, usize, usize)> = Vec::new();
        for (t, track) in self.tracks.iter().enumerate() {
            for (d, det) in found.iter().enumerate() {
                let score = similarity(&track.box_, det);
                if score > 0.0 {
                    pairs.push((score, t, d));
                }
            }
        }
        pairs.sort_by(|a, b| b.0.total_cmp(&a.0));
        let mut track_used = vec![false; self.tracks.len()];
        let mut det_track: Vec<Option<usize>> = vec![None; found.len()];
        for (_, t, d) in pairs {
            if !track_used[t] && det_track[d].is_none() {
                track_used[t] = true;
                det_track[d] = Some(t);
            }
        }
        for (track, used) in self.tracks.iter_mut().zip(&track_used) {
            if !used {
                track.misses += 1;
            }
        }
        let mut out = Vec::with_capacity(found.len());
        for (det, matched) in found.into_iter().zip(det_track) {
            let tracked = match matched {
                Some(t) => {
                    let track = &mut self.tracks[t];
                    track.box_ = smooth(&track.box_, det);
                    track.misses = 0;
                    track.seen = now;
                    track.box_.clone()
                }
                None => {
                    self.next_id += 1;
                    let det = Detection { id: Some(self.next_id), ..det };
                    self.tracks.push(Track { box_: det.clone(), misses: 0, seen: now });
                    det
                }
            };
            out.push(tracked);
        }
        self.tracks.retain(|t| t.misses <= MAX_MISSES || now.duration_since(t.seen) < LOST_AFTER);
        out
    }

    /// Whether track `id` is still alive (seen within the last few frames,
    /// or the last `LOST_AFTER`).
    pub fn alive(&self, id: u64) -> bool {
        self.tracks.iter().any(|t| t.box_.id == Some(id))
    }
}

/// How alike two boxes are, 0 meaning "not the same object". Overlap
/// counts most; centers within two box sizes still match a fast-moving box.
fn similarity(a: &Detection, b: &Detection) -> f64 {
    if !a.label.eq_ignore_ascii_case(&b.label) {
        return 0.0;
    }
    let iou = iou(a, b);
    if iou > 0.0 {
        return 1.0 + iou;
    }
    let ((ax, ay), (bx, by)) = (a.center(), b.center());
    let reach = 2.0 * a.w.max(a.h).max(b.w.max(b.h));
    let distance = (ax - bx).hypot(ay - by);
    if reach > 0.0 && distance < reach { 1.0 - distance / reach } else { 0.0 }
}

fn iou(a: &Detection, b: &Detection) -> f64 {
    let w = (a.x + a.w).min(b.x + b.w) - a.x.max(b.x);
    let h = (a.y + a.h).min(b.y + b.h) - a.y.max(b.y);
    if w <= 0.0 || h <= 0.0 {
        return 0.0;
    }
    let overlap = w * h;
    overlap / (a.w * a.h + b.w * b.h - overlap)
}

fn smooth(old: &Detection, new: Detection) -> Detection {
    let mix = |o: f64, n: f64| o + (n - o) * SMOOTHING;
    Detection {
        x: mix(old.x, new.x),
        y: mix(old.y, new.y),
        w: mix(old.w, new.w),
        h: mix(old.h, new.h),
        id: old.id,
        ..new
    }
}

/// A tracker plus the query that feeds it, per macro and results list.
struct Entry {
    query: Query,
    tracker: Tracker,
    used: Instant,
}

static TRACKERS: Mutex<Option<HashMap<(String, String), Entry>>> = Mutex::new(None);

/// Runs `found` through the tracker for (`macro_id`, `list`), starting over
/// when the query changed or the tracker sat idle.
pub fn update(macro_id: &str, list: &str, query: &Query, found: Vec<Detection>) -> Vec<Detection> {
    let mut all = TRACKERS.lock().unwrap_or_else(|e| e.into_inner());
    let all = all.get_or_insert_with(HashMap::new);
    let entry = all.entry((macro_id.to_string(), list.to_string())).or_insert_with(|| Entry {
        query: query.clone(),
        tracker: Tracker::default(),
        used: Instant::now(),
    });
    if entry.query != *query || entry.used.elapsed() > STALE_AFTER {
        entry.query = query.clone();
        entry.tracker.tracks.clear();
    }
    entry.used = Instant::now();
    entry.tracker.update(found)
}

/// The query that last filled (`macro_id`, `list`), for repeating it.
pub fn query_for(macro_id: &str, list: &str) -> Option<Query> {
    let all = TRACKERS.lock().unwrap_or_else(|e| e.into_inner());
    all.as_ref()?.get(&(macro_id.to_string(), list.to_string())).map(|e| e.query.clone())
}

/// Whether track `id` in (`macro_id`, `list`) is still alive.
pub fn alive(macro_id: &str, list: &str, id: u64) -> bool {
    let all = TRACKERS.lock().unwrap_or_else(|e| e.into_inner());
    all.as_ref()
        .and_then(|all| all.get(&(macro_id.to_string(), list.to_string())))
        .is_some_and(|e| e.tracker.alive(id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::detect::tests::boxed;

    #[test]
    fn ids_follow_objects_across_frames() {
        let mut t = Tracker::default();
        let first = t.update(vec![boxed(0.0, 0.0, 10.0, 20.0, "p", 0.9), boxed(100.0, 0.0, 10.0, 20.0, "p", 0.8)]);
        assert_eq!(first.iter().map(|d| d.id).collect::<Vec<_>>(), vec![Some(1), Some(2)]);
        // Both moved a little and came back in the other order.
        let second = t.update(vec![boxed(104.0, 2.0, 10.0, 20.0, "p", 0.8), boxed(3.0, 1.0, 10.0, 20.0, "p", 0.9)]);
        assert_eq!(second.iter().map(|d| d.id).collect::<Vec<_>>(), vec![Some(2), Some(1)]);
        // Smoothed part of the way toward the new position.
        assert!((second[0].x - 102.4).abs() < 1e-9, "{}", second[0].x);
        // A different label never continues a track.
        let third = t.update(vec![boxed(3.0, 1.0, 10.0, 20.0, "nametag", 0.9)]);
        assert_eq!(third[0].id, Some(3));
    }

    #[test]
    fn small_fast_boxes_match_by_distance() {
        let mut t = Tracker::default();
        t.update(vec![boxed(0.0, 0.0, 10.0, 10.0, "p", 0.9)]);
        // No overlap, but within two box sizes.
        assert_eq!(t.update(vec![boxed(12.0, 0.0, 10.0, 10.0, "p", 0.9)])[0].id, Some(1));
        // Too far: a new object.
        assert_eq!(t.update(vec![boxed(80.0, 0.0, 10.0, 10.0, "p", 0.9)])[0].id, Some(2));
    }

    #[test]
    fn lost_tracks_drop_after_a_few_slow_misses() {
        let mut t = Tracker::default();
        let start = Instant::now();
        let frame = |n: u32| start + Duration::from_millis(100) * n;
        t.update_at(vec![boxed(0.0, 0.0, 10.0, 10.0, "p", 0.9)], frame(0));
        for n in 1..=MAX_MISSES {
            t.update_at(vec![], frame(n));
        }
        assert!(t.alive(1));
        // Reappearing within the limit keeps its id.
        assert_eq!(t.update_at(vec![boxed(1.0, 0.0, 10.0, 10.0, "p", 0.9)], frame(6))[0].id, Some(1));
        for n in 7..=7 + MAX_MISSES {
            t.update_at(vec![], frame(n));
        }
        assert!(!t.alive(1));
    }

    #[test]
    fn fast_detectors_keep_tracks_through_brief_misses() {
        // At 140 frames/s, ten missed frames is only about 70 ms.
        let mut t = Tracker::default();
        let start = Instant::now();
        let frame = |n: u32| start + Duration::from_millis(7) * n;
        t.update_at(vec![boxed(0.0, 0.0, 10.0, 10.0, "p", 0.9)], frame(0));
        for n in 1..=10 {
            t.update_at(vec![], frame(n));
        }
        assert_eq!(t.update_at(vec![boxed(1.0, 0.0, 10.0, 10.0, "p", 0.9)], frame(11))[0].id, Some(1));
        for n in 12..=60 {
            t.update_at(vec![], frame(n));
        }
        assert!(!t.alive(1));
    }
}
