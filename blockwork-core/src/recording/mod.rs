use crate::hotkey_types::{HotkeyAction, HotkeyBinding};
use crate::input::types::{Axis, Coordinate, Direction, InputToken, MacroKey};
use crate::input::value::Value;
use crate::macros::backend::{self, CaptureDecision, CaptureEvent, CaptureTimestamp};
use crate::macros::{Instruction, InstructionKind};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Mutex, OnceLock, RwLock};
use std::time::{Duration, Instant, SystemTime};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

pub static RECORDING_ACTIVE: AtomicBool = AtomicBool::new(false);
/// Whether mouse movement is captured into the recording at all. Off by
/// default - most recordings care about clicks/keys, and movement floods
/// the instruction list with a token per pixel of travel.
pub static RECORD_MOUSE_MOVEMENT: AtomicBool = AtomicBool::new(false);
pub static RECORD_MOUSE_RELATIVE: AtomicBool = AtomicBool::new(false);
static GRAB_FAILED: AtomicBool = AtomicBool::new(false);

/// Modifier keys currently held physically; used to match hotkey combos.
/// Backends must not feed their own injected events into this, or a macro
/// holding Ctrl down would change which combo the next keypress matches.
static HELD_MODS: AtomicU8 = AtomicU8::new(0);

/// Clears tracked modifier state. Needed on Windows, where a modifier released
/// while a secure desktop (UAC, Ctrl+Alt+Del, lock screen) is in front never
/// reaches the hook, leaving a phantom bit that would block future combos.
// Only Windows has an event to hang this off; evdev holds devices open across
// the equivalent transitions and never loses key-ups.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn reset_held_mods() {
    HELD_MODS.store(0, Ordering::Relaxed);
}

pub fn grab_failed() -> bool {
    GRAB_FAILED.load(Ordering::Relaxed)
}

pub fn set_grab_failed(v: bool) {
    GRAB_FAILED.store(v, Ordering::Relaxed);
}

/// A signal pushed from the synchronous capture callback to the GUI's async
/// subscription, over an unbounded channel so the GUI reacts immediately.
pub enum QueueSignal {
    Hotkey(HotkeyAction),
    Stop,
}

static RECORDING_QUEUE: OnceLock<Mutex<VecDeque<Instruction>>> = OnceLock::new();
// Anchors for whichever `CaptureTimestamp` variant the backend produces
// (evdev: Hardware; Windows/macOS: Now), measuring against the backend's own clock.
static BASELINE_NOW: OnceLock<Mutex<Option<Instant>>> = OnceLock::new();
static BASELINE_HW: OnceLock<Mutex<Option<SystemTime>>> = OnceLock::new();
static LAST_ELAPSED: OnceLock<Mutex<Option<Duration>>> = OnceLock::new();
// Movement is sampled to retain a smooth path without replaying every raw
// pointer event.
static LAST_RECORDED_MOUSE_MOVE: OnceLock<Mutex<Option<Duration>>> = OnceLock::new();
static LAST_MOUSE_POS: OnceLock<Mutex<Option<(f64, f64)>>> = OnceLock::new();
// libei pauses after each pointer update while it services the portal. Do not
// duplicate that time in a recorded wait.
#[cfg(target_os = "linux")]
const MOUSE_PLAYBACK_OVERHEAD: Duration = Duration::from_millis(10);
#[cfg(not(target_os = "linux"))]
const MOUSE_PLAYBACK_OVERHEAD: Duration = Duration::ZERO;
// Offset added to hardware-timestamped elapsed so the first event reflects
// the gap since recording-start, instead of collapsing to zero.
static HW_ELAPSED_OFFSET: OnceLock<Mutex<Duration>> = OnceLock::new();

static HOTKEY_TABLE: OnceLock<RwLock<Vec<HotkeyBinding>>> = OnceLock::new();

/// Armed when a `StartRecordingImmediate` combo is pressed, so recording waits
/// until every key in the combo releases - otherwise those key-ups get recorded.
struct PendingRecordStart {
    mods_mask: u8,
    trigger_key: MacroKey,
    trigger_released: bool,
}
static PENDING_RECORD_START: OnceLock<Mutex<Option<PendingRecordStart>>> = OnceLock::new();

/// Called when a `StartRecordingImmediate` combo is pressed, from the hotkey
/// detection in `start_grab_thread`'s capture callback.
pub fn arm_pending_record_start(mods_mask: u8, trigger_key: MacroKey) {
    if let Ok(mut g) = PENDING_RECORD_START.get_or_init(|| Mutex::new(None)).lock() {
        *g = Some(PendingRecordStart { mods_mask, trigger_key, trigger_released: false });
    }
}

/// Called on every KeyRelease (all platforms) to check whether an armed
/// `StartRecordingImmediate` combo has now been fully released.
fn check_pending_record_start(key: &MacroKey, held_mods: u8) {
    let cell = PENDING_RECORD_START.get_or_init(|| Mutex::new(None));
    let Ok(mut guard) = cell.lock() else { return };
    let Some(pending) = guard.as_mut() else { return };
    if key == &pending.trigger_key {
        pending.trigger_released = true;
    }
    if pending.trigger_released && (held_mods & pending.mods_mask) == 0 {
        push_queue_signal(QueueSignal::Hotkey(HotkeyAction::StartRecordingImmediate));
        *guard = None;
    }
}

type QueueChannel = (UnboundedSender<QueueSignal>, Mutex<Option<UnboundedReceiver<QueueSignal>>>);

fn queue_channel() -> &'static QueueChannel {
    static CHANNEL: OnceLock<QueueChannel> = OnceLock::new();
    CHANNEL.get_or_init(|| {
        let (tx, rx) = mpsc::unbounded_channel();
        (tx, Mutex::new(Some(rx)))
    })
}

pub fn push_queue_signal(signal: QueueSignal) {
    let _ = queue_channel().0.send(signal);
}

/// Takes the receiver end of the queue channel. Must be called exactly once
/// (by the GUI subscription that owns dispatching these signals as messages).
pub fn take_queue_receiver() -> UnboundedReceiver<QueueSignal> {
    queue_channel()
        .1
        .lock()
        .ok()
        .and_then(|mut g| g.take())
        .expect("queue receiver already taken")
}

pub fn get_last_mouse_pos() -> Option<(f64, f64)> {
    LAST_MOUSE_POS.get_or_init(|| Mutex::new(None)).lock().ok().and_then(|g| *g)
}

pub fn set_last_mouse_pos(x: f64, y: f64) {
    if let Ok(mut g) = LAST_MOUSE_POS.get_or_init(|| Mutex::new(None)).lock() {
        *g = Some((x, y));
    }
}

pub fn get_recording_queue() -> &'static Mutex<VecDeque<Instruction>> {
    RECORDING_QUEUE.get_or_init(|| Mutex::new(VecDeque::new()))
}

pub fn update_hotkey_table(bindings: Vec<HotkeyBinding>) {
    // Matching happens in start_grab_thread's capture callback rather than via
    // OS-level hotkey registration, since StopRecording must only fire while recording.
    let table = HOTKEY_TABLE.get_or_init(|| RwLock::new(vec![]));
    if let Ok(mut t) = table.write() {
        *t = bindings;
    }
}

/// Anchors both clocks to the moment recording actually starts, rather than
/// lazily to the first event - so the gap before the first captured input
/// becomes a leading `Wait` instead of being dropped.
pub fn reset_timing() {
    if let Ok(mut t) = BASELINE_NOW.get_or_init(|| Mutex::new(None)).lock() {
        *t = Some(Instant::now());
    }
    if let Ok(mut t) = BASELINE_HW.get_or_init(|| Mutex::new(None)).lock() {
        // Reset to None so elapsed_since_session_start lazily sets the baseline from
        // the first Hardware event. Setting it directly with SystemTime::now() breaks
        // macOS, whose mach_absolute_time clock isn't comparable to Unix time.
        *t = None;
    }
    if let Ok(mut t) = LAST_ELAPSED.get_or_init(|| Mutex::new(None)).lock() {
        *t = Some(Duration::ZERO);
    }
    if let Ok(mut t) = LAST_RECORDED_MOUSE_MOVE.get_or_init(|| Mutex::new(None)).lock() {
        *t = None;
    }
    if let Ok(mut p) = LAST_MOUSE_POS.get_or_init(|| Mutex::new(None)).lock() {
        *p = None;
    }
    if let Ok(mut o) = HW_ELAPSED_OFFSET.get_or_init(|| Mutex::new(Duration::ZERO)).lock() {
        *o = Duration::ZERO;
    }
}

/// Elapsed time since the session's first event, on whichever clock `ts` uses.
/// Negative deltas (clock steps, or out-of-order evdev device messages) clamp
/// to zero rather than corrupting the recording.
fn elapsed_since_session_start(ts: CaptureTimestamp) -> Duration {
    match ts {
        CaptureTimestamp::Now => {
            let now = Instant::now();
            let baseline = BASELINE_NOW.get_or_init(|| Mutex::new(None));
            let mut baseline = match baseline.lock() {
                Ok(g) => g,
                Err(_) => return Duration::ZERO,
            };
            let start = *baseline.get_or_insert(now);
            now.saturating_duration_since(start)
        }
        CaptureTimestamp::Hardware(now) => {
            let baseline = BASELINE_HW.get_or_init(|| Mutex::new(None));
            let mut baseline = match baseline.lock() {
                Ok(g) => g,
                Err(_) => return Duration::ZERO,
            };
            if let Some(start) = *baseline {
                let raw = now.duration_since(start).unwrap_or(Duration::ZERO);
                if let Ok(off) = HW_ELAPSED_OFFSET.get_or_init(|| Mutex::new(Duration::ZERO)).lock() {
                    raw + *off
                } else {
                    raw
                }
            } else {
                // Lazily initialise the hardware baseline on first event, using the
                // pre-anchored Instant-based elapsed as the offset for that gap.
                *baseline = Some(now);
                drop(baseline);
                let instant_elapsed = elapsed_since_session_start(CaptureTimestamp::Now);
                if let Ok(mut off) = HW_ELAPSED_OFFSET.get_or_init(|| Mutex::new(Duration::ZERO)).lock() {
                    *off = instant_elapsed;
                }
                instant_elapsed
            }
        }
    }
}

fn check_hotkey(mods: u8, key_name: &str) -> Option<HotkeyAction> {
    let table = HOTKEY_TABLE.get_or_init(|| RwLock::new(vec![]));
    if let Ok(bindings) = table.try_read() {
        for binding in bindings.iter() {
            // StopRecording is handled directly in start_grab_thread's
            // RECORDING_ACTIVE branch; skip it here or its unmodified key would
            // be swallowed system-wide whenever the app is running.
            if matches!(binding.action, HotkeyAction::StopRecording) {
                continue;
            }
            if binding.combo.modifiers == mods && binding.combo.key == key_name {
                return Some(binding.action.clone());
            }
        }
    }
    None
}

/// Looks up the configured `StopRecording` binding's key name, if any. It's
/// always combo-less (enforced at capture time), so matching only compares the key.
fn stop_recording_key() -> Option<String> {
    let table = HOTKEY_TABLE.get_or_init(|| RwLock::new(vec![]));
    table.try_read().ok().and_then(|bindings| {
        bindings
            .iter()
            .find(|b| matches!(b.action, HotkeyAction::StopRecording))
            .map(|b| b.combo.key.clone())
    })
}

/// Builds the same capture callback `start_grab_thread()` installs on the
/// platform's native hook, for a caller that has its own event source
/// instead (e.g. the Wine bridge, which gets real input from a native
/// Linux helper process rather than from `WH_KEYBOARD_LL`/`WH_MOUSE_LL`,
/// which don't see real host input under Wine). Identical recording-queue
/// bookkeeping and cursor tracking either way - this is the one place that
/// logic lives.
pub fn build_capture_callback() -> Box<dyn FnMut(CaptureEvent, CaptureTimestamp) -> CaptureDecision + Send + 'static> {
    Box::new(move |event: CaptureEvent, ts: CaptureTimestamp| {
            match &event {
                CaptureEvent::KeyPress(key) => {
                    let bit = key.modifier_bit();
                    if bit != 0 {
                        HELD_MODS.fetch_or(bit, Ordering::Relaxed);
                    }
                }
                CaptureEvent::KeyRelease(key) => {
                    let bit = key.modifier_bit();
                    if bit != 0 {
                        HELD_MODS.fetch_and(!bit, Ordering::Relaxed);
                    }
                    check_pending_record_start(key, HELD_MODS.load(Ordering::Relaxed));
                }
                _ => {}
            }

            // Always track the cursor so absolute recording and relative playback
            // have a baseline. A real position source wins; integrating deltas is
            // the fallback for platforms that report none.
            let prev_pos = get_last_mouse_pos();
            let absolute_pos = if RECORDING_ACTIVE.load(Ordering::Relaxed)
                && !RECORD_MOUSE_RELATIVE.load(Ordering::Relaxed)
                && matches!(event, CaptureEvent::MouseMoveRel(_, _))
            {
                backend::absolute_mouse_position()
            } else {
                None
            };
            match &event {
                CaptureEvent::MouseMoveRel(dx, dy) => {
                    if let Some((x, y)) = absolute_pos {
                        set_last_mouse_pos(x as f64, y as f64);
                    } else if let Some((lx, ly)) = prev_pos {
                        set_last_mouse_pos(lx + *dx as f64, ly + *dy as f64);
                    }
                }
                CaptureEvent::MouseMoveAbs(x, y) => {
                    set_last_mouse_pos(*x, *y);
                }
                _ => {}
            }

            if RECORDING_ACTIVE.load(Ordering::Relaxed) {
                // The configured (combo-less) StopRecording key stops recording.
                // Escape has no special status - it's just the shipped default binding.
                if let CaptureEvent::KeyPress(key) = &event {
                    let is_stop_key = key
                        .hotkey_name()
                        .is_some_and(|name| stop_recording_key().as_deref() == Some(name.as_str()));
                    if is_stop_key {
                        RECORDING_ACTIVE.store(false, Ordering::Relaxed);
                        push_queue_signal(QueueSignal::Stop);
                        return CaptureDecision::Suppress;
                    }
                }

                let elapsed = elapsed_since_session_start(ts);
                let absolute_event =
                    absolute_pos.map(|(x, y)| CaptureEvent::MouseMoveAbs(x as f64, y as f64));
                let event_for_instruction = absolute_event.as_ref().unwrap_or(&event);
                let instr = capture_event_to_instruction(event_for_instruction, prev_pos);
                if let Some(instr) = instr {
                    let last_elapsed = LAST_ELAPSED.get_or_init(|| Mutex::new(None));
                    if let Ok(mut last) = last_elapsed.lock() {
                        let prev = *last;
                        *last = Some(elapsed);
                        if let Ok(mut q) = get_recording_queue().lock() {
                            let is_mouse_move = matches!(event_for_instruction, CaptureEvent::MouseMoveRel(_, _) | CaptureEvent::MouseMoveAbs(_, _));
                            if is_mouse_move {
                                const MOUSE_SAMPLE_INTERVAL: Duration = Duration::from_millis(16);
                                let last_mouse_move = LAST_RECORDED_MOUSE_MOVE.get_or_init(|| Mutex::new(None));
                                let elapsed_since_last = last_mouse_move
                                    .lock()
                                    .ok()
                                    .and_then(|last| last.map(|last| elapsed.saturating_sub(last)));
                                let append = elapsed_since_last
                                    .map_or(true, |delta| delta >= MOUSE_SAMPLE_INTERVAL)
                                    || !matches!(
                                        q.back(),
                                        Some(last)
                                            if matches!(last.kind, InstructionKind::Token(InputToken::MoveMouse(_, _, _)))
                                    );
                                if append {
                                    let previous_was_mouse = matches!(
                                        q.back(),
                                        Some(last)
                                            if matches!(last.kind, InstructionKind::Token(InputToken::MoveMouse(_, _, _)))
                                    );
                                    let wait = if previous_was_mouse {
                                        elapsed_since_last
                                    } else {
                                        prev.map(|last| elapsed.saturating_sub(last))
                                    };
                                    if let Some(wait) = wait {
                                        let wait = wait.saturating_sub(MOUSE_PLAYBACK_OVERHEAD);
                                        if !wait.is_zero() {
                                            q.push_back(Instruction::new(InstructionKind::Wait(Value::number(
                                                wait.as_secs_f64() * 1000.0,
                                            ))));
                                        }
                                    }
                                    q.push_back(instr);
                                    if let Ok(mut last) = last_mouse_move.lock() {
                                        *last = Some(elapsed);
                                    }
                                } else if let Some(last) = q.back_mut().filter(|last| {
                                    matches!(
                                        last.kind,
                                        InstructionKind::Token(InputToken::MoveMouse(_, _, _))
                                    )
                                }) {
                                    *last = instr;
                                }
                            } else {
                                if let Some(prev_elapsed) = prev {
                                    let elapsed_ms = elapsed.saturating_sub(prev_elapsed).as_secs_f64() * 1000.0;
                                    if elapsed_ms > 0.0 {
                                        q.push_back(Instruction::new(InstructionKind::Wait(Value::number(elapsed_ms))));
                                    }
                                }
                                q.push_back(instr);
                            }
                        }
                    }
                }

                return CaptureDecision::Passthrough;
            }

            // Hotkey detection (only when not recording), against the physical
            // keys the backend reports - no OS-level hotkey registration involved.
            if let CaptureEvent::KeyPress(key) = &event {
                if !key.is_modifier() {
                    if let Some(name) = key.hotkey_name() {
                        let mods = HELD_MODS.load(Ordering::Relaxed);
                        if let Some(action) = check_hotkey(mods, &name) {
                            backend::note_hotkey_matched();
                            if matches!(action, HotkeyAction::StartRecordingImmediate) {
                                arm_pending_record_start(mods, key.clone());
                            } else {
                                push_queue_signal(QueueSignal::Hotkey(action));
                            }
                            // Swallow the trigger so the combo doesn't also reach
                            // whatever has focus; backends suppress the key-up too.
                            return CaptureDecision::Suppress;
                        }
                    }
                }
            }

            CaptureDecision::Passthrough
    })
}

pub fn start_grab_thread() {
    static STARTED: OnceLock<()> = OnceLock::new();
    STARTED.get_or_init(|| {
        backend::start_capture(build_capture_callback());
    });
}

/// `prev_pos` is the tracked absolute cursor position from just before this
/// event was applied (see `build_capture_callback`) - used to convert
/// between relative deltas and absolute coordinates, whichever the event
/// itself isn't already expressed in, based on `RECORD_MOUSE_RELATIVE`.
/// Mouse movement is dropped entirely unless `RECORD_MOUSE_MOVEMENT` is set.
fn capture_event_to_instruction(event: &CaptureEvent, prev_pos: Option<(f64, f64)>) -> Option<Instruction> {
    Some(Instruction::new(match event {
        CaptureEvent::KeyPress(key) => {
            InstructionKind::Token(InputToken::Key(key.clone(), Direction::Press))
        }
        CaptureEvent::KeyRelease(key) => {
            InstructionKind::Token(InputToken::Key(key.clone(), Direction::Release))
        }
        CaptureEvent::ButtonPress(btn) => {
            InstructionKind::Token(InputToken::Button(btn.clone(), Direction::Press))
        }
        CaptureEvent::ButtonRelease(btn) => {
            InstructionKind::Token(InputToken::Button(btn.clone(), Direction::Release))
        }
        CaptureEvent::Scroll(h, v) => {
            if *v != 0 {
                InstructionKind::Token(InputToken::Scroll(Value::number(*v as f64), Axis::Vertical))
            } else if *h != 0 {
                InstructionKind::Token(InputToken::Scroll(Value::number(*h as f64), Axis::Horizontal))
            } else {
                return None;
            }
        }
        CaptureEvent::MouseMoveRel(dx, dy) => {
            if !RECORD_MOUSE_MOVEMENT.load(Ordering::Relaxed) {
                return None;
            }
            if *dx == 0 && *dy == 0 {
                return None;
            }
            if RECORD_MOUSE_RELATIVE.load(Ordering::Relaxed) {
                InstructionKind::Token(InputToken::MoveMouse(Value::number(*dx as f64), Value::number(*dy as f64), Coordinate::Rel))
            } else {
                // Absolute mode: no backend emits MouseMoveAbs directly (see
                // `CaptureEvent` doc comment), so absolute recording is built
                // by walking this delta against the tracked cursor position.
                // Without a seeded baseline yet, there's nothing to add the
                // delta onto - drop rather than record a bogus position.
                let (x, y) = prev_pos?;
                InstructionKind::Token(InputToken::MoveMouse(Value::number(x + *dx as f64), Value::number(y + *dy as f64), Coordinate::Abs))
            }
        }
        CaptureEvent::MouseMoveAbs(x, y) => {
            if !RECORD_MOUSE_MOVEMENT.load(Ordering::Relaxed) {
                return None;
            }
            if RECORD_MOUSE_RELATIVE.load(Ordering::Relaxed) {
                let (px, py) = prev_pos?;
                let (dx, dy) = (*x - px, *y - py);
                if dx == 0.0 && dy == 0.0 {
                    return None;
                }
                InstructionKind::Token(InputToken::MoveMouse(Value::number(dx), Value::number(dy), Coordinate::Rel))
            } else {
                InstructionKind::Token(InputToken::MoveMouse(Value::number(*x), Value::number(*y), Coordinate::Abs))
            }
        }
    }))
}
