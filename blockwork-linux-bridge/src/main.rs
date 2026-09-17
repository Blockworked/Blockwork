//! Native Linux helper for a Windows host embedding blockwork-ffi under Proton.
//!
//! `WH_KEYBOARD_LL`/`WH_MOUSE_LL` don't see real host input under Wine, and
//! `SendInput`-based emission is unreliable there too. This process is
//! launched by `blockwork-ffi` (see its Wine-detection code) and bridges
//! real input across a shared-memory region at the path given as `argv[1]`:
//!
//! - capture: reads `/dev/input/event*` directly (non-exclusive — never
//!   grabs devices) and pushes `wire::WireCapture` messages into the
//!   capture ring.
//! - control/run: pops `wire::WireControlCommand`s off the control ring.
//!   `RunMacro(id)` loads that macro straight from the OS-default config
//!   dir (the same real directory the Windows side's `macros-dir` override
//!   setting reaches via its `Z:` mapping) and runs it natively, right
//!   here. This is why this process exists rather than the Windows side
//!   running the macro and shipping individual input events across:
//!   `raise_current_thread_priority()` gets real `SCHED_FIFO` here, versus
//!   Wine's much weaker emulation of `SetThreadPriority` inside the
//!   Wine-hosted embedder.
//!
//!   Emission uses `EvdevBackend` (`uinput`), the same backend the native
//!   Linux desktop app uses — not the X Test extension, which goes nowhere
//!   under `winewayland.drv` (a native Wayland client the system's Xwayland
//!   instance was never a client of).
//!
//! Exits if the Windows side's heartbeat goes stale (host closed/crashed),
//! so this never orphans itself.

use evdev::{AbsoluteAxisCode, EventType, KeyCode, PropType, RelativeAxisCode};
use blockwork_core::config;
use blockwork_core::macros::backend::evdev::EvdevBackend;
use blockwork_core::macros::backend::evdev_mapping::{evdev_button_from_code, evdev_key_to_macro_key};
use blockwork_core::macros::backend::InputBackend;
use blockwork_core::macros::priority::raise_current_thread_priority;
use blockwork_core::macros::runner::VariableStore;
use blockwork_core::macros::run_registry;
use blockwork_core::wire::{
    self, SharedRegion, WireCapture, WireCaptureEvent, WireControlCommand, WireTimestamp,
};
use std::fs::OpenOptions;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const HEARTBEAT_TIMEOUT: Duration = Duration::from_secs(5);
const HEARTBEAT_POLL: Duration = Duration::from_millis(200);
/// How often `control_loop` checks the (otherwise empty) control ring when
/// idle. Was 1ms; shrunk now that the thread runs `SCHED_FIFO` (see
/// `control_loop`'s docs) — cheap to poll this often at real-time priority,
/// and it directly bounds worst-case command-notice latency.
const CONTROL_POLL: Duration = Duration::from_micros(200);

/// This process's stderr isn't inherited (the Windows-side launcher creates
/// it with `bInheritHandles = FALSE`), so writing there goes nowhere
/// observable. A file next to the shm path is a location known reachable
/// from wherever this process ends up running.
#[derive(Clone)]
struct FileLogWriter(Arc<Mutex<std::fs::File>>);

impl std::io::Write for FileLogWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().write(buf)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.0.lock().unwrap().flush()
    }
}

fn main() {
    let shm_path = match std::env::args().nth(1) {
        Some(p) => p,
        None => {
            eprintln!("usage: blockwork-linux-bridge <shm-path>");
            std::process::exit(1);
        }
    };

    let log_path = format!("{shm_path}.log");
    match OpenOptions::new().create(true).append(true).open(&log_path) {
        Ok(f) => {
            let writer = FileLogWriter(Arc::new(Mutex::new(f)));
            tracing_subscriber::fmt().with_writer(move || writer.clone()).with_ansi(false).init();
        }
        Err(_) => {
            // Last resort if the log file can't be created, even though
            // nothing may be watching stderr.
            tracing_subscriber::fmt().with_writer(std::io::stderr).init();
        }
    }
    tracing::info!("log file: {log_path}");

    let file = match OpenOptions::new().read(true).write(true).open(&shm_path) {
        Ok(f) => f,
        Err(e) => {
            tracing::error!("failed to open shared memory file '{shm_path}': {e}");
            std::process::exit(1);
        }
    };

    // SAFETY: the Windows side owns this file's lifetime for as long as
    // this process runs (a stale tmpfs file left after abnormal
    // termination is harmless). Sized to `SHARED_REGION_SIZE` by the
    // creator before this process was launched.
    let mmap = match unsafe { memmap2::MmapMut::map_mut(&file) } {
        Ok(m) => m,
        Err(e) => {
            tracing::error!("failed to mmap shared memory: {e}");
            std::process::exit(1);
        }
    };
    if mmap.len() < wire::SHARED_REGION_SIZE {
        tracing::error!("shared memory file too small: {} < {}", mmap.len(), wire::SHARED_REGION_SIZE);
        std::process::exit(1);
    }

    // Leak the mmap so `region` can be `'static` — this process's lifetime
    // is the mapping's lifetime anyway, no clean unmap needed.
    let mmap: &'static mut memmap2::MmapMut = Box::leak(Box::new(mmap));
    let region: &'static SharedRegion = unsafe { &*(mmap.as_ptr() as *const SharedRegion) };

    tracing::info!("blockwork-linux-bridge started, shm: {shm_path}");

    let backend = match EvdevBackend::new() {
        Ok(b) => Arc::new(Mutex::new(b)) as Arc<Mutex<dyn InputBackend>>,
        Err(e) => {
            tracing::error!("failed to create EvdevBackend (uinput virtual device): {e}");
            std::process::exit(1);
        }
    };

    spawn_capture_threads(region);

    let worker_tx = spawn_macro_worker(Arc::clone(&backend));
    std::thread::spawn(move || control_loop(region, worker_tx));

    // Heartbeat watchdog on the main thread: exit once the Windows side
    // stops updating its heartbeat (host closed, or its process died).
    let mut last_seen = region.windows_heartbeat.load(Ordering::Relaxed);
    let mut last_change = Instant::now();
    loop {
        std::thread::sleep(HEARTBEAT_POLL);
        let current = region.windows_heartbeat.load(Ordering::Relaxed);
        if current != last_seen {
            last_seen = current;
            last_change = Instant::now();
        } else if last_change.elapsed() >= HEARTBEAT_TIMEOUT {
            tracing::info!("Windows-side heartbeat stale, exiting");
            break;
        }
        region.linux_heartbeat.fetch_add(1, Ordering::Relaxed);
    }
}

/// Enumerates `/dev/input/event*` and spawns one non-exclusive reader
/// thread per usable device. Deliberately does not call `.grab()` — see
/// module docs. Does not watch for hotplug (v1 limitation: devices
/// connected after startup aren't picked up).
fn spawn_capture_threads(region: &'static SharedRegion) {
    let devices: Vec<_> = evdev::enumerate()
        .filter_map(|(path, device)| {
            let name = device.name().unwrap_or("").to_owned();
            if name == "macros-input" {
                // Our own virtual emission device — reading it back would
                // be a feedback loop.
                return None;
            }
            let is_buttonpad = device.properties().contains(PropType::BUTTONPAD);
            let has_abs = device
                .supported_absolute_axes()
                .map(|s| s.contains(AbsoluteAxisCode::ABS_X))
                .unwrap_or(false);
            if is_buttonpad || has_abs {
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
                return None;
            }
            tracing::info!("capturing from {:?} ({name})", path);
            Some(device)
        })
        .collect();

    if devices.is_empty() {
        tracing::warn!("no usable input devices found");
    }

    for mut device in devices {
        std::thread::spawn(move || {
            let mut pending_dx = 0i32;
            let mut pending_dy = 0i32;
            let mut pending_wv = 0i32;
            let mut pending_wh = 0i32;
            // Only read inside a SYNCHRONIZATION arm guarded by a nonzero
            // pending_* check; the lint can't see that.
            #[allow(unused_assignments)]
            let mut last_ts = std::time::SystemTime::now();

            loop {
                let events = match device.fetch_events() {
                    Ok(e) => e,
                    Err(e) => {
                        tracing::warn!("evdev read error, stopping this device: {e}");
                        break;
                    }
                };

                for event in events {
                    last_ts = event.timestamp();
                    match event.event_type() {
                        EventType::KEY => {
                            let key = KeyCode(event.code());
                            let value = event.value();
                            if value == 2 {
                                continue; // autorepeat
                            }
                            let is_btn = key.0 >= 0x100; // BTN_MISC
                            let pressed = value == 1;
                            let wire_event = if is_btn {
                                evdev_button_from_code(key.0).map(|b| {
                                    if pressed { WireCaptureEvent::ButtonPress(b) } else { WireCaptureEvent::ButtonRelease(b) }
                                })
                            } else {
                                evdev_key_to_macro_key(key).map(|k| {
                                    if pressed { WireCaptureEvent::KeyPress(k) } else { WireCaptureEvent::KeyRelease(k) }
                                })
                            };
                            if let Some(wire_event) = wire_event {
                                push_capture(region, wire_event, event.timestamp());
                            }
                        }
                        EventType::RELATIVE => match RelativeAxisCode(event.code()) {
                            RelativeAxisCode::REL_X => pending_dx += event.value(),
                            RelativeAxisCode::REL_Y => pending_dy += event.value(),
                            RelativeAxisCode::REL_WHEEL => pending_wv += event.value(),
                            RelativeAxisCode::REL_HWHEEL => pending_wh += event.value(),
                            _ => {}
                        },
                        EventType::SYNCHRONIZATION => {
                            if pending_dx != 0 || pending_dy != 0 {
                                push_capture(
                                    region,
                                    WireCaptureEvent::MouseMoveRel(
                                        std::mem::take(&mut pending_dx),
                                        std::mem::take(&mut pending_dy),
                                    ),
                                    last_ts,
                                );
                            }
                            if pending_wv != 0 {
                                push_capture(region, WireCaptureEvent::Scroll(0, std::mem::take(&mut pending_wv)), last_ts);
                            }
                            if pending_wh != 0 {
                                push_capture(region, WireCaptureEvent::Scroll(std::mem::take(&mut pending_wh), 0), last_ts);
                            }
                        }
                        _ => {}
                    }
                }
            }
        });
    }
}

fn push_capture(region: &SharedRegion, event: WireCaptureEvent, ts: std::time::SystemTime) {
    let msg = WireCapture { event, ts: WireTimestamp::from_system_time(ts) };
    let bytes = wire::encode_capture(&msg);
    if !region.capture.try_push(&bytes) {
        tracing::warn!("capture ring full, dropping event");
    }
}

/// A `RunMacro` dispatch, handed from `control_loop` to the persistent
/// worker spawned by `spawn_macro_worker` — see that function's docs for
/// why it's not run inline or on a fresh thread per dispatch.
struct RunRequest {
    id: String,
    elapsed_overshoot_ms: f64,
}

/// Dispatch thread: decodes control commands and either runs them inline
/// (`StopLoop`, just flips a flag — cheap) or hands `RunMacro` to the
/// persistent worker via `worker_tx`. Raised to `SCHED_FIFO` and polls at a
/// much tighter interval than the old 1ms, both to trim worst-case latency
/// between a `RunMacro` command landing in the ring and this thread
/// noticing it — that latency sits unaccounted-for ahead of the run's own
/// `Instant::now()` anchor, and at default priority this thread's
/// `thread::sleep` wake-up competes with everything else runnable, the same
/// variable delay the run thread's `SCHED_FIFO` priority exists to avoid.
fn control_loop(region: &'static SharedRegion, worker_tx: std::sync::mpsc::Sender<RunRequest>) {
    raise_current_thread_priority();
    let mut buf = [0u8; wire::SLOT_SIZE - 4];
    loop {
        match region.control.try_pop(&mut buf) {
            Some(len) => {
                let Some(cmd) = wire::decode_control(&buf[..len]) else {
                    tracing::warn!("failed to decode WireControlCommand, skipping");
                    continue;
                };
                match cmd {
                    WireControlCommand::RunMacro(id, elapsed_overshoot_ms) => {
                        if worker_tx.send(RunRequest { id, elapsed_overshoot_ms }).is_err() {
                            tracing::error!("macro worker thread is gone, dropping RunMacro");
                        }
                    }
                    WireControlCommand::StopLoop => {
                        let cleared = run_registry::stop_all();
                        tracing::debug!("stop_loop: cleared {cleared} run(s)");
                    }
                }
            }
            None => std::thread::sleep(CONTROL_POLL),
        }
    }
}

/// Spawns the single persistent thread that actually executes `RunMacro`
/// requests, and returns the channel `control_loop` feeds it through.
///
/// Previously every `RunMacro` dispatch spawned a brand-new OS thread on
/// the hot path — stack mmap, kernel thread creation, scheduling admission,
/// all variable-latency work ahead of this run's `Instant::now()` deadline
/// anchor. Pre-spawning one worker and feeding it requests over a channel
/// removes thread creation from that path: the worker is either already
/// parked in `recv()` (a fast, low-jitter wake), or, if the previous run
/// hasn't returned yet, the new request queues until it does. That queuing
/// is a behavior improvement, not a regression — the old per-call spawn let
/// a just-stopped run and a just-started one race concurrently on the same
/// `InputBackend`'s mutex; a single worker makes consecutive runs strictly
/// sequential instead.
fn spawn_macro_worker(backend: Arc<Mutex<dyn InputBackend>>) -> std::sync::mpsc::Sender<RunRequest> {
    let (tx, rx) = std::sync::mpsc::channel::<RunRequest>();
    let spawned = std::thread::Builder::new().name("macros-run".to_string()).spawn(move || {
        while let Ok(req) = rx.recv() {
            run_macro_blocking(req.id, req.elapsed_overshoot_ms, Arc::clone(&backend));
        }
    });
    if let Err(e) = spawned {
        // Without this thread, every future RunMacro hits the `send`
        // failure branch in `control_loop` and gets logged there too.
        tracing::error!("blockwork-linux-bridge: failed to spawn persistent macro worker thread: {e}");
    }
    tx
}

/// Loads and runs one macro to completion (including loop-mode repeats),
/// mirroring `blockwork-ffi`'s (pre-Wine-bridge) `blockwork_run_macro` — same
/// variable snapshot, speed-multiplier/loop-mode resolution, and post-run
/// variable persistence. The difference: it runs natively here, not
/// embedded in the Wine-hosted process, so `raise_current_thread_priority()`
/// gets real `SCHED_FIFO`. Always called from the persistent worker thread
/// spawned by `spawn_macro_worker` — never spawns its own.
fn run_macro_blocking(id: String, elapsed_overshoot_ms: f64, backend: Arc<Mutex<dyn InputBackend>>) {
    let Some(mac) = config::get_macro_by_id(&id) else {
        tracing::warn!("run_macro: macro '{id}' not found");
        return;
    };

    let emulator = backend;
    let variables: VariableStore =
        Arc::new(Mutex::new(mac.variables.iter().map(|v| (v.name.clone(), v.value.clone())).collect()));
    let settings = config::load_settings();
    let speed_multiplier = mac.speed_multiplier * settings.global_speed_multiplier.unwrap_or(1.0);
    let loop_mode = settings.loop_mode_enabled.unwrap_or(false);
    // Confirms a RunMacro command reached and was accepted here — otherwise
    // there's no way to tell from the log whether a playback attempt made
    // it across the bridge at all versus never being sent.
    tracing::info!(
        "run_macro: starting '{}' ({id}), speed_multiplier={speed_multiplier}, loop_mode={loop_mode}, elapsed_overshoot_ms={elapsed_overshoot_ms}",
        mac.name
    );

    let mut offset = Duration::from_secs_f64(elapsed_overshoot_ms.max(0.0) / 1000.0);
    let flag = run_registry::begin_run();
    loop {
        mac.clone().run_with_offset(Arc::clone(&emulator), Some(Arc::clone(&flag)), speed_multiplier, Arc::clone(&variables), offset);
        // Only the first iteration corresponds to the real attempt-start
        // trigger; a loop-mode repeat starting right after has nothing to
        // backdate against.
        offset = Duration::ZERO;
        let keep_looping = loop_mode && flag.lock().map(|g| *g).unwrap_or(false);
        if !keep_looping {
            break;
        }
    }
    run_registry::end_run(&flag);
    if let Ok(values) = variables.lock() {
        let mut mac = mac.clone();
        mac.sync_variables_from(&values);
        if let Err(e) = mac.save() {
            tracing::warn!("blockwork-linux-bridge: failed to persist variable values: {e}");
        }
    }
}
