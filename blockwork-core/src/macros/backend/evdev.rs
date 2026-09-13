use std::collections::HashSet;
use std::io;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Mutex, OnceLock};

use evdev::{AbsoluteAxisCode, AttributeSet, EventType, InputEvent, KeyCode, PropType, RelativeAxisCode, uinput::VirtualDevice, uinput::VirtualDeviceBuilder};
use tracing::warn;

use crate::input::types::{Axis, Direction, MacroButton, MacroKey};
use crate::macros::backend::{CaptureDecision, CaptureEvent, CaptureTimestamp, InputBackend};

use super::evdev_mapping::{
    char_to_evdev, evdev_button_from_code, evdev_key_to_macro_key, macro_button_to_evdev,
    macro_key_to_evdev,
};

static VIRTUAL_DEVICE: OnceLock<Mutex<VirtualDevice>> = OnceLock::new();
static CURSOR_X: AtomicI32 = AtomicI32::new(0);
static CURSOR_Y: AtomicI32 = AtomicI32::new(0);

fn syn_event() -> InputEvent {
    InputEvent::new(EventType::SYNCHRONIZATION.0, 0, 0)
}

fn get_or_init_virtual_device() -> Result<&'static Mutex<VirtualDevice>, io::Error> {
    if let Some(vd) = VIRTUAL_DEVICE.get() {
        return Ok(vd);
    }
    let vd = build_virtual_device()?;
    // Ignore error if another thread initialized it first.
    let _ = VIRTUAL_DEVICE.set(Mutex::new(vd));
    VIRTUAL_DEVICE.get().ok_or_else(|| io::Error::other("virtual device init race"))
}

fn build_virtual_device() -> io::Result<VirtualDevice> {
    let mut keys = AttributeSet::<KeyCode>::new();
    for k in [
        KeyCode::KEY_A, KeyCode::KEY_B, KeyCode::KEY_C, KeyCode::KEY_D, KeyCode::KEY_E,
        KeyCode::KEY_F, KeyCode::KEY_G, KeyCode::KEY_H, KeyCode::KEY_I, KeyCode::KEY_J,
        KeyCode::KEY_K, KeyCode::KEY_L, KeyCode::KEY_M, KeyCode::KEY_N, KeyCode::KEY_O,
        KeyCode::KEY_P, KeyCode::KEY_Q, KeyCode::KEY_R, KeyCode::KEY_S, KeyCode::KEY_T,
        KeyCode::KEY_U, KeyCode::KEY_V, KeyCode::KEY_W, KeyCode::KEY_X, KeyCode::KEY_Y,
        KeyCode::KEY_Z,
        KeyCode::KEY_1, KeyCode::KEY_2, KeyCode::KEY_3, KeyCode::KEY_4, KeyCode::KEY_5,
        KeyCode::KEY_6, KeyCode::KEY_7, KeyCode::KEY_8, KeyCode::KEY_9, KeyCode::KEY_0,
        KeyCode::KEY_MINUS, KeyCode::KEY_EQUAL, KeyCode::KEY_LEFTBRACE, KeyCode::KEY_RIGHTBRACE,
        KeyCode::KEY_BACKSLASH, KeyCode::KEY_SEMICOLON, KeyCode::KEY_APOSTROPHE, KeyCode::KEY_GRAVE,
        KeyCode::KEY_COMMA, KeyCode::KEY_DOT, KeyCode::KEY_SLASH,
        KeyCode::KEY_ENTER, KeyCode::KEY_BACKSPACE, KeyCode::KEY_TAB, KeyCode::KEY_SPACE,
        KeyCode::KEY_ESC, KeyCode::KEY_DELETE, KeyCode::KEY_INSERT,
        KeyCode::KEY_HOME, KeyCode::KEY_END, KeyCode::KEY_PAGEUP, KeyCode::KEY_PAGEDOWN,
        KeyCode::KEY_UP, KeyCode::KEY_DOWN, KeyCode::KEY_LEFT, KeyCode::KEY_RIGHT,
        KeyCode::KEY_LEFTSHIFT, KeyCode::KEY_RIGHTSHIFT,
        KeyCode::KEY_LEFTCTRL, KeyCode::KEY_RIGHTCTRL,
        KeyCode::KEY_LEFTALT, KeyCode::KEY_RIGHTALT,
        KeyCode::KEY_LEFTMETA, KeyCode::KEY_RIGHTMETA,
        KeyCode::KEY_CAPSLOCK, KeyCode::KEY_NUMLOCK, KeyCode::KEY_SCROLLLOCK,
        KeyCode::KEY_PAUSE, KeyCode::KEY_SYSRQ,
        KeyCode::KEY_F1, KeyCode::KEY_F2, KeyCode::KEY_F3, KeyCode::KEY_F4,
        KeyCode::KEY_F5, KeyCode::KEY_F6, KeyCode::KEY_F7, KeyCode::KEY_F8,
        KeyCode::KEY_F9, KeyCode::KEY_F10, KeyCode::KEY_F11, KeyCode::KEY_F12,
        KeyCode::KEY_F13, KeyCode::KEY_F14, KeyCode::KEY_F15, KeyCode::KEY_F16,
        KeyCode::KEY_F17, KeyCode::KEY_F18, KeyCode::KEY_F19, KeyCode::KEY_F20,
        KeyCode::KEY_F21, KeyCode::KEY_F22, KeyCode::KEY_F23, KeyCode::KEY_F24,
        KeyCode::KEY_KP0, KeyCode::KEY_KP1, KeyCode::KEY_KP2, KeyCode::KEY_KP3,
        KeyCode::KEY_KP4, KeyCode::KEY_KP5, KeyCode::KEY_KP6, KeyCode::KEY_KP7,
        KeyCode::KEY_KP8, KeyCode::KEY_KP9,
        KeyCode::KEY_KPPLUS, KeyCode::KEY_KPMINUS, KeyCode::KEY_KPASTERISK,
        KeyCode::KEY_KPSLASH, KeyCode::KEY_KPDOT, KeyCode::KEY_KPENTER,
        KeyCode::KEY_VOLUMEDOWN, KeyCode::KEY_MUTE, KeyCode::KEY_VOLUMEUP,
        KeyCode::KEY_SELECT,
        KeyCode::BTN_LEFT, KeyCode::BTN_RIGHT, KeyCode::BTN_MIDDLE,
        KeyCode::BTN_BACK, KeyCode::BTN_FORWARD,
    ] {
        keys.insert(k);
    }

    let mut rel_axes = AttributeSet::<RelativeAxisCode>::new();
    for ax in [
        RelativeAxisCode::REL_X,
        RelativeAxisCode::REL_Y,
        RelativeAxisCode::REL_WHEEL,
        RelativeAxisCode::REL_HWHEEL,
    ] {
        rel_axes.insert(ax);
    }

    VirtualDeviceBuilder::new()?
        .name("macros-input")
        .with_keys(&keys)?
        .with_relative_axes(&rel_axes)?
        .build()
}

// ── Emission ──────────────────────────────────────────────────────────────────

fn emit_key(key: KeyCode, pressed: bool) -> Result<(), String> {
    let vd = get_or_init_virtual_device().map_err(|e| e.to_string())?;
    let mut vd = vd.lock().unwrap();
    vd.emit(&[
        InputEvent::new(EventType::KEY.0, key.0, if pressed { 1 } else { 0 }),
        syn_event(),
    ])
    .map_err(|e| e.to_string())
}

fn emit_key_click(key: KeyCode, needs_shift: bool) -> Result<(), String> {
    let vd = get_or_init_virtual_device().map_err(|e| e.to_string())?;
    let mut vd = vd.lock().unwrap();
    let mut events: Vec<InputEvent> = Vec::with_capacity(6);
    if needs_shift {
        events.push(InputEvent::new(EventType::KEY.0, KeyCode::KEY_LEFTSHIFT.0, 1));
    }
    events.push(InputEvent::new(EventType::KEY.0, key.0, 1));
    events.push(InputEvent::new(EventType::KEY.0, key.0, 0));
    if needs_shift {
        events.push(InputEvent::new(EventType::KEY.0, KeyCode::KEY_LEFTSHIFT.0, 0));
    }
    events.push(syn_event());
    vd.emit(&events).map_err(|e| e.to_string())
}

pub struct EvdevBackend;

impl EvdevBackend {
    pub fn new() -> Result<Self, io::Error> {
        get_or_init_virtual_device()?;
        Ok(Self)
    }
}

impl InputBackend for EvdevBackend {
    fn key(&mut self, key: MacroKey, dir: Direction) -> Result<(), String> {
        let (evdev_key, needs_shift) =
            macro_key_to_evdev(&key).ok_or_else(|| format!("no evdev mapping for {:?}", key))?;
        match dir {
            Direction::Press => {
                if needs_shift {
                    emit_key(KeyCode::KEY_LEFTSHIFT, true)?;
                }
                emit_key(evdev_key, true)
            }
            Direction::Release => {
                let r = emit_key(evdev_key, false);
                if needs_shift {
                    emit_key(KeyCode::KEY_LEFTSHIFT, false)?;
                }
                r
            }
            Direction::Click => emit_key_click(evdev_key, needs_shift),
        }
    }

    fn raw_keycode(&mut self, keycode: u16, dir: Direction) -> Result<(), String> {
        let key = KeyCode(keycode);
        match dir {
            Direction::Press => emit_key(key, true),
            Direction::Release => emit_key(key, false),
            Direction::Click => emit_key_click(key, false),
        }
    }

    fn button(&mut self, button: MacroButton, dir: Direction) -> Result<(), String> {
        // Scroll variants become REL_WHEEL / REL_HWHEEL
        let (rel_axis, amount): (Option<RelativeAxisCode>, i32) = match button {
            MacroButton::ScrollUp => (Some(RelativeAxisCode::REL_WHEEL), 1),
            MacroButton::ScrollDown => (Some(RelativeAxisCode::REL_WHEEL), -1),
            MacroButton::ScrollLeft => (Some(RelativeAxisCode::REL_HWHEEL), -1),
            MacroButton::ScrollRight => (Some(RelativeAxisCode::REL_HWHEEL), 1),
            _ => (None, 0),
        };
        if let Some(axis) = rel_axis {
            let vd = get_or_init_virtual_device().map_err(|e| e.to_string())?;
            let mut vd = vd.lock().unwrap();
            return vd
                .emit(&[InputEvent::new(EventType::RELATIVE.0, axis.0, amount), syn_event()])
                .map_err(|e| e.to_string());
        }

        let evdev_btn = macro_button_to_evdev(&button)
            .ok_or_else(|| format!("no evdev mapping for {:?}", button))?;
        match dir {
            Direction::Press => emit_key(evdev_btn, true),
            Direction::Release => emit_key(evdev_btn, false),
            Direction::Click => emit_key_click(evdev_btn, false),
        }
    }

    fn move_mouse_rel(&mut self, dx: i32, dy: i32) -> Result<(), String> {
        CURSOR_X.fetch_add(dx, Ordering::Relaxed);
        CURSOR_Y.fetch_add(dy, Ordering::Relaxed);
        let vd = get_or_init_virtual_device().map_err(|e| e.to_string())?;
        let mut vd = vd.lock().unwrap();
        vd.emit(&[
            InputEvent::new(EventType::RELATIVE.0, RelativeAxisCode::REL_X.0, dx),
            InputEvent::new(EventType::RELATIVE.0, RelativeAxisCode::REL_Y.0, dy),
            syn_event(),
        ])
        .map_err(|e| e.to_string())
    }

    fn move_mouse_abs(&mut self, x: i32, y: i32) -> Result<(), String> {
        // uinput has no absolute-positioning axis for a virtual mouse, so
        // this has to land on an exact pixel by computing a relative delta
        // from the real current position (re-queried fresh, not the
        // free-running CURSOR_X/Y estimate - see `cursor_pos`) and emitting
        // that.
        let (cur_x, cur_y) = self.cursor_pos().unwrap_or((0, 0));
        let dx = x - cur_x;
        let dy = y - cur_y;
        self.move_mouse_rel(dx, dy)
    }

    fn scroll(&mut self, amount: i32, axis: Axis) -> Result<(), String> {
        let (rel, val) = match axis {
            Axis::Vertical => (RelativeAxisCode::REL_WHEEL, amount),
            Axis::Horizontal => (RelativeAxisCode::REL_HWHEEL, amount),
        };
        let vd = get_or_init_virtual_device().map_err(|e| e.to_string())?;
        let mut vd = vd.lock().unwrap();
        vd.emit(&[InputEvent::new(EventType::RELATIVE.0, rel.0, val), syn_event()])
            .map_err(|e| e.to_string())
    }

    fn text(&mut self, s: &str) -> Result<(), String> {
        // Unlike Windows/macOS (which go through enigo's own text-injection
        // APIs), this backend synthesizes each character as a plain
        // press/release on the uinput virtual device. Emitted back-to-back
        // with no gap, X11/Wayland consumers - browsers especially - can
        // coalesce or drop keystrokes that arrive faster than they can be
        // processed (the same reason tools like xdotool/ydotool default to
        // a non-zero per-character delay). A small delay between characters
        // keeps typing reliable without being noticeably slow.
        const INTER_CHAR_DELAY: std::time::Duration = std::time::Duration::from_micros(500);
        let mut chars = s.chars().peekable();
        while let Some(c) = chars.next() {
            match char_to_evdev(c) {
                Some((key, needs_shift)) => emit_key_click(key, needs_shift)?,
                None => warn!("no evdev mapping for char {:?}, skipping", c),
            }
            if chars.peek().is_some() {
                std::thread::sleep(INTER_CHAR_DELAY);
            }
        }
        Ok(())
    }

    fn cursor_pos(&self) -> Option<(i32, i32)> {
        // Prefer the real, compositor-tracked position (via XWayland) when
        // reachable - see `x11_cursor`'s doc comment for why the free-running
        // CURSOR_X/Y sum can't be trusted as ground truth. Re-anchor that sum
        // to what we just learned so it stays a reasonable estimate for the
        // (rarer) case a later call has no X11 to query.
        if let Some(pos) = super::x11_cursor::query_cursor_pos() {
            CURSOR_X.store(pos.0, Ordering::Relaxed);
            CURSOR_Y.store(pos.1, Ordering::Relaxed);
            return Some(pos);
        }
        Some((
            CURSOR_X.load(Ordering::Relaxed),
            CURSOR_Y.load(Ordering::Relaxed),
        ))
    }
}

// ── Capture ───────────────────────────────────────────────────────────────────

enum DeviceMsg {
    KeyPress { key: KeyCode, raw: InputEvent },
    KeyRelease { key: KeyCode, raw: InputEvent },
    ButtonPress { key: KeyCode, raw: InputEvent },
    ButtonRelease { key: KeyCode, raw: InputEvent },
    MouseMove { dx: i32, dy: i32, raw_x: Option<InputEvent>, raw_y: Option<InputEvent> },
    Scroll { v: i32, h: i32, raw_v: Option<InputEvent>, raw_h: Option<InputEvent> },
    OtherBatch(Vec<InputEvent>),
    /// Single-finger drag distance on a touchpad, read non-exclusively (see
    /// `spawn_touchpad_reader`) - there's no raw event to suppress or
    /// re-emit here, the physical device was never grabbed.
    TouchpadMove { dx: i32, dy: i32, ts: std::time::SystemTime },
    TouchpadButtonPress { ts: std::time::SystemTime },
    TouchpadButtonRelease { ts: std::time::SystemTime },
}

pub(super) fn start_capture_thread(
    mut callback: Box<dyn FnMut(CaptureEvent, CaptureTimestamp) -> CaptureDecision + Send + 'static>,
) {
    static STARTED: OnceLock<()> = OnceLock::new();
    STARTED.get_or_init(|| {
        let vd = match get_or_init_virtual_device() {
            Ok(vd) => vd,
            Err(e) => {
                warn!("Cannot start evdev capture (no virtual device): {}", e);
                crate::recording::set_grab_failed(true);
                return;
            }
        };

        let (tx, rx) = std::sync::mpsc::sync_channel::<DeviceMsg>(256);

        // Touchpads/tablets (ABS_X without REL_X, or the BUTTONPAD property)
        // are never exclusively grabbed: our virtual device only declares
        // relative axes, so we have nothing capable of re-emitting a
        // touchpad's real ABS/multitouch protocol, and grabbing without
        // re-emitting it would just make the touchpad stop working
        // entirely. Instead they're read non-exclusively below, purely to
        // observe single-finger drags and clicks for recording/hotkeys -
        // the same non-exclusive approach the Wine bridge already uses for
        // its own reasons (see its module doc comment).
        let is_touchpad = |device: &evdev::Device| -> bool {
            let is_buttonpad = device.properties().contains(PropType::BUTTONPAD);
            let has_abs = device
                .supported_absolute_axes()
                .map(|s| s.contains(AbsoluteAxisCode::ABS_X))
                .unwrap_or(false);
            let has_rel = device
                .supported_relative_axes()
                .map(|s| s.contains(RelativeAxisCode::REL_X))
                .unwrap_or(false);
            is_buttonpad || (has_abs && !has_rel)
        };

        // Enumerate and grab exclusive (keyboard/mouse-like) devices.
        let grabbed: Vec<_> = evdev::enumerate()
            .filter_map(|(path, mut device)| {
                let name = device.name().unwrap_or("").to_owned();
                if name == "macros-input" {
                    return None;
                }
                if is_touchpad(&device) {
                    return None;
                }
                let has_keys = device
                    .supported_keys()
                    .map(|s| s.contains(KeyCode::KEY_A) || s.contains(KeyCode::BTN_LEFT))
                    .unwrap_or(false);
                let has_rel = device
                    .supported_relative_axes()
                    .map(|s| s.contains(RelativeAxisCode::REL_X))
                    .unwrap_or(false);
                if !has_keys && !has_rel {
                    tracing::info!("evdev: skipping {:?} ({}) - no keys or relative motion", path, name);
                    return None;
                }
                match device.grab() {
                    Ok(()) => {
                        tracing::info!("evdev: grabbed {:?} ({}) - keys={} rel={}", path, name, has_keys, has_rel);
                        Some(device)
                    }
                    Err(e) => {
                        warn!("Failed to grab {:?} ({}): {}", path, name, e);
                        None
                    }
                }
            })
            .collect();

        // Enumerate touchpads separately, opened non-exclusively.
        let touchpads: Vec<_> = evdev::enumerate()
            .filter_map(|(path, device)| {
                let name = device.name().unwrap_or("").to_owned();
                if name == "macros-input" || !is_touchpad(&device) {
                    return None;
                }
                tracing::info!("evdev: reading {:?} ({}) non-exclusively as a touchpad", path, name);
                Some(device)
            })
            .collect();

        if grabbed.is_empty() && touchpads.is_empty() {
            warn!("No input devices found; global hotkeys and recording unavailable.");
            crate::recording::set_grab_failed(true);
        } else if grabbed.is_empty() {
            warn!("No devices could be exclusively grabbed (only touchpads found); global hotkeys unavailable, but touchpad recording still works.");
            crate::recording::set_grab_failed(true);
        } else {
            crate::recording::set_grab_failed(false);
        }

        for device in touchpads {
            spawn_touchpad_reader(device, tx.clone());
        }

        // Spawn one reader thread per grabbed device.
        for mut device in grabbed {
            let tx = tx.clone();
            std::thread::Builder::new()
                .name("evdev-reader".into())
                .spawn(move || {
                    crate::macros::priority::raise_current_thread_priority();
                    let mut pending_dx = 0i32;
                    let mut pending_dy = 0i32;
                    let mut pending_wv = 0i32;
                    let mut pending_wh = 0i32;
                    let mut raw_x: Option<InputEvent> = None;
                    let mut raw_y: Option<InputEvent> = None;
                    let mut raw_wv: Option<InputEvent> = None;
                    let mut raw_wh: Option<InputEvent> = None;
                    let mut other_batch: Vec<InputEvent> = Vec::new();

                    loop {
                        let events = match device.fetch_events() {
                            Ok(e) => e,
                            Err(e) => {
                                warn!("evdev read error: {}", e);
                                break;
                            }
                        };

                        for event in events {
                            match event.event_type() {
                                EventType::KEY => {
                                    if !other_batch.is_empty() {
                                        let _ = tx.send(DeviceMsg::OtherBatch(
                                            std::mem::take(&mut other_batch),
                                        ));
                                    }
                                    let key = KeyCode(event.code());
                                    let value = event.value();
                                    if value == 2 {
                                        // Autorepeat: pass through silently
                                        let _ = tx.send(DeviceMsg::OtherBatch(vec![event]));
                                        continue;
                                    }
                                    let is_btn = key.0 >= 0x100; // BTN_MISC = 0x100
                                    let pressed = value == 1;
                                    let msg = if is_btn {
                                        if pressed {
                                            DeviceMsg::ButtonPress { key, raw: event }
                                        } else {
                                            DeviceMsg::ButtonRelease { key, raw: event }
                                        }
                                    } else if pressed {
                                        DeviceMsg::KeyPress { key, raw: event }
                                    } else {
                                        DeviceMsg::KeyRelease { key, raw: event }
                                    };
                                    let _ = tx.send(msg);
                                }
                                EventType::RELATIVE => {
                                    match RelativeAxisCode(event.code()) {
                                        RelativeAxisCode::REL_X => {
                                            pending_dx += event.value();
                                            raw_x = Some(event);
                                        }
                                        RelativeAxisCode::REL_Y => {
                                            pending_dy += event.value();
                                            raw_y = Some(event);
                                        }
                                        RelativeAxisCode::REL_WHEEL => {
                                            pending_wv += event.value();
                                            raw_wv = Some(event);
                                        }
                                        RelativeAxisCode::REL_HWHEEL => {
                                            pending_wh += event.value();
                                            raw_wh = Some(event);
                                        }
                                        _ => other_batch.push(event),
                                    }
                                }
                                EventType::SYNCHRONIZATION => {
                                    if !other_batch.is_empty() {
                                        let _ = tx.send(DeviceMsg::OtherBatch(
                                            std::mem::take(&mut other_batch),
                                        ));
                                    }
                                    if pending_dx != 0 || pending_dy != 0 {
                                        let _ = tx.send(DeviceMsg::MouseMove {
                                            dx: std::mem::replace(&mut pending_dx, 0),
                                            dy: std::mem::replace(&mut pending_dy, 0),
                                            raw_x: raw_x.take(),
                                            raw_y: raw_y.take(),
                                        });
                                    }
                                    if pending_wv != 0 || pending_wh != 0 {
                                        let _ = tx.send(DeviceMsg::Scroll {
                                            v: std::mem::replace(&mut pending_wv, 0),
                                            h: std::mem::replace(&mut pending_wh, 0),
                                            raw_v: raw_wv.take(),
                                            raw_h: raw_wh.take(),
                                        });
                                    }
                                }
                                _ => other_batch.push(event),
                            }
                        }
                    }
                })
                .ok();
        }

        // Dispatch thread: single-threaded callback + re-emission.
        std::thread::Builder::new()
            .name("evdev-dispatch".into())
            .spawn(move || {
                crate::macros::priority::raise_current_thread_priority();
                let mut suppressed_keys: HashSet<u16> = HashSet::new();

                while let Ok(msg) = rx.recv() {
                    match msg {
                        DeviceMsg::KeyPress { key, raw } => {
                            let macro_key = match evdev_key_to_macro_key(key) {
                                Some(k) => k,
                                None => {
                                    reemit(vd, &[raw]);
                                    continue;
                                }
                            };
                            match callback(CaptureEvent::KeyPress(macro_key), CaptureTimestamp::Hardware(raw.timestamp())) {
                                CaptureDecision::Passthrough => {
                                    suppressed_keys.remove(&key.0);
                                    reemit(vd, &[raw]);
                                }
                                CaptureDecision::Suppress => {
                                    suppressed_keys.insert(key.0);
                                }
                            }
                        }
                        DeviceMsg::KeyRelease { key, raw } => {
                            let macro_key = match evdev_key_to_macro_key(key) {
                                Some(k) => k,
                                None => {
                                    reemit(vd, &[raw]);
                                    continue;
                                }
                            };
                            let was_suppressed = suppressed_keys.remove(&key.0);
                            match callback(CaptureEvent::KeyRelease(macro_key), CaptureTimestamp::Hardware(raw.timestamp())) {
                                CaptureDecision::Passthrough if !was_suppressed => {
                                    reemit(vd, &[raw]);
                                }
                                _ => {}
                            }
                        }
                        DeviceMsg::ButtonPress { key, raw } => {
                            let btn = match evdev_button_from_code(key.0) {
                                Some(b) => b,
                                None => {
                                    reemit(vd, &[raw]);
                                    continue;
                                }
                            };
                            match callback(CaptureEvent::ButtonPress(btn), CaptureTimestamp::Hardware(raw.timestamp())) {
                                CaptureDecision::Passthrough => reemit(vd, &[raw]),
                                CaptureDecision::Suppress => {}
                            }
                        }
                        DeviceMsg::ButtonRelease { key, raw } => {
                            let btn = match evdev_button_from_code(key.0) {
                                Some(b) => b,
                                None => {
                                    reemit(vd, &[raw]);
                                    continue;
                                }
                            };
                            match callback(CaptureEvent::ButtonRelease(btn), CaptureTimestamp::Hardware(raw.timestamp())) {
                                CaptureDecision::Passthrough => reemit(vd, &[raw]),
                                CaptureDecision::Suppress => {}
                            }
                        }
                        DeviceMsg::MouseMove { dx, dy, raw_x, raw_y } => {
                            let ts = CaptureTimestamp::Hardware(
                                raw_x.as_ref().or(raw_y.as_ref())
                                    .map(|e| e.timestamp())
                                    .unwrap_or_else(std::time::SystemTime::now),
                            );
                            CURSOR_X.fetch_add(dx, Ordering::Relaxed);
                            CURSOR_Y.fetch_add(dy, Ordering::Relaxed);
                            match callback(CaptureEvent::MouseMoveRel(dx, dy), ts) {
                                CaptureDecision::Passthrough => {
                                    let mut events: Vec<InputEvent> = Vec::with_capacity(3);
                                    if let Some(e) = raw_x { events.push(e); }
                                    if let Some(e) = raw_y { events.push(e); }
                                    if !events.is_empty() {
                                        reemit(vd, &events);
                                    }
                                }
                                CaptureDecision::Suppress => {}
                            }
                        }
                        DeviceMsg::Scroll { v, h, raw_v, raw_h } => {
                            let ts = CaptureTimestamp::Hardware(
                                raw_v.as_ref().or(raw_h.as_ref())
                                    .map(|e| e.timestamp())
                                    .unwrap_or_else(std::time::SystemTime::now),
                            );
                            if v != 0 {
                                match callback(CaptureEvent::Scroll(0, v), ts) {
                                    CaptureDecision::Passthrough => {
                                        if let Some(e) = raw_v { reemit(vd, &[e]); }
                                    }
                                    CaptureDecision::Suppress => {}
                                }
                            }
                            if h != 0 {
                                match callback(CaptureEvent::Scroll(h, 0), ts) {
                                    CaptureDecision::Passthrough => {
                                        if let Some(e) = raw_h { reemit(vd, &[e]); }
                                    }
                                    CaptureDecision::Suppress => {}
                                }
                            }
                        }
                        DeviceMsg::OtherBatch(events) => {
                            reemit(vd, &events);
                        }
                        DeviceMsg::TouchpadMove { dx, dy, ts } => {
                            CURSOR_X.fetch_add(dx, Ordering::Relaxed);
                            CURSOR_Y.fetch_add(dy, Ordering::Relaxed);
                            // Never grabbed, so its real motion already
                            // reached the desktop untouched - nothing to
                            // suppress or re-emit, only observe.
                            let _ = callback(CaptureEvent::MouseMoveRel(dx, dy), CaptureTimestamp::Hardware(ts));
                        }
                        DeviceMsg::TouchpadButtonPress { ts } => {
                            let _ = callback(CaptureEvent::ButtonPress(MacroButton::Left), CaptureTimestamp::Hardware(ts));
                        }
                        DeviceMsg::TouchpadButtonRelease { ts } => {
                            let _ = callback(CaptureEvent::ButtonRelease(MacroButton::Left), CaptureTimestamp::Hardware(ts));
                        }
                    }
                }
            })
            .ok();
    });
}

/// Reads a touchpad non-exclusively (never `.grab()`ed - see the comment
/// where this is spawned) purely to observe single-finger drags and real
/// clicks for recording/hotkeys. Two-/three-finger gestures are ignored via
/// `BTN_TOOL_DOUBLETAP`/`BTN_TOOL_TRIPLETAP` so a scroll or swipe doesn't
/// get misread as a pointer drag.
fn spawn_touchpad_reader(mut device: evdev::Device, tx: std::sync::mpsc::SyncSender<DeviceMsg>) {
    std::thread::Builder::new()
        .name("evdev-touchpad-reader".into())
        .spawn(move || {
            crate::macros::priority::raise_current_thread_priority();
            let mut x: Option<i32> = None;
            let mut y: Option<i32> = None;
            let mut last_x: Option<i32> = None;
            let mut last_y: Option<i32> = None;
            let mut finger_down = false;
            let mut multi_finger = false;
            let mut btn_left_down = false;

            loop {
                let events = match device.fetch_events() {
                    Ok(e) => e,
                    Err(e) => {
                        warn!("evdev touchpad read error: {}", e);
                        break;
                    }
                };

                for event in events {
                    match event.event_type() {
                        EventType::ABSOLUTE => match AbsoluteAxisCode(event.code()) {
                            AbsoluteAxisCode::ABS_X => x = Some(event.value()),
                            AbsoluteAxisCode::ABS_Y => y = Some(event.value()),
                            _ => {}
                        },
                        EventType::KEY => match KeyCode(event.code()) {
                            KeyCode::BTN_TOOL_FINGER => {
                                finger_down = event.value() != 0;
                                if finger_down {
                                    // Fresh contact: no baseline yet, so the
                                    // next SYN just records a position
                                    // rather than a (likely huge, bogus)
                                    // delta from wherever the last touch let go.
                                    last_x = None;
                                    last_y = None;
                                }
                            }
                            KeyCode::BTN_TOOL_DOUBLETAP | KeyCode::BTN_TOOL_TRIPLETAP => {
                                multi_finger = event.value() != 0;
                            }
                            KeyCode::BTN_LEFT => {
                                let pressed = event.value() != 0;
                                if pressed != btn_left_down {
                                    btn_left_down = pressed;
                                    let ts = event.timestamp();
                                    let msg = if pressed {
                                        DeviceMsg::TouchpadButtonPress { ts }
                                    } else {
                                        DeviceMsg::TouchpadButtonRelease { ts }
                                    };
                                    let _ = tx.send(msg);
                                }
                            }
                            _ => {}
                        },
                        EventType::SYNCHRONIZATION => {
                            if finger_down && !multi_finger {
                                if let (Some(cx), Some(cy)) = (x, y) {
                                    if let (Some(lx), Some(ly)) = (last_x, last_y) {
                                        let (dx, dy) = (cx - lx, cy - ly);
                                        if dx != 0 || dy != 0 {
                                            let _ = tx.send(DeviceMsg::TouchpadMove { dx, dy, ts: event.timestamp() });
                                        }
                                    }
                                    last_x = Some(cx);
                                    last_y = Some(cy);
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        })
        .ok();
}

fn reemit(vd: &'static Mutex<VirtualDevice>, events: &[InputEvent]) {
    if events.is_empty() {
        return;
    }
    let mut all: Vec<InputEvent> = Vec::with_capacity(events.len() + 1);
    all.extend_from_slice(events);
    all.push(syn_event());
    if let Ok(mut vd) = vd.lock() {
        if let Err(e) = vd.emit(&all) {
            warn!("evdev re-emit error: {}", e);
        }
    }
}
