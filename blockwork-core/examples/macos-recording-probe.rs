//! An external, untagged HID probe for the packaged app's recording workflow.
//! Start recording a disposable macro, then run this example. It only targets
//! scripts/macos/test-window.swift, and verifies delivery in that window's log.
//! Normal Blockwork playback is tagged and deliberately excluded by recording.

#[cfg(all(target_os = "macos", feature = "system"))]
fn main() -> Result<(), String> {
    use blockwork_core::window::{self, Action, MatchBy};
    use core_graphics::{
        event::{
            CGEvent, CGEventFlags, CGEventTapLocation, CGEventType, CGMouseButton, EventField,
        },
        event_source::{CGEventSource, CGEventSourceStateID},
        geometry::CGPoint,
    };
    use foreign_types::ForeignType;
    unsafe extern "C" {
        fn CGEventCreateScrollWheelEvent2(
            source: core_graphics::sys::CGEventSourceRef,
            units: u32,
            wheels: u32,
            first: i32,
            second: i32,
            third: i32,
        ) -> core_graphics::sys::CGEventRef;
    }
    let title = std::env::var("BW_LIVE_WINDOW").map_err(|e| e.to_string())?;
    let gestures = std::env::args().any(|arg| arg == "--gestures");
    let stop_recording = std::env::args().any(|arg| arg == "--stop-recording");
    if title != "Blockwork macOS Test Window" {
        return Err("only target the dedicated native fixture".into());
    }
    let path = std::env::var("BLOCKWORK_TEST_EVENT_LOG").map_err(|e| e.to_string())?;
    let offset = std::fs::read(&path).map_err(|e| e.to_string())?.len();
    window::act(Action::Focus, &title, MatchBy::Title)?;
    let rect = window::bounds(&title, MatchBy::Title)?;
    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
        .map_err(|_| "couldn't create the probe's event source")?;
    let point = CGPoint::new(rect.x + 60.0, rect.y + 350.0);
    for kind in [
        CGEventType::MouseMoved,
        CGEventType::LeftMouseDown,
        CGEventType::LeftMouseUp,
    ] {
        CGEvent::new_mouse_event(source.clone(), kind, point, CGMouseButton::Left)
            .map_err(|_| "couldn't create the probe's pointer event")?
            .post(CGEventTapLocation::HID);
    }
    let shift = CGEventFlags::CGEventFlagShift | CGEventFlags::from_bits_retain(0x0002);
    for (code, down, flags) in [
        (0, true, CGEventFlags::empty()),
        (0, false, CGEventFlags::empty()),
        (56, true, shift),
        (11, true, shift),
        (11, false, shift),
        (56, false, CGEventFlags::empty()),
        (8, true, CGEventFlags::empty()),
        (8, false, CGEventFlags::empty()),
    ] {
        let event = CGEvent::new_keyboard_event(source.clone(), code, down)
            .map_err(|_| "couldn't create the probe's keyboard event")?;
        event.set_flags(flags);
        event.post(CGEventTapLocation::HID);
    }
    if gestures {
        let start = CGPoint::new(rect.x + 34.0, rect.y + 350.0);
        let end = CGPoint::new(rect.x + 94.0, rect.y + 350.0);
        for (kind, point) in [
            (CGEventType::MouseMoved, start),
            (CGEventType::LeftMouseDown, start),
            (CGEventType::LeftMouseDragged, end),
            (CGEventType::LeftMouseUp, end),
        ] {
            let event = CGEvent::new_mouse_event(source.clone(), kind, point, CGMouseButton::Left)
                .map_err(|_| "couldn't create the probe's drag event")?;
            if matches!(kind, CGEventType::LeftMouseDragged) {
                event.set_double_value_field(EventField::MOUSE_EVENT_DELTA_X, 60.0);
            }
            event.post(CGEventTapLocation::HID);
        }
        let raw = unsafe { CGEventCreateScrollWheelEvent2(source.as_ptr(), 1, 1, -2, 0, 0) };
        if raw.is_null() {
            return Err("couldn't create the probe's scroll event".into());
        }
        let scroll = unsafe { CGEvent::from_ptr(raw) };
        scroll.set_location(end);
        scroll.post(CGEventTapLocation::HID);
    }
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    loop {
        let log = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let rows: Vec<serde_json::Value> = log[offset..]
            .lines()
            .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
            .collect();
        let text: String = rows
            .iter()
            .filter(|row| row["type"] == 10)
            .filter_map(|row| row["characters"].as_str().map(str::to_owned))
            .collect();
        let gestures_delivered = !gestures
            || (rows.iter().any(|row| row["type"] == 6)
                && rows.iter().any(|row| row["type"] == 22));
        if text == "aBc" && gestures_delivered {
            if stop_recording {
                for down in [true, false] {
                    let event = CGEvent::new_keyboard_event(source.clone(), 53, down)
                        .map_err(|_| "couldn't create the probe's Escape event")?;
                    event.set_flags(CGEventFlags::empty());
                    event.post(CGEventTapLocation::HID);
                }
            }
            println!(
                "Untagged HID probe delivered aBc to the fixture; gestures={gestures}, stop key posted={stop_recording}"
            );
            return Ok(());
        }
        if std::time::Instant::now() >= deadline {
            return Err(format!("probe delivered {text:?}, expected aBc"));
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

#[cfg(not(all(target_os = "macos", feature = "system")))]
fn main() {
    eprintln!("This probe requires macOS and blockwork-core's system feature.");
}
