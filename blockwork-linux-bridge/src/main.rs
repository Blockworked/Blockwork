//! Native Linux helper for Proton/Wine hosts. Captures `/dev/input/event*`
//! and runs macros over the shared-memory region at `argv[1]`.
//! Exits when the Windows-side heartbeat goes stale.

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
/// Poll interval for the control ring while idle. Tight on purpose:
/// this thread runs at real-time priority and the poll bounds RunMacro notice latency.
const CONTROL_POLL: Duration = Duration::from_micros(200);

/// stderr isn't inherited from the Windows-side launcher, so log to a file
/// next to the shm path instead.
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

    // SAFETY: Windows side owns the file for our lifetime. Sized to
    // `SHARED_REGION_SIZE` before launch; a stale tmpfs file is harmless.
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

    // Leak the mmap for a 'static region; process and mapping lifetimes match.
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

    // Watchdog: exit once the Windows side stops bumping its heartbeat.
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

/// One non-exclusive reader thread per usable `/dev/input/event*` device.
/// No `.grab()`, no hotplug watch: devices added after startup are ignored.
fn spawn_capture_threads(region: &'static SharedRegion) {
    let devices: Vec<_> = evdev::enumerate()
        .filter_map(|(path, device)| {
            let name = device.name().unwrap_or("").to_owned();
            if name == "macros-input" {
                // Our own virtual emission device - reading it back would
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

/// RunMacro request forwarded from `control_loop` to the worker.
struct RunRequest {
    id: String,
    elapsed_overshoot_ms: f64,
}

/// Polls the control ring. StopLoop runs inline; RunMacro goes to the worker.
/// Runs at real-time priority so wake delay doesn't add playback latency.
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

/// Single persistent macro worker. Avoids spawning a thread per RunMacro
/// (that cost sits ahead of the run's timing anchor) and keeps
/// consecutive runs sequential instead of racing on the backend lock.
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

/// Runs one macro to completion, same snapshotting as `blockwork_run_macro`
/// but natively here so it gets real `SCHED_FIFO`. Runs on the worker thread.
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
    // Log accept so a missing playback attempt is distinguishable from a lost command.
    tracing::info!(
        "run_macro: starting '{}' ({id}), speed_multiplier={speed_multiplier}, loop_mode={loop_mode}, elapsed_overshoot_ms={elapsed_overshoot_ms}",
        mac.name
    );

    let mut offset = Duration::from_secs_f64(elapsed_overshoot_ms.max(0.0) / 1000.0);
    let flag = run_registry::begin_run();
    loop {
        mac.clone().run_with_offset(Arc::clone(&emulator), Some(Arc::clone(&flag)), speed_multiplier, Arc::clone(&variables), offset);
        // Only the first iteration backdates against the trigger.
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
