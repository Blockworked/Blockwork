//! Portable complete-contact frames, shared with future Android backends.
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub const MAX_CONTACTS: usize = 10;
pub const MAX_FRAMES: usize = 40_000;
pub const MAX_DURATION_MS: u32 = 600_000;
pub const NORMALIZED_MAX: i32 = 1_000_000;
pub const MAX_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TouchGestureKind {
    Tap,
    LongPress,
    Swipe,
    Pinch,
    Rotate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TouchContact {
    pub id: u8,
    pub x: i32,
    pub y: i32,
}
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TouchFrame {
    pub at_ms: u32,
    pub contacts: Vec<TouchContact>,
}
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TouchSequence {
    pub version: u32,
    pub frames: Vec<TouchFrame>,
}

impl TouchSequence {
    pub fn parse(text: &str) -> Result<Self, String> {
        if text.len() > MAX_BYTES {
            return Err("Touch paths exceed the 2 MiB limit".into());
        }
        let sequence: Self =
            serde_json::from_str(text).map_err(|e| format!("Invalid touch paths: {e}"))?;
        sequence.validate()?;
        Ok(sequence)
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.version != 1 {
            return Err("Unsupported touch path version".into());
        }
        if self.frames.len() < 2 || self.frames.len() > MAX_FRAMES {
            return Err("Touch paths need 2 to 40,000 frames".into());
        }
        if self.frames[0].at_ms != 0
            || self.frames[0].contacts.is_empty()
            || !self.frames.last().unwrap().contacts.is_empty()
        {
            return Err(
                "Touch paths must start with contacts at time 0 and finish with no contacts".into(),
            );
        }
        let mut previous = HashSet::new();
        let mut time = None;
        for frame in &self.frames {
            if frame.at_ms > MAX_DURATION_MS || time.is_some_and(|t| frame.at_ms <= t) {
                return Err("Touch frame times must increase, within ten minutes".into());
            }
            time = Some(frame.at_ms);
            let ids: HashSet<_> = frame.contacts.iter().map(|p| p.id).collect();
            if ids.len() != frame.contacts.len()
                || ids.iter().any(|id| *id as usize >= MAX_CONTACTS)
                || ids.union(&previous).count() > MAX_CONTACTS
            {
                return Err("Touch frames need unique contact IDs from 0 to 9".into());
            }
            previous = ids;
        }
        if serde_json::to_vec(self).map_err(|e| e.to_string())?.len() > MAX_BYTES {
            return Err("Touch paths exceed the 2 MiB limit".into());
        }
        Ok(())
    }
}

pub struct Gesture {
    pub kind: TouchGestureKind,
    pub fingers: u8,
    pub x: f64,
    pub y: f64,
    pub end_x: f64,
    pub end_y: f64,
    pub duration_ms: f64,
    pub span: f64,
    pub amount: f64,
}
impl Gesture {
    pub fn sequence(&self) -> Result<TouchSequence, String> {
        if ![
            self.x,
            self.y,
            self.end_x,
            self.end_y,
            self.duration_ms,
            self.span,
            self.amount,
        ]
        .iter()
        .all(|n| n.is_finite())
        {
            return Err("Gesture values must be finite numbers".into());
        }
        if self.duration_ms < 1.0 || self.duration_ms > MAX_DURATION_MS as f64 || self.span < 0.0 {
            return Err(
                "Gesture duration must be 1 to 600,000 ms and spread must be non-negative".into(),
            );
        }
        let fingers = if matches!(
            self.kind,
            TouchGestureKind::Pinch | TouchGestureKind::Rotate
        ) {
            2
        } else {
            self.fingers
        };
        if !(1..=10).contains(&fingers) {
            return Err("Gestures support 1 to 10 fingers".into());
        }
        if matches!(
            self.kind,
            TouchGestureKind::Pinch | TouchGestureKind::Rotate
        ) && self.span <= 0.0
        {
            return Err("Pinch and rotation gestures need a positive spread".into());
        }
        if self.kind == TouchGestureKind::Pinch && !(0.01..=20.0).contains(&self.amount) {
            return Err("Pinch scale must be 0.01 to 20".into());
        }
        if self.kind == TouchGestureKind::Rotate && self.amount.abs() > 3600.0 {
            return Err("Rotation must be within 3,600 degrees".into());
        }
        let moving = matches!(
            self.kind,
            TouchGestureKind::Swipe | TouchGestureKind::Pinch | TouchGestureKind::Rotate
        );
        let duration = self.duration_ms.round() as u32;
        let steps = if moving {
            duration.div_ceil(16).max(1)
        } else {
            1
        };
        let point = |n: f64| -> Result<i32, String> {
            if !n.is_finite() || n < i32::MIN as f64 || n > i32::MAX as f64 {
                Err("Gesture point is out of range".into())
            } else {
                Ok(n.round() as i32)
            }
        };
        let mut frames = Vec::with_capacity(steps as usize + 2);
        for step in 0..=if moving { steps } else { 0 } {
            let t = step as f64 / steps as f64;
            let mut contacts = Vec::with_capacity(fingers as usize);
            for id in 0..fingers {
                let offset = (id as f64 - (fingers as f64 - 1.0) / 2.0) * self.span;
                let (x, y) = match self.kind {
                    TouchGestureKind::Swipe => (
                        self.x + (self.end_x - self.x) * t + offset,
                        self.y + (self.end_y - self.y) * t,
                    ),
                    TouchGestureKind::Pinch => {
                        (self.x + offset * (1.0 + (self.amount - 1.0) * t), self.y)
                    }
                    TouchGestureKind::Rotate => {
                        let angle = (self.amount * t).to_radians();
                        (self.x + offset * angle.cos(), self.y + offset * angle.sin())
                    }
                    _ => (self.x + offset, self.y),
                };
                contacts.push(TouchContact {
                    id,
                    x: point(x)?,
                    y: point(y)?,
                });
            }
            // Reserve one millisecond at the end for the release frame.
            frames.push(TouchFrame {
                at_ms: if moving {
                    ((duration - 1) as u64 * step as u64 / steps as u64) as u32
                } else {
                    0
                },
                contacts,
            });
        }
        frames.dedup_by_key(|frame| frame.at_ms);
        frames.push(TouchFrame {
            at_ms: duration,
            contacts: Vec::new(),
        });
        let sequence = TouchSequence { version: 1, frames };
        sequence.validate()?;
        Ok(sequence)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn gesture(kind: TouchGestureKind) -> Gesture {
        Gesture {
            kind,
            fingers: 3,
            x: 400.0,
            y: 400.0,
            end_x: 500.0,
            end_y: 600.0,
            duration_ms: 200.0,
            span: 40.0,
            amount: 2.0,
        }
    }
    #[test]
    fn gestures_keep_contacts_and_release_every_finger() {
        for kind in [
            TouchGestureKind::Tap,
            TouchGestureKind::LongPress,
            TouchGestureKind::Swipe,
            TouchGestureKind::Pinch,
            TouchGestureKind::Rotate,
        ] {
            let sequence = gesture(kind).sequence().unwrap();
            assert!(sequence.frames.last().unwrap().contacts.is_empty());
            assert_eq!(
                TouchSequence::parse(&serde_json::to_string(&sequence).unwrap()).unwrap(),
                sequence
            );
            assert!(sequence.frames.iter().all(|f| f.contacts.len() <= 3));
        }
        let swipe = gesture(TouchGestureKind::Swipe).sequence().unwrap();
        assert_eq!(swipe.frames[0].contacts[1].x, 400);
        assert_eq!(swipe.frames[swipe.frames.len() - 2].contacts[1].x, 500);
        let pinch = gesture(TouchGestureKind::Pinch).sequence().unwrap();
        assert_eq!(pinch.frames.last().unwrap().at_ms, 200);
        assert_eq!(pinch.frames[0].contacts.len(), 2);
    }
    #[test]
    fn long_holds_are_small_and_invalid_paths_never_start() {
        let mut hold = gesture(TouchGestureKind::LongPress);
        hold.duration_ms = 600000.0;
        assert_eq!(hold.sequence().unwrap().frames.len(), 2);
        hold.x = f64::NAN;
        assert!(hold.sequence().is_err());
        let mut seq = gesture(TouchGestureKind::Swipe).sequence().unwrap();
        seq.frames[0].contacts[1].id = 0;
        assert!(seq.validate().is_err());
        let mut seq = gesture(TouchGestureKind::Tap).sequence().unwrap();
        seq.frames.pop();
        assert!(seq.validate().is_err());
    }
}
