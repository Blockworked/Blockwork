//! Touchscreen recording: contacts are read without a grab, so the desktop
//! keeps receiving them, and each gesture becomes one touch paths block.
use super::{close_span, open_span};
use crate::input::touch::{MAX_CONTACTS, NORMALIZED_MAX, TouchContact, TouchFrame, TouchSequence};
use crate::input::value::Value;
use crate::macros::{Instruction, InstructionKind};
use std::sync::Mutex;
use std::time::Duration;

/// Frames closer than this replace the previous one while the fingers stay
/// the same, like recorded mouse movement.
const FRAME_INTERVAL_MS: u32 = 16;
/// Keeps one recorded gesture well under the 2 MiB touch path limit.
pub const RECORD_FRAME_LIMIT: usize = 5_000;
/// A recorded gesture is cut at one minute; playback allows ten.
pub const RECORD_DURATION_MS: u32 = 60_000;

/// One axis's reported range, from the device's absinfo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AxisRange {
    pub min: i32,
    pub max: i32,
}

impl AxisRange {
    fn normalize(self, value: i32) -> i32 {
        let span = i64::from(self.max) - i64::from(self.min);
        if span <= 0 {
            return 0;
        }
        let offset = (i64::from(value) - i64::from(self.min)).clamp(0, span);
        (offset * i64::from(NORMALIZED_MAX) / span) as i32
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct Slot {
    tracking: bool,
    x: i32,
    y: i32,
}

/// Kernel multitouch protocol B state for one device. Contact IDs are slot
/// numbers, which the protocol keeps stable for a finger's whole contact.
#[derive(Debug, Clone)]
pub struct ContactTracker {
    x: AxisRange,
    y: AxisRange,
    slot: usize,
    slots: [Slot; MAX_CONTACTS],
}

/// The multitouch axis codes the tracker reads (linux/input-event-codes.h).
pub mod codes {
    pub const ABS_MT_SLOT: u16 = 0x2f;
    pub const ABS_MT_POSITION_X: u16 = 0x35;
    pub const ABS_MT_POSITION_Y: u16 = 0x36;
    pub const ABS_MT_TRACKING_ID: u16 = 0x39;
}

impl ContactTracker {
    pub fn new(x: AxisRange, y: AxisRange) -> Self {
        Self {
            x,
            y,
            slot: 0,
            slots: [Slot::default(); MAX_CONTACTS],
        }
    }

    /// Applies one absolute-axis event. Slots past the tenth are ignored.
    pub fn abs(&mut self, code: u16, value: i32) {
        if code == codes::ABS_MT_SLOT {
            self.slot = usize::try_from(value).unwrap_or(usize::MAX);
            return;
        }
        let Some(slot) = self.slots.get_mut(self.slot) else {
            return;
        };
        match code {
            codes::ABS_MT_TRACKING_ID => slot.tracking = value >= 0,
            codes::ABS_MT_POSITION_X => slot.x = value,
            codes::ABS_MT_POSITION_Y => slot.y = value,
            _ => {}
        }
    }

    /// The complete contact frame at a SYN_REPORT, normalized to the device.
    pub fn contacts(&self) -> Vec<TouchContact> {
        self.slots
            .iter()
            .enumerate()
            .filter(|(_, slot)| slot.tracking)
            .map(|(id, slot)| TouchContact {
                id: id as u8,
                x: self.x.normalize(slot.x),
                y: self.y.normalize(slot.y),
            })
            .collect()
    }

    /// Forgets every contact, after the kernel dropped events (SYN_DROPPED).
    pub fn reset(&mut self) {
        self.slots = [Slot::default(); MAX_CONTACTS];
    }
}

/// What one frame did to the gesture being recorded.
#[derive(Debug, PartialEq)]
pub enum Step {
    Idle,
    Started,
    Continued,
    /// The fingers lifted, or the gesture hit a recording limit; `truncated`
    /// says which. The sequence always ends with every contact released.
    Finished {
        sequence: TouchSequence,
        start: Duration,
        truncated: bool,
    },
}

/// Turns frames into one touch path per gesture, from the first finger down
/// to the last finger up.
#[derive(Debug, Default)]
pub struct GestureRecorder {
    start: Option<Duration>,
    frames: Vec<TouchFrame>,
}

impl GestureRecorder {
    pub fn is_open(&self) -> bool {
        self.start.is_some()
    }

    pub fn frame(&mut self, at: Duration, contacts: Vec<TouchContact>) -> Step {
        let Some(start) = self.start else {
            if contacts.is_empty() {
                return Step::Idle;
            }
            self.start = Some(at);
            self.frames = vec![TouchFrame { at_ms: 0, contacts }];
            return Step::Started;
        };
        let last = self.frames.last().map_or(0, |f| f.at_ms);
        let at_ms = u32::try_from(at.saturating_sub(start).as_millis()).unwrap_or(u32::MAX);
        if contacts.is_empty() {
            return self.close(at_ms.max(last + 1), false);
        }
        if at_ms >= RECORD_DURATION_MS || self.frames.len() + 1 >= RECORD_FRAME_LIMIT {
            return self.close(last + 1, true);
        }
        let same_fingers = self.frames.last().is_some_and(|f| {
            f.contacts.len() == contacts.len()
                && f.contacts.iter().zip(&contacts).all(|(a, b)| a.id == b.id)
        });
        if self.frames.len() > 1 && same_fingers && at_ms < last + FRAME_INTERVAL_MS {
            self.frames.last_mut().unwrap().contacts = contacts;
        } else if self.frames.last().is_some_and(|f| f.contacts == contacts) {
            // Nothing moved; keep the frame count for real changes.
        } else {
            self.frames.push(TouchFrame {
                at_ms: at_ms.max(last + 1),
                contacts,
            });
        }
        Step::Continued
    }

    /// Ends the gesture now, releasing every finger, e.g. when recording stops.
    pub fn finish(&mut self, at: Duration) -> Step {
        let Some(start) = self.start else {
            return Step::Idle;
        };
        let last = self.frames.last().map_or(0, |f| f.at_ms);
        let at_ms = u32::try_from(at.saturating_sub(start).as_millis()).unwrap_or(u32::MAX);
        self.close(
            at_ms.clamp(last + 1, RECORD_DURATION_MS.max(last + 1)),
            true,
        )
    }

    fn close(&mut self, at_ms: u32, truncated: bool) -> Step {
        let start = self.start.take().unwrap_or_default();
        let mut frames = std::mem::take(&mut self.frames);
        frames.push(TouchFrame {
            at_ms,
            contacts: Vec::new(),
        });
        Step::Finished {
            sequence: TouchSequence { version: 1, frames },
            start,
            truncated,
        }
    }
}

/// The open gesture's placeholder in the recording queue.
struct Open {
    id: String,
    start: Duration,
}

/// Recorder state shared by the reader thread and `finish_open`.
struct Recording {
    gesture: GestureRecorder,
    open: Option<Open>,
}

static RECORDING: Mutex<Option<Recording>> = Mutex::new(None);

fn placeholder() -> Instruction {
    Instruction::new(InstructionKind::TouchSequence {
        sequence: Box::new(Value::text("")),
        normalized: true,
        monitor: 0,
    })
}

/// Feeds one frame into the shared recorder, queueing the gesture's block.
/// Public so tests and other providers drive the same bookkeeping.
pub fn record_frame(at: Duration, contacts: Vec<TouchContact>) {
    let mut guard = RECORDING.lock().unwrap_or_else(|p| p.into_inner());
    let recording = guard.get_or_insert_with(|| Recording {
        gesture: GestureRecorder::default(),
        open: None,
    });
    let step = recording.gesture.frame(at, contacts);
    apply(recording, step, at);
}

fn apply(recording: &mut Recording, step: Step, at: Duration) {
    match step {
        Step::Started => {
            recording.open = open_span(placeholder(), at).map(|id| Open { id, start: at });
        }
        Step::Finished {
            sequence,
            start,
            truncated,
        } => {
            let Some(open) = recording.open.take() else {
                return;
            };
            let length =
                Duration::from_millis(u64::from(sequence.frames.last().map_or(0, |f| f.at_ms)));
            let kind = match sequence.validate() {
                Ok(()) => serde_json::to_string(&sequence).ok().map(|json| {
                    InstructionKind::TouchSequence {
                        sequence: Box::new(Value::text(json)),
                        normalized: true,
                        monitor: 0,
                    }
                }),
                Err(error) => {
                    crate::run_log::error(
                        &crate::run_log::Source::app("Touch recording"),
                        format!("A recorded touch gesture was dropped: {error}"),
                    );
                    None
                }
            };
            if truncated && kind.is_some() {
                crate::run_log::info(
                    &crate::run_log::Source::app("Touch recording"),
                    "A long touch gesture was cut at the recording limit",
                );
            }
            close_span(&open.id, kind, start.min(open.start), length);
        }
        Step::Idle | Step::Continued => {}
    }
}

/// Closes a gesture still in progress, as if every finger lifted now.
pub(super) fn finish_open() {
    let mut guard = RECORDING.lock().unwrap_or_else(|p| p.into_inner());
    if let Some(mut recording) = guard.take() {
        let at = recording
            .open
            .as_ref()
            .map_or(Duration::ZERO, |open| open.start)
            .max(super::elapsed_at(std::time::Instant::now()));
        let step = recording.gesture.finish(at);
        apply(&mut recording, step, at);
    }
}

/// Touchscreens that recording can read on this platform, by name.
pub fn devices() -> Vec<String> {
    #[cfg(target_os = "linux")]
    {
        linux::touchscreens()
            .into_iter()
            .map(|(name, _)| name)
            .collect()
    }
    #[cfg(not(target_os = "linux"))]
    {
        Vec::new()
    }
}

pub(super) fn start() {
    #[cfg(target_os = "linux")]
    linux::start();
    #[cfg(not(target_os = "linux"))]
    crate::run_log::info(
        &crate::run_log::Source::app("Touch recording"),
        "Touch recording is only available on Linux; other input is still recorded",
    );
}

#[cfg(target_os = "linux")]
mod linux {
    use super::{AxisRange, ContactTracker};
    use evdev::{AbsoluteAxisCode as Abs, EventSummary, PropType, SynchronizationCode};
    use std::sync::atomic::Ordering;
    use std::time::Duration;

    /// Direct-touch devices with multitouch protocol B, excluding our own
    /// virtual touchscreen, which would otherwise record its own playback.
    pub(super) fn touchscreens() -> Vec<(String, evdev::Device)> {
        evdev::enumerate()
            .filter_map(|(_, device)| {
                let name = device.name().unwrap_or("").to_owned();
                let axes = device.supported_absolute_axes()?;
                (!name.starts_with("Blockwork Virtual ")
                    && device.properties().contains(PropType::DIRECT)
                    && axes.contains(Abs::ABS_MT_SLOT)
                    && axes.contains(Abs::ABS_MT_POSITION_X)
                    && axes.contains(Abs::ABS_MT_POSITION_Y))
                .then_some((name, device))
            })
            .collect()
    }

    fn ranges(device: &evdev::Device) -> Option<(AxisRange, AxisRange)> {
        let mut x = None;
        let mut y = None;
        for (axis, info) in device.get_absinfo().ok()? {
            let range = AxisRange {
                min: info.minimum(),
                max: info.maximum(),
            };
            match axis {
                Abs::ABS_MT_POSITION_X => x = Some(range),
                Abs::ABS_MT_POSITION_Y => y = Some(range),
                _ => {}
            }
        }
        Some((x?, y?))
    }

    pub(super) fn start() {
        let devices = touchscreens();
        if devices.is_empty() {
            crate::run_log::info(
                &crate::run_log::Source::app("Touch recording"),
                "No touchscreen was found to record",
            );
            return;
        }
        // One recorder serves every screen; two touched at once would mix
        // their contacts, so only the first device in a gesture counts.
        for (name, device) in devices {
            let Some((x, y)) = ranges(&device) else {
                continue;
            };
            if let Err(error) = device.set_nonblocking(true) {
                tracing::warn!("Touch recording can't read {name}: {error}");
                continue;
            }
            let _ = std::thread::Builder::new()
                .name("touch-recording".into())
                .spawn(move || read(device, ContactTracker::new(x, y)));
        }
    }

    fn read(mut device: evdev::Device, mut tracker: ContactTracker) {
        while crate::recording::RECORDING_ACTIVE.load(Ordering::Relaxed) {
            let events = match device.fetch_events() {
                Ok(events) => events.collect::<Vec<_>>(),
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(4));
                    continue;
                }
                Err(error) => {
                    tracing::warn!("Touch recording stopped reading a device: {error}");
                    break;
                }
            };
            for event in events {
                match event.destructure() {
                    EventSummary::AbsoluteAxis(_, axis, value) => tracker.abs(axis.0, value),
                    EventSummary::Synchronization(_, SynchronizationCode::SYN_DROPPED, _) => {
                        tracker.reset()
                    }
                    EventSummary::Synchronization(_, SynchronizationCode::SYN_REPORT, _) => {
                        let at = super::super::elapsed_since_session_start(
                            crate::macros::backend::CaptureTimestamp::Hardware(event.timestamp()),
                        );
                        super::record_frame(at, tracker.contacts());
                    }
                    _ => {}
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }
    fn contact(id: u8, x: i32) -> TouchContact {
        TouchContact { id, x, y: 0 }
    }

    #[test]
    fn tracker_follows_protocol_b_slots_and_normalizes() {
        let range = AxisRange { min: 0, max: 1000 };
        let mut tracker = ContactTracker::new(range, range);
        tracker.abs(codes::ABS_MT_SLOT, 0);
        tracker.abs(codes::ABS_MT_TRACKING_ID, 41);
        tracker.abs(codes::ABS_MT_POSITION_X, 500);
        tracker.abs(codes::ABS_MT_POSITION_Y, 1000);
        tracker.abs(codes::ABS_MT_SLOT, 3);
        tracker.abs(codes::ABS_MT_TRACKING_ID, 42);
        tracker.abs(codes::ABS_MT_POSITION_X, 2000);
        let contacts = tracker.contacts();
        assert_eq!(contacts.len(), 2);
        assert_eq!(
            contacts[0],
            TouchContact {
                id: 0,
                x: 500_000,
                y: 1_000_000
            }
        );
        // Out-of-range positions clamp to the device's edge.
        assert_eq!(contacts[1].x, 1_000_000);
        tracker.abs(codes::ABS_MT_SLOT, 0);
        tracker.abs(codes::ABS_MT_TRACKING_ID, -1);
        assert_eq!(tracker.contacts().len(), 1);
        // An eleventh slot never becomes a contact.
        tracker.abs(codes::ABS_MT_SLOT, 12);
        tracker.abs(codes::ABS_MT_TRACKING_ID, 7);
        assert_eq!(tracker.contacts().len(), 1);
        tracker.reset();
        assert!(tracker.contacts().is_empty());
        let flat = AxisRange { min: 5, max: 5 };
        assert_eq!(flat.normalize(9), 0);
    }

    #[test]
    fn gestures_record_from_first_finger_to_last_and_validate() {
        let mut gesture = GestureRecorder::default();
        assert_eq!(gesture.frame(ms(5), vec![]), Step::Idle);
        assert_eq!(gesture.frame(ms(100), vec![contact(0, 10)]), Step::Started);
        // Frames inside the interval with the same fingers coalesce.
        assert_eq!(
            gesture.frame(ms(104), vec![contact(0, 20)]),
            Step::Continued
        );
        assert_eq!(
            gesture.frame(ms(108), vec![contact(0, 30)]),
            Step::Continued
        );
        // A second finger always gets its own frame.
        gesture.frame(ms(110), vec![contact(0, 30), contact(1, 50)]);
        gesture.frame(ms(140), vec![contact(1, 60)]);
        let Step::Finished {
            sequence,
            start,
            truncated,
        } = gesture.frame(ms(140), vec![])
        else {
            panic!("the gesture should finish");
        };
        assert_eq!(start, ms(100));
        assert!(!truncated);
        sequence.validate().unwrap();
        let times: Vec<_> = sequence.frames.iter().map(|f| f.at_ms).collect();
        assert_eq!(times, vec![0, 4, 10, 40, 41]);
        assert_eq!(sequence.frames[1].contacts[0].x, 30);
        assert!(!gesture.is_open());
    }

    #[test]
    fn a_quick_tap_and_limits_still_make_valid_paths() {
        let mut gesture = GestureRecorder::default();
        gesture.frame(ms(0), vec![contact(0, 1)]);
        let Step::Finished { sequence, .. } = gesture.frame(ms(0), vec![]) else {
            panic!();
        };
        sequence.validate().unwrap();
        assert_eq!(sequence.frames.len(), 2);

        let mut gesture = GestureRecorder::default();
        gesture.frame(ms(0), vec![contact(0, 1)]);
        gesture.frame(ms(30), vec![contact(0, 2)]);
        let Step::Finished {
            sequence,
            truncated,
            ..
        } = gesture.frame(ms(u64::from(RECORD_DURATION_MS) + 5), vec![contact(0, 3)])
        else {
            panic!("an over-long gesture is cut");
        };
        assert!(truncated);
        sequence.validate().unwrap();

        let mut gesture = GestureRecorder::default();
        gesture.frame(ms(0), vec![contact(0, 0)]);
        let mut finished = None;
        for i in 1..(RECORD_FRAME_LIMIT as u64 + 10) {
            if let Step::Finished { sequence, .. } =
                gesture.frame(ms(i * 20), vec![contact(0, i as i32)])
            {
                finished = Some(sequence);
                break;
            }
        }
        let sequence = finished.expect("frame limit closes the gesture");
        assert!(sequence.frames.len() <= RECORD_FRAME_LIMIT);
        sequence.validate().unwrap();

        let mut gesture = GestureRecorder::default();
        assert_eq!(gesture.finish(ms(3)), Step::Idle);
        gesture.frame(ms(10), vec![contact(0, 0)]);
        let Step::Finished { sequence, .. } = gesture.finish(ms(10)) else {
            panic!();
        };
        sequence.validate().unwrap();
    }
}
