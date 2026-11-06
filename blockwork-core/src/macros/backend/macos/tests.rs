use super::*;

#[test]
fn swallowed_hotkeys_swallow_their_repeats_and_release() {
    let mut keys = SuppressedKeys::default();
    assert!(keys.filter(10, 1, false).is_none());
    assert!(matches!(
        keys.decide(10, 1, CaptureDecision::Suppress),
        CaptureDecision::Suppress
    ));
    assert!(matches!(
        keys.filter(10, 1, true),
        Some(CaptureDecision::Suppress)
    ));
    assert!(matches!(
        keys.filter(10, 2, true),
        Some(CaptureDecision::Passthrough)
    ));
    assert!(matches!(
        keys.decide(11, 1, CaptureDecision::Passthrough),
        CaptureDecision::Suppress
    ));
    assert!(matches!(
        keys.decide(11, 1, CaptureDecision::Passthrough),
        CaptureDecision::Passthrough
    ));
    assert!(matches!(
        keys.decide(10, 1, CaptureDecision::Passthrough),
        CaptureDecision::Passthrough
    ));
    assert!(matches!(
        keys.filter(10, 1, true),
        Some(CaptureDecision::Passthrough)
    ));
}

#[test]
fn media_events_preserve_key_direction_and_modifiers() {
    for key in [
        MacroKey::VolumeUp,
        MacroKey::VolumeDown,
        MacroKey::VolumeMute,
    ] {
        for down in [true, false] {
            let event = system_keys::event(
                system_keys::code(&key).unwrap(),
                down,
                CGEventFlags::CGEventFlagShift,
            )
            .unwrap();
            let captured = system_keys::capture(&event).expect("media event");
            match captured {
                CaptureEvent::KeyPress(actual) if down => assert_eq!(actual, key),
                CaptureEvent::KeyRelease(actual) if !down => assert_eq!(actual, key),
                _ => panic!("wrong media event direction"),
            }
            assert_eq!(event.get_flags(), CGEventFlags::CGEventFlagShift);
        }
    }
}

#[test]
#[ignore = "dispatches and suppresses media events before they change system volume"]
fn live_media_keys_are_dispatched_and_captured() {
    use std::sync::{Arc, Mutex};
    let received = Arc::new(Mutex::new(Vec::new()));
    let captured = Arc::clone(&received);
    // Install the suppressing tap before posting any volume event.
    let tap = system_keys::KeyTap::new(move |kind, event| {
        if kind == 14
            && event.get_integer_value_field(EventField::EVENT_SOURCE_USER_DATA)
                == SYNTHETIC_EVENT_TAG
        {
            if let Some(event) = system_keys::capture(event) {
                if let Ok(mut received) = captured.lock() {
                    received.push(event);
                }
            }
            return CaptureDecision::Suppress;
        }
        CaptureDecision::Passthrough
    })
    .expect("media test requires Accessibility access");
    let source = tap.mach_port().create_runloop_source(0).unwrap();
    let run_loop = CFRunLoop::get_current();
    run_loop.add_source(&source, unsafe { kCFRunLoopDefaultMode });
    tap.enable();
    let mut backend = MacosBackend::new().unwrap();
    for key in [
        MacroKey::VolumeUp,
        MacroKey::VolumeDown,
        MacroKey::VolumeMute,
    ] {
        backend.key(key, Direction::Click).unwrap();
    }
    CFRunLoop::run_in_mode(
        unsafe { kCFRunLoopDefaultMode },
        std::time::Duration::from_millis(500),
        false,
    );
    run_loop.remove_source(&source, unsafe { kCFRunLoopDefaultMode });
    let received = received.lock().unwrap();
    assert_eq!(
        received.len(),
        6,
        "each media click must deliver down and up"
    );
    for (pair, expected) in received.chunks_exact(2).zip([
        MacroKey::VolumeUp,
        MacroKey::VolumeDown,
        MacroKey::VolumeMute,
    ]) {
        assert!(matches!(&pair[0], CaptureEvent::KeyPress(key) if *key == expected));
        assert!(matches!(&pair[1], CaptureEvent::KeyRelease(key) if *key == expected));
    }
}

#[test]
fn unavailable_native_keys_fail_clearly_for_every_direction() {
    let mut backend = MacosBackend::new().unwrap();
    for key in [
        MacroKey::F21,
        MacroKey::F22,
        MacroKey::F23,
        MacroKey::F24,
        MacroKey::Select,
    ] {
        for direction in [Direction::Press, Direction::Release, Direction::Click] {
            let error = backend.key(key.clone(), direction).unwrap_err();
            assert!(
                error.contains(&format!("{key:?}"))
                    && error.contains("no native macOS virtual-key code")
            );
        }
    }
}

#[test]
fn pc_navigation_keys_follow_apples_usb_keyboard_translation() {
    for (key, native, canonical) in [
        (MacroKey::PrintScr, 105, MacroKey::F13),
        (MacroKey::ScrollLock, 107, MacroKey::F14),
        (MacroKey::Pause, 113, MacroKey::F15),
    ] {
        let (code, shifted) = macro_key_to_cgkeycode(&key).expect("PC key has a native alias");
        assert_eq!(code, native);
        assert!(!shifted);
        assert_eq!(cg_keycode_to_macro_key(code), Some(canonical));
        let mut backend = MacosBackend::new().unwrap();
        for down in [true, false] {
            let event = backend.keyboard_event(code, down).unwrap();
            assert!(
                event
                    .get_flags()
                    .contains(CGEventFlags::CGEventFlagSecondaryFn)
            );
        }
    }
}

#[test]
fn extended_keys_round_trip_without_becoming_unknown_codes() {
    for code in [
        64, 65, 67, 69, 71, 72, 73, 74, 75, 78, 79, 80, 82, 83, 84, 85, 86, 87, 88, 89, 90, 91, 92,
        105, 106, 107, 113, 114,
    ] {
        let key = cg_keycode_to_macro_key(code).unwrap();
        assert!(!matches!(key, MacroKey::Other(_)), "code {code}");
        assert_eq!(macro_key_to_cgkeycode(&key), Some((code, false)));
    }
    assert_eq!(macro_key_to_cgkeycode(&MacroKey::Other(65536)), None);
}

#[test]
fn unicode_events_preserve_non_us_text_and_surrogate_pairs() {
    unsafe extern "C" {
        fn CGEventKeyboardGetUnicodeString(
            event: core_graphics::sys::CGEventRef,
            max: usize,
            actual: *mut usize,
            text: *mut u16,
        );
    }
    let backend = MacosBackend::new().unwrap();
    for text in ["é", "中", "🦀", "A", "\n", "\t"] {
        for down in [true, false] {
            let event = backend.unicode_event(text, down).unwrap();
            let mut utf16 = [0; 8];
            let mut count = 0;
            unsafe {
                CGEventKeyboardGetUnicodeString(
                    event.as_ptr(),
                    utf16.len(),
                    &mut count,
                    utf16.as_mut_ptr(),
                );
            }
            assert_eq!(String::from_utf16(&utf16[..count]).unwrap(), text);
            assert_eq!(event.get_flags(), CGEventFlags::empty());
            assert_eq!(
                event.get_type() as u32,
                if down {
                    CGEventType::KeyDown as u32
                } else {
                    CGEventType::KeyUp as u32
                }
            );
        }
    }
}

#[test]
fn right_modifier_release_is_seen_while_left_is_still_held() {
    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState).unwrap();
    let event = CGEvent::new_keyboard_event(source, 60, false).unwrap();
    event.set_flags(CGEventFlags::from_bits_retain(
        CGEventFlags::CGEventFlagShift.bits() | 0x0002,
    ));
    assert!(matches!(
        modifier_event(&event),
        Some(CaptureEvent::KeyRelease(MacroKey::RShift))
    ));
    event.set_flags(CGEventFlags::from_bits_retain(
        CGEventFlags::CGEventFlagShift.bits() | 0x0002 | 0x0004,
    ));
    assert!(matches!(
        modifier_event(&event),
        Some(CaptureEvent::KeyPress(MacroKey::RShift))
    ));
}

#[test]
fn function_key_flags_do_not_leak_into_later_letters() {
    let stale = CGEventFlags::CGEventFlagSecondaryFn
        | CGEventFlags::CGEventFlagNumericPad
        | CGEventFlags::CGEventFlagHelp
        | CGEventFlags::CGEventFlagShift;
    let flags = keyboard_flags(8, stale, false);
    assert_eq!(flags, CGEventFlags::CGEventFlagShift);
    assert!(keyboard_flags(8, stale, true).contains(CGEventFlags::CGEventFlagSecondaryFn));
    assert!(
        keyboard_flags(79, CGEventFlags::empty(), false)
            .contains(CGEventFlags::CGEventFlagSecondaryFn)
    );
}

#[test]
fn menu_alias_holds_option_for_the_following_combo_key() {
    let mut backend = MacosBackend::new().unwrap();
    let keys = crate::input::key_names::parse_key_combo("LMenu+a");
    let (modifier, _) = macro_key_to_cgkeycode(&keys[0]).unwrap();
    backend.keyboard_event(modifier, true).unwrap();
    let (letter, _) = macro_key_to_cgkeycode(&keys[1]).unwrap();
    let press = backend.keyboard_event(letter, true).unwrap();
    assert!(
        press
            .get_flags()
            .contains(CGEventFlags::CGEventFlagAlternate)
    );
    backend.keyboard_event(modifier, false).unwrap();
    assert!(
        !backend
            .keyboard_event(letter, false)
            .unwrap()
            .get_flags()
            .contains(CGEventFlags::CGEventFlagAlternate)
    );
    // The distinct physical application-menu key must retain its raw code.
    assert_eq!(cg_keycode_to_macro_key(110), Some(MacroKey::Other(110)));
    assert_eq!(
        macro_key_to_cgkeycode(&MacroKey::Other(110)),
        Some((110, false))
    );
}

#[test]
fn queued_modifier_flags_do_not_depend_on_os_delivery_timing() {
    let mut backend = MacosBackend::new().unwrap();
    backend.keyboard_event(56, true).unwrap();
    assert!(
        backend
            .keyboard_event(0, true)
            .unwrap()
            .get_flags()
            .contains(CGEventFlags::CGEventFlagShift)
    );
    backend.keyboard_event(60, true).unwrap();
    backend.keyboard_event(56, false).unwrap();
    assert!(
        backend
            .keyboard_event(11, true)
            .unwrap()
            .get_flags()
            .contains(CGEventFlags::CGEventFlagShift)
    );
    backend.keyboard_event(60, false).unwrap();
    assert!(
        !backend
            .keyboard_event(8, true)
            .unwrap()
            .get_flags()
            .contains(CGEventFlags::CGEventFlagShift)
    );
}

#[test]
fn fractional_scroll_accumulates_both_axes_in_line_units() {
    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState).unwrap();
    let event = CGEvent::new(source).unwrap();
    event.set_type(CGEventType::ScrollWheel);
    event.set_double_value_field(
        EventField::SCROLL_WHEEL_EVENT_FIXED_POINT_DELTA_AXIS_1,
        0.25,
    );
    event.set_double_value_field(
        EventField::SCROLL_WHEEL_EVENT_FIXED_POINT_DELTA_AXIS_2,
        -0.5,
    );
    let mut scroll = ScrollAccumulator::default();
    assert_eq!(scroll.read(&event), (0, 0));
    // A negative (rightward) CoreGraphics delta is a positive Blockwork one.
    assert_eq!(scroll.read(&event), (1, 0));
    assert_eq!(scroll.read(&event), (0, 0));
    assert_eq!(scroll.read(&event), (1, 1));
}

#[test]
fn native_scroll_pixels_convert_to_lines_without_tenfold_playback() {
    let backend = MacosBackend::new().unwrap();
    for (units, amount) in [(0, 10), (1, 1)] {
        let raw = unsafe {
            CGEventCreateScrollWheelEvent2(backend.source.as_ptr(), units, 1, amount, 0, 0)
        };
        assert!(!raw.is_null());
        let event = unsafe { CGEvent::from_ptr(raw) };
        assert_eq!(ScrollAccumulator::default().read(&event), (0, 1));
    }
}

#[test]
fn dragging_uses_the_held_button_and_absolute_target() {
    let mut backend = MacosBackend::new().unwrap();
    for (mask, kind, button) in [
        (0, CGEventType::MouseMoved, 0),
        (1, CGEventType::LeftMouseDragged, 0),
        (2, CGEventType::RightMouseDragged, 1),
        (8, CGEventType::OtherMouseDragged, 3),
    ] {
        backend.held_buttons = mask;
        let event = backend.movement_event(-20, 300, -100, 42).unwrap();
        assert_eq!(event.get_type() as u32, kind as u32);
        assert_eq!(
            event.get_integer_value_field(EventField::MOUSE_EVENT_BUTTON_NUMBER),
            button
        );
        assert_eq!((event.location().x, event.location().y), (-20.0, 300.0));
        assert_eq!(
            event.get_double_value_field(EventField::MOUSE_EVENT_DELTA_X),
            -100.0
        );
    }
}

#[test]
#[ignore = "creates and disables a real macOS event tap"]
fn live_disabled_event_tap_recovers() {
    use core_foundation::base::TCFType;
    let tap = CGEventTap::new(
        CGEventTapLocation::HID,
        CGEventTapPlacement::HeadInsertEventTap,
        CGEventTapOptions::ListenOnly,
        vec![CGEventType::KeyDown],
        |_, _, event| CallbackResult::Replace(event.clone()),
    )
    .expect("event tap needs Accessibility access");
    let port = tap.mach_port().as_concrete_TypeRef();
    unsafe {
        CGEventTapEnable(port, false);
    }
    assert!(!unsafe { CGEventTapIsEnabled(port) });
    assert!(recover_tap(CGEventType::TapDisabledByTimeout, port));
    assert!(unsafe { CGEventTapIsEnabled(port) });
}

#[test]
#[ignore = "installs a real macOS event tap and suppresses an F18 probe"]
fn live_capture_records_and_suppresses_a_keyboard_probe() {
    let title = std::env::var("BW_LIVE_WINDOW").expect("set BW_LIVE_WINDOW");
    let path = std::env::var("BLOCKWORK_TEST_EVENT_LOG").expect("set BLOCKWORK_TEST_EVENT_LOG");
    crate::window::act(
        crate::window::Action::Focus,
        &title,
        crate::window::MatchBy::Title,
    )
    .unwrap();
    let offset = std::fs::read(&path).unwrap().len();
    let (send, receive) = std::sync::mpsc::channel();
    start_capture_thread(Box::new(move |event, timestamp| {
        let down = match event {
            CaptureEvent::KeyPress(MacroKey::F18) => true,
            CaptureEvent::KeyRelease(MacroKey::F18) => false,
            _ => return CaptureDecision::Passthrough,
        };
        if send.send((down, timestamp)).is_ok() {
            CaptureDecision::Suppress
        } else {
            CaptureDecision::Passthrough
        }
    }));
    std::thread::sleep(std::time::Duration::from_millis(300));
    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState).unwrap();
    // Untagged probes go through the real tap, unlike Blockwork playback.
    for down in [true, false] {
        let event = CGEvent::new_keyboard_event(source.clone(), 79, down).unwrap();
        event.post(CGEventTapLocation::HID);
    }
    let (down, first) = receive
        .recv_timeout(std::time::Duration::from_secs(2))
        .expect("keyboard tap needs Accessibility access");
    let (up, last) = receive
        .recv_timeout(std::time::Duration::from_secs(2))
        .unwrap();
    assert!(down && !up);
    let (CaptureTimestamp::Hardware(first), CaptureTimestamp::Hardware(last)) = (first, last)
    else {
        panic!("macOS must preserve hardware timestamps")
    };
    assert!(last.duration_since(first).is_ok());
    let probe_count = || {
        let log = std::fs::read_to_string(&path).unwrap();
        log[offset..]
            .lines()
            .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
            .filter(|row| row["keyCode"] == 79)
            .count()
    };
    std::thread::sleep(std::time::Duration::from_millis(50));
    assert_eq!(
        probe_count(),
        0,
        "suppressed input must not reach the fixture"
    );
    let mut backend = MacosBackend::new().unwrap();
    backend.raw_keycode(79, Direction::Click).unwrap();
    assert!(
        receive
            .recv_timeout(std::time::Duration::from_millis(150))
            .is_err(),
        "Blockwork playback must not feed back into capture"
    );
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    while probe_count() < 2 && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert_eq!(
        probe_count(),
        2,
        "tagged playback must still reach the fixture"
    );
}

#[test]
fn extra_mouse_buttons_keep_their_identity() {
    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState).unwrap();
    for button in [
        MacroButton::Middle,
        MacroButton::Back,
        MacroButton::Forward,
        MacroButton::Other(9),
    ] {
        let number = mouse_button_number(&button).unwrap();
        let event = CGEvent::new_mouse_event(
            source.clone(),
            CGEventType::OtherMouseDown,
            CGPoint::new(0.0, 0.0),
            CGMouseButton::Center,
        )
        .unwrap();
        event.set_integer_value_field(EventField::MOUSE_EVENT_BUTTON_NUMBER, number as i64);
        assert_eq!(captured_mouse_button(&event), button);
    }
    assert!(mouse_button_number(&MacroButton::Other(32)).is_err());
}

#[test]
fn queued_cursor_waits_for_every_move_and_resyncs_with_physical_input() {
    let mut cursor = QueuedCursor::default();
    assert_eq!(cursor.resolve((294, 1158), [10, 20, 30, 40]), (294, 1158));
    cursor.enqueue((260, 1158), CGEventType::MouseMoved);
    assert_eq!(cursor.resolve((294, 1158), [10, 20, 30, 40]), (260, 1158));
    cursor.enqueue((234, 1158), CGEventType::MouseMoved);
    cursor.enqueue((294, 1158), CGEventType::LeftMouseDragged);
    // Returning to the starting location must not acknowledge queued moves.
    assert_eq!(cursor.resolve((294, 1158), [10, 20, 30, 40]), (294, 1158));
    assert!(cursor.target.is_some());
    assert_eq!(cursor.resolve((260, 1158), [11, 20, 30, 40]), (294, 1158));
    assert_eq!(cursor.resolve((234, 1158), [12, 20, 30, 40]), (294, 1158));
    assert_eq!(cursor.resolve((294, 1158), [12, 21, 30, 40]), (294, 1158));
    assert!(cursor.target.is_none());
    assert_eq!(cursor.resolve((600, 800), [13, 21, 30, 40]), (600, 800));
    // WindowServer counters wrap without losing the pending move.
    cursor.resolve((600, 800), [u32::MAX, 21, 30, 40]);
    cursor.enqueue((610, 810), CGEventType::MouseMoved);
    assert_eq!(
        cursor.resolve((600, 800), [u32::MAX, 21, 30, 40]),
        (610, 810)
    );
    assert_eq!(cursor.resolve((610, 810), [0, 21, 30, 40]), (610, 810));
    assert!(cursor.target.is_none());
}

#[test]
fn capture_preserves_event_positions_and_raw_relative_deltas() {
    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState).unwrap();
    let event = CGEvent::new_mouse_event(
        source,
        CGEventType::LeftMouseDragged,
        CGPoint::new(543.0, 654.0),
        CGMouseButton::Left,
    )
    .unwrap();
    event.set_double_value_field(EventField::MOUSE_EVENT_DELTA_X, 10.0);
    event.set_double_value_field(EventField::MOUSE_EVENT_DELTA_Y, -20.0);
    assert!(matches!(
        captured_movement(&event, false),
        CaptureEvent::MouseMoveAbs(543.0, 654.0)
    ));
    assert!(matches!(
        captured_movement(&event, true),
        CaptureEvent::MouseMoveRel(10, -20)
    ));
}
#[test]
#[ignore = "posts input to scripts/macos/test-window.swift"]
fn live_mouse_queue_preserves_rapid_relative_moves_clicks_drags_and_scroll() {
    let title = std::env::var("BW_LIVE_WINDOW").unwrap();
    assert_eq!(title, "Blockwork macOS Test Window");
    let path = std::env::var("BLOCKWORK_TEST_EVENT_LOG").unwrap();
    crate::window::act(
        crate::window::Action::Focus,
        &title,
        crate::window::MatchBy::Title,
    )
    .unwrap();
    let rect = crate::window::bounds(&title, crate::window::MatchBy::Title).unwrap();
    let x = rect.x as i32 + 60;
    let y = rect.y as i32 + 350;
    let offset = std::fs::read(&path).unwrap().len();
    let mut backend = MacosBackend::new().unwrap();
    backend.move_mouse_abs(x, y).unwrap();
    backend.move_mouse_rel(10, 0).unwrap();
    backend.move_mouse_rel(20, 0).unwrap();
    backend.button(MacroButton::Left, Direction::Press).unwrap();
    backend.move_mouse_rel(60, 0).unwrap();
    backend
        .button(MacroButton::Left, Direction::Release)
        .unwrap();
    backend.scroll(-2, Axis::Vertical).unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    loop {
        let log = std::fs::read_to_string(&path).unwrap();
        let rows: Vec<serde_json::Value> = log[offset..]
            .lines()
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect();
        if rows.iter().any(|row| row["type"] == 22) {
            let events: Vec<_> = rows
                .iter()
                .filter(|row| matches!(row["type"].as_u64(), Some(1 | 2 | 6 | 22)))
                .map(|row| {
                    (
                        row["type"].as_i64().unwrap(),
                        row["x"].as_f64().unwrap() as i32,
                        row["y"].as_f64().unwrap() as i32,
                        row["dx"].as_f64().unwrap() as i32,
                    )
                })
                .collect();
            assert_eq!(
                events,
                vec![
                    (1, x + 30, y, 0),
                    (6, x + 90, y, 60),
                    (2, x + 90, y, 0),
                    (22, x + 90, y, 0)
                ]
            );
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "fixture did not receive the gesture: {rows:?}"
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    // After delivery, a physical move must replace the queued position.
    let point = CGPoint::new((x + 100) as f64, y as f64);
    let external = CGEvent::new_mouse_event(
        backend.source.clone(),
        CGEventType::MouseMoved,
        point,
        CGMouseButton::Left,
    )
    .unwrap();
    external.post(CGEventTapLocation::HID);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while backend.cursor_pos() != Some((x + 100, y)) {
        assert!(
            std::time::Instant::now() < deadline,
            "cursor did not resync"
        );
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

#[test]
#[ignore = "posts and intercepts native keyboard input; requires Accessibility"]
fn live_pc_key_aliases_deliver_matching_native_pairs() {
    use std::sync::{Arc, Mutex};
    let received = Arc::new(Mutex::new(Vec::new()));
    let captured = Arc::clone(&received);
    let tap = system_keys::KeyTap::new(move |kind, event| {
        if matches!(kind, 10 | 11)
            && event.get_integer_value_field(EventField::EVENT_SOURCE_USER_DATA)
                == SYNTHETIC_EVENT_TAG
        {
            captured.lock().unwrap().push((
                event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE),
                kind,
                event.get_flags().bits(),
            ));
            // F14/F15 are system brightness shortcuts on macOS. Observe
            // their native pairs before the OS consumes them, and prevent
            // this opt-in test from changing the user's brightness.
            return CaptureDecision::Suppress;
        }
        CaptureDecision::Passthrough
    })
    .expect("native-key test requires Accessibility access");
    let source = tap.mach_port().create_runloop_source(0).unwrap();
    let run_loop = CFRunLoop::get_current();
    run_loop.add_source(&source, unsafe { kCFRunLoopDefaultMode });
    tap.enable();
    let mut backend = MacosBackend::new().unwrap();
    for key in [MacroKey::PrintScr, MacroKey::ScrollLock, MacroKey::Pause] {
        backend.key(key, Direction::Click).unwrap();
    }
    CFRunLoop::run_in_mode(
        unsafe { kCFRunLoopDefaultMode },
        std::time::Duration::from_millis(500),
        false,
    );
    run_loop.remove_source(&source, unsafe { kCFRunLoopDefaultMode });
    let received = received.lock().unwrap();
    assert_eq!(
        received.len(),
        6,
        "each alias needs one native down and up: {received:?}"
    );
    for (pair, code) in received.chunks_exact(2).zip([105, 107, 113]) {
        assert_eq!((pair[0].0, pair[0].1), (code, 10));
        assert_eq!((pair[1].0, pair[1].1), (code, 11));
        for event in pair {
            assert_ne!(event.2 & CGEventFlags::CGEventFlagSecondaryFn.bits(), 0);
        }
    }
}

#[test]
#[ignore = "posts input to scripts/macos/test-window.swift"]
fn live_keyboard_playback_preserves_text_and_modifier_order() {
    let title =
        std::env::var("BW_LIVE_WINDOW").expect("set BW_LIVE_WINDOW to the native fixture title");
    let path = std::env::var("BLOCKWORK_TEST_EVENT_LOG")
        .expect("set BLOCKWORK_TEST_EVENT_LOG to the fixture log");
    crate::window::act(
        crate::window::Action::Focus,
        &title,
        crate::window::MatchBy::Title,
    )
    .unwrap();
    assert!(
        crate::window::active()
            .unwrap()
            .matches(&title, crate::window::MatchBy::Title)
    );
    let offset = std::fs::read(&path).unwrap().len();
    let mut backend = MacosBackend::new().unwrap();
    backend.key(MacroKey::LShift, Direction::Press).unwrap();
    backend
        .key(MacroKey::Unicode('a'), Direction::Click)
        .unwrap();
    backend.key(MacroKey::RShift, Direction::Press).unwrap();
    backend.key(MacroKey::LShift, Direction::Release).unwrap();
    backend
        .key(MacroKey::Unicode('b'), Direction::Click)
        .unwrap();
    backend.key(MacroKey::RShift, Direction::Release).unwrap();
    backend
        .key(MacroKey::Unicode('c'), Direction::Click)
        .unwrap();
    backend.text("é中🦀").unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    loop {
        let log = std::fs::read_to_string(&path).unwrap();
        let typed: String = log[offset..]
            .lines()
            .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
            .filter(|row| row["type"] == 10)
            .filter_map(|row| row["characters"].as_str().map(str::to_owned))
            .collect();
        if typed.chars().count() >= 6 || std::time::Instant::now() >= deadline {
            assert_eq!(typed, "ABcé中🦀");
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let offset = std::fs::read(&path).unwrap().len();
    backend.key(MacroKey::LMenu, Direction::Press).unwrap();
    backend
        .key(MacroKey::Unicode('x'), Direction::Click)
        .unwrap();
    backend.key(MacroKey::LMenu, Direction::Release).unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    loop {
        let log = std::fs::read_to_string(&path).unwrap();
        let rows: Vec<serde_json::Value> = log[offset..]
            .lines()
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect();
        if let Some(press) = rows
            .iter()
            .find(|row| row["type"] == 10 && row["keyCode"] == 7)
        {
            assert_ne!(
                press["flags"].as_u64().unwrap() & CGEventFlags::CGEventFlagAlternate.bits(),
                0
            );
            if rows.iter().any(|row| {
                row["type"] == 12
                    && row["flags"].as_u64().unwrap() & CGEventFlags::CGEventFlagAlternate.bits()
                        == 0
            }) {
                break;
            }
        }
        assert!(
            std::time::Instant::now() < deadline,
            "LMenu option combo/release did not reach the fixture: {rows:?}"
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}
