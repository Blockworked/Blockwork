//! Live touch checks against the mock desktop's fake touchscreen and uinput.
//! Run through `scripts/test-touch.sh`, which starts the broker and preloads
//! the fake device nodes.
#![cfg(target_os = "linux")]

use blockwork_core::input::touch::{TouchContact, TouchFrame, TouchSequence};
use blockwork_core::input::touch_playback;
use blockwork_core::input::types::{Axis, Direction, MacroButton, MacroKey};
use blockwork_core::macros::backend::InputBackend;
use blockwork_core::macros::{Instruction, InstructionKind};
use blockwork_core::recording;
use std::process::Command;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

fn fake(args: &[&str]) {
    let script = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../scripts/mock-desktop/fake-input.py"
    );
    let out = Command::new("python3")
        .arg(script)
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "fake-input {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

fn uinput_log() -> String {
    let dir = std::env::var("FAKE_EVDEV_DIR").expect("run through scripts/test-touch.sh");
    std::fs::read_to_string(format!("{dir}/uinput.log")).unwrap_or_default()
}

fn sequences(queue: &[Instruction]) -> Vec<TouchSequence> {
    queue
        .iter()
        .filter_map(|i| match &i.kind {
            InstructionKind::TouchSequence {
                sequence,
                normalized,
                ..
            } => {
                assert!(normalized, "recordings are normalized");
                Some(TouchSequence::parse(&sequence.eval_text().unwrap()).unwrap())
            }
            _ => None,
        })
        .collect()
}

/// Records what the fake screen does while `gestures` runs.
fn record(gestures: impl FnOnce()) -> Vec<Instruction> {
    recording::get_recording_queue().lock().unwrap().clear();
    recording::RECORD_TOUCH.store(true, Ordering::SeqCst);
    recording::reset_timing();
    recording::RECORDING_ACTIVE.store(true, Ordering::SeqCst);
    recording::start_sources();
    std::thread::sleep(Duration::from_millis(200));
    gestures();
    std::thread::sleep(Duration::from_millis(200));
    recording::RECORDING_ACTIVE.store(false, Ordering::SeqCst);
    recording::finish_sources();
    std::thread::sleep(Duration::from_millis(50));
    recording::get_recording_queue()
        .lock()
        .unwrap()
        .drain(..)
        .collect()
}

#[test]
#[ignore = "needs the fake touchscreen (scripts/test-touch.sh)"]
fn records_swipes_pinches_and_taps_from_a_touchscreen() {
    assert_eq!(
        recording::touch::devices(),
        ["Fake Touchscreen"],
        "the fake screen is found as a direct multitouch device"
    );
    let queue = record(|| {
        fake(&["swipe", "0.1", "0.5", "0.9", "0.5", "300"]);
        std::thread::sleep(Duration::from_millis(400));
        fake(&["pinch", "0.5", "0.5", "0.2", "0.6", "300"]);
        std::thread::sleep(Duration::from_millis(400));
        fake(&["swipe", "0.2", "0.8", "0.2", "0.2", "250", "3"]);
        std::thread::sleep(Duration::from_millis(400));
        fake(&["touch", "--ms=60", "0.25,0.75", "0.25,0.75"]);
    });
    let paths = sequences(&queue);
    assert_eq!(paths.len(), 4, "one block per gesture: {queue:#?}");
    let close = |a: i32, b: f64| (a as f64 / 1_000_000.0 - b).abs() < 0.01;

    // Swipe: one finger left to right, ending with every finger lifted.
    let swipe = &paths[0];
    let first = &swipe.frames[0].contacts;
    let last_down = &swipe.frames[swipe.frames.len() - 2].contacts;
    assert_eq!(first.len(), 1);
    assert!(
        close(first[0].x, 0.1) && close(first[0].y, 0.5),
        "{first:?}"
    );
    assert!(close(last_down[0].x, 0.9), "{last_down:?}");
    assert!(swipe.frames.last().unwrap().contacts.is_empty());
    let ms = swipe.frames.last().unwrap().at_ms;
    assert!(
        (250..=600).contains(&ms),
        "the swipe keeps its pace: {ms} ms"
    );
    assert!(swipe.frames.len() >= 10, "{} frames", swipe.frames.len());

    // Pinch: two fingers, spreading from 0.2 to 0.6 apart.
    let pinch = &paths[1];
    let spread = |f: &TouchFrame| (f.contacts[1].x - f.contacts[0].x) as f64 / 1_000_000.0;
    assert_eq!(pinch.frames[0].contacts.len(), 2);
    assert!((spread(&pinch.frames[0]) - 0.2).abs() < 0.01);
    assert!((spread(&pinch.frames[pinch.frames.len() - 2]) - 0.6).abs() < 0.01);

    // Three-finger swipe up keeps three ids for the whole gesture.
    let three = &paths[2];
    assert!(
        three.frames[..three.frames.len() - 1].iter().all(|f| f
            .contacts
            .iter()
            .map(|c| c.id)
            .collect::<Vec<_>>()
            == [0, 1, 2])
    );
    assert!(close(
        three.frames[three.frames.len() - 2].contacts[0].y,
        0.2
    ));

    // A still tap merges its identical frames.
    let tap = &paths[3];
    assert_eq!(tap.frames.len(), 2, "{tap:?}");
    assert!(close(tap.frames[0].contacts[0].x, 0.25));

    // Waits between the gestures come from their timing.
    let waits: Vec<f64> = queue
        .iter()
        .filter_map(|i| match &i.kind {
            InstructionKind::Wait(v) => v.eval_number().ok(),
            _ => None,
        })
        .collect();
    assert!(
        waits.iter().filter(|w| **w >= 300.0).count() >= 3,
        "{waits:?}"
    );
}

#[test]
#[ignore = "needs the fake touchscreen (scripts/test-touch.sh)"]
fn a_gesture_still_down_when_recording_stops_is_finished() {
    let queue = record(|| {
        let script = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../scripts/mock-desktop/fake-input.py"
        );
        // Stop recording while the swipe is still moving.
        let mut swipe = Command::new("python3")
            .args([script, "swipe", "0.1", "0.1", "0.9", "0.9", "1500"])
            .spawn()
            .unwrap();
        std::thread::sleep(Duration::from_millis(500));
        recording::RECORDING_ACTIVE.store(false, Ordering::SeqCst);
        let _ = swipe.wait();
    });
    let paths = sequences(&queue);
    assert_eq!(paths.len(), 1, "{queue:#?}");
    let path = &paths[0];
    assert!(
        path.frames.last().unwrap().contacts.is_empty(),
        "fingers are lifted"
    );
    assert!(path.frames.len() >= 3);
}

/// Forwards touch to the native provider, as the evdev backend does.
struct TouchOnly;
impl InputBackend for TouchOnly {
    fn ensure_touch_support(&mut self) -> Result<(), String> {
        blockwork_core::input::native_touch::prepare()
    }
    fn touch_frame(&mut self, contacts: &[TouchContact]) -> Result<(), String> {
        blockwork_core::input::native_touch::frame(contacts)
    }
    fn cancel_touch(&mut self) -> Result<(), String> {
        blockwork_core::input::native_touch::cancel()
    }
    fn key(&mut self, _: MacroKey, _: Direction) -> Result<(), String> {
        unreachable!()
    }
    fn raw_keycode(&mut self, _: u16, _: Direction) -> Result<(), String> {
        unreachable!()
    }
    fn button(&mut self, _: MacroButton, _: Direction) -> Result<(), String> {
        unreachable!()
    }
    fn move_mouse_rel(&mut self, _: i32, _: i32) -> Result<(), String> {
        unreachable!()
    }
    fn move_mouse_abs(&mut self, _: i32, _: i32) -> Result<(), String> {
        unreachable!()
    }
    fn scroll(&mut self, _: i32, _: Axis) -> Result<(), String> {
        unreachable!()
    }
    fn text(&mut self, _: &str) -> Result<(), String> {
        unreachable!()
    }
    fn cursor_pos(&self) -> Option<(i32, i32)> {
        None
    }
}

fn contact(id: u8, x: i32, y: i32) -> TouchContact {
    TouchContact { id, x, y }
}

fn frame(at_ms: u32, contacts: Vec<TouchContact>) -> TouchFrame {
    TouchFrame { at_ms, contacts }
}

/// Lines the virtual touchscreen wrote, without their timestamps, once the
/// broker has logged everything it was sent.
fn virtual_events(since: usize) -> Vec<String> {
    let mut seen = usize::MAX;
    for _ in 0..40 {
        std::thread::sleep(Duration::from_millis(50));
        let now = uinput_log().lines().count();
        if now == seen {
            break;
        }
        seen = now;
    }
    uinput_log()
        .lines()
        .skip(since)
        .filter_map(|l| l.split_once(' ').map(|(_, rest)| rest.to_owned()))
        .filter_map(|l| {
            l.strip_prefix("Blockwork Virtual Touch ")
                .map(str::to_owned)
        })
        .collect()
}

#[test]
#[ignore = "needs the fake uinput (scripts/test-touch.sh)"]
fn plays_touch_paths_through_a_virtual_touchscreen() {
    let monitors = blockwork_core::screen::monitors();
    assert_eq!(monitors.len(), 1, "one monitor: {monitors:?}");
    let screen = monitors[0];
    let backend: Arc<Mutex<dyn InputBackend>> = Arc::new(Mutex::new(TouchOnly));
    let before = uinput_log().lines().count();

    // A normalized two-finger pinch, with one finger lifting early.
    let path = TouchSequence {
        version: 1,
        frames: vec![
            frame(
                0,
                vec![contact(0, 400_000, 500_000), contact(1, 600_000, 500_000)],
            ),
            frame(
                40,
                vec![contact(0, 300_000, 500_000), contact(1, 700_000, 500_000)],
            ),
            frame(80, vec![contact(1, 800_000, 500_000)]),
            frame(120, vec![]),
        ],
    };
    let mapped = touch_playback::map_to_screen(&path, true, screen).unwrap();
    let start = Instant::now();
    touch_playback::play(&mapped, backend.clone(), 1.0, || false).unwrap();
    let took = start.elapsed();
    assert!(
        took >= Duration::from_millis(120) && took < Duration::from_millis(500),
        "{took:?}"
    );

    let events = virtual_events(before);
    let px = |f: f64, size: f64| ((size - 1.0) * f).round() as i32;
    let (w, h) = (screen.width, screen.height);
    let expect = [
        format!("abs mt_x {}", px(0.4, w)),
        format!("abs mt_x {}", px(0.6, w)),
        format!("abs mt_y {}", px(0.5, h)),
        format!("abs mt_x {}", px(0.8, w)),
        "abs tracking_id -1".to_owned(),
        "key BTN_TOUCH down".to_owned(),
        "key BTN_TOUCH up".to_owned(),
    ];
    for line in &expect {
        assert!(events.contains(line), "missing {line:?} in {events:#?}");
    }
    // Each frame is one sync, plus the release on the way out.
    let syncs = events.iter().filter(|e| *e == "sync").count();
    assert!(syncs >= 4, "{syncs} syncs: {events:#?}");
    // Nothing is left down: the last BTN_TOUCH is up and every used slot lifted.
    let last_touch = events
        .iter()
        .rev()
        .find(|e| e.starts_with("key BTN_TOUCH"))
        .unwrap();
    assert_eq!(last_touch, "key BTN_TOUCH up");

    // A path cut short by the caller still releases every finger.
    let before = uinput_log().lines().count();
    let long = TouchSequence {
        version: 1,
        frames: vec![
            frame(0, vec![contact(0, 500_000, 500_000)]),
            frame(2000, vec![]),
        ],
    };
    let mapped = touch_playback::map_to_screen(&long, true, screen).unwrap();
    let stop_at = Instant::now() + Duration::from_millis(100);
    let error =
        touch_playback::play(&mapped, backend, 1.0, || Instant::now() > stop_at).unwrap_err();
    assert!(error.contains("stopped"), "{error}");
    let events = virtual_events(before);
    assert_eq!(
        events
            .iter()
            .rev()
            .find(|e| e.starts_with("key BTN_TOUCH"))
            .unwrap(),
        "key BTN_TOUCH up",
        "{events:#?}"
    );
    assert_eq!(
        events.iter().filter(|e| *e == "abs tracking_id -1").count(),
        10,
        "every slot is released"
    );
}
