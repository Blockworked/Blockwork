//! Wine detection, shared-memory setup, and launching the native Linux
//! helper (`blockwork-linux-bridge`) — needed because `WH_KEYBOARD_LL`/
//! `WH_MOUSE_LL` don't see real host input under Wine, and `SendInput`
//! emission is unreliable there too. Mirrors Click Between Frames' proven
//! mechanism (`theyareonit/Click-Between-Frames`, `src/windows.cpp`) — same
//! API calls, same `Z:` drive trick, extended with a second ring buffer
//! (CBF only needs capture) carrying `WireControlCommand`s: macro playback
//! itself runs natively on the Linux side too, the only way it gets real
//! `SCHED_FIFO` scheduling instead of Wine's weaker `SetThreadPriority`
//! emulation.
#![cfg(windows)]

use blockwork_core::wire::{self, SharedRegion, WireCapture, WireControlCommand};
use std::ffi::c_void;
use std::os::windows::ffi::OsStrExt;
use std::sync::atomic::Ordering;
use windows_sys::Win32::Foundation::{
    CloseHandle, GENERIC_READ, GENERIC_WRITE, HANDLE, INVALID_HANDLE_VALUE,
};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileA, SetEndOfFile, SetFilePointerEx, CREATE_ALWAYS, FILE_ATTRIBUTE_NORMAL, FILE_BEGIN,
    FILE_SHARE_READ, FILE_SHARE_WRITE,
};
use windows_sys::Win32::System::LibraryLoader::{GetModuleHandleA, GetProcAddress};
use windows_sys::Win32::System::Memory::{
    CreateFileMappingA, GetProcessHeap, HeapFree, MapViewOfFile, FILE_MAP_ALL_ACCESS,
    PAGE_READWRITE,
};
use windows_sys::Win32::System::Threading::{
    CreateProcessA, GetCurrentProcessId, PROCESS_INFORMATION, STARTUPINFOA,
};

/// True if running under Wine on a Linux host (i.e. Proton) — checked via
/// `wine_get_host_version`, same technique CBF's `windows.cpp` uses.
pub fn detect_wine_linux() -> bool {
    unsafe {
        let ntdll = GetModuleHandleA(b"ntdll.dll\0".as_ptr());
        if ntdll.is_null() {
            tracing::info!("wine_bridge: GetModuleHandleA(ntdll.dll) returned null");
            return false;
        }
        let Some(proc) = GetProcAddress(ntdll, b"wine_get_host_version\0".as_ptr()) else {
            tracing::info!("wine_bridge: wine_get_host_version not found in ntdll.dll (native Windows, not Wine)");
            return false;
        };
        let f: unsafe extern "system" fn(*mut *const u8, *mut *const u8) = std::mem::transmute(proc);
        let mut sysname: *const u8 = std::ptr::null();
        let mut release: *const u8 = std::ptr::null();
        f(&mut sysname, &mut release);
        if sysname.is_null() {
            tracing::info!("wine_bridge: wine_get_host_version returned null sysname");
            return false;
        }
        let sysname_str = std::ffi::CStr::from_ptr(sysname as *const i8).to_string_lossy().into_owned();
        tracing::info!("wine_bridge: wine_get_host_version reports sysname='{sysname_str}'");
        sysname_str == "Linux"
    }
}

/// Converts a Windows-side path (as `CCFileUtils` resolved it, e.g. the
/// bundled `linux-input.so` resource) to its real Unix path via Wine's
/// `wine_get_unix_file_name`. Returns `None` on any failure — caller should
/// treat that as "bridge unavailable" rather than panicking.
fn wine_unix_path(windows_path: &str) -> Option<String> {
    unsafe {
        let kernel32 = GetModuleHandleA(b"kernel32.dll\0".as_ptr());
        if kernel32.is_null() {
            return None;
        }
        let proc = GetProcAddress(kernel32, b"wine_get_unix_file_name\0".as_ptr())?;
        let f: unsafe extern "C" fn(*const u16) -> *mut i8 = std::mem::transmute(proc);

        let wide: Vec<u16> = std::ffi::OsStr::new(windows_path).encode_wide().chain(std::iter::once(0)).collect();
        let result = f(wide.as_ptr());
        if result.is_null() {
            return None;
        }
        let unix_path = std::ffi::CStr::from_ptr(result).to_string_lossy().into_owned();
        HeapFree(GetProcessHeap(), 0, result as *const c_void);
        Some(unix_path)
    }
}

/// Owns the shared-memory mapping and the handles behind it. Kept alive in
/// `blockwork-ffi`'s static state for the process lifetime — no
/// clean-shutdown path today (same as `EMULATOR`/the capture thread).
#[allow(dead_code)] // fields exist to keep the handles/mapping alive, never read again
pub struct WineBridge {
    shm_file: HANDLE,
    shm_mapping: HANDLE,
    view: *mut c_void,
    pub region: &'static SharedRegion,
}

// SAFETY: the raw handles/pointer are only ever touched to bump the
// heartbeat (from `blockwork_init`'s caller thread) and read via `region`
// (itself all-atomics/lock-free ring buffers) — no interior mutation of
// the handles themselves after setup.
unsafe impl Send for WineBridge {}
unsafe impl Sync for WineBridge {}

/// The Linux helper's watchdog exits once `windows_heartbeat` goes stale
/// for a few seconds — this keeps it alive for as long as this process
/// runs, so a real host crash (not just quitting) still lets the helper
/// notice and exit rather than orphaning itself.
pub fn spawn_heartbeat_thread(region: &'static SharedRegion) {
    std::thread::spawn(move || loop {
        region.windows_heartbeat.fetch_add(1, Ordering::Relaxed);
        std::thread::sleep(std::time::Duration::from_secs(1));
    });
}

/// Sets up the shared-memory region and launches `linux_bridge_resource_path`
/// (a Windows-side path to the bundled native Linux binary) as a real Unix
/// process, mirroring CBF's `windows.cpp:windowsSetup()` almost exactly,
/// plus a second ring buffer for emission. Returns `None` on any failure —
/// every step logs why via `tracing::warn!`.
pub fn setup_and_launch(linux_bridge_resource_path: &str) -> Option<WineBridge> {
    unsafe {
        let pid = GetCurrentProcessId();
        let win_shm_path = format!("Z:\\dev\\shm\\macros-{pid}\0");
        let unix_shm_path = format!("/dev/shm/macros-{pid}");

        let shm_file = CreateFileA(
            win_shm_path.as_ptr(),
            GENERIC_READ | GENERIC_WRITE,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            std::ptr::null(),
            CREATE_ALWAYS,
            FILE_ATTRIBUTE_NORMAL,
            std::ptr::null_mut(),
        );
        if shm_file == INVALID_HANDLE_VALUE {
            tracing::warn!("wine_bridge: failed to create shm file");
            return None;
        }

        let size = wire::SHARED_REGION_SIZE as i64;
        if SetFilePointerEx(shm_file, size, std::ptr::null_mut(), FILE_BEGIN) == 0
            || SetEndOfFile(shm_file) == 0
        {
            tracing::warn!("wine_bridge: failed to size shm file");
            CloseHandle(shm_file);
            return None;
        }

        let shm_mapping = CreateFileMappingA(
            shm_file,
            std::ptr::null(),
            PAGE_READWRITE,
            0,
            wire::SHARED_REGION_SIZE as u32,
            std::ptr::null(),
        );
        if shm_mapping.is_null() {
            tracing::warn!("wine_bridge: CreateFileMappingA failed");
            CloseHandle(shm_file);
            return None;
        }

        let view = MapViewOfFile(shm_mapping, FILE_MAP_ALL_ACCESS, 0, 0, wire::SHARED_REGION_SIZE);
        if view.Value.is_null() {
            tracing::warn!("wine_bridge: MapViewOfFile failed");
            CloseHandle(shm_mapping);
            CloseHandle(shm_file);
            return None;
        }
        let view = view.Value;
        std::ptr::write_bytes(view as *mut u8, 0, wire::SHARED_REGION_SIZE);

        let Some(unix_bin_path) = wine_unix_path(linux_bridge_resource_path) else {
            tracing::warn!("wine_bridge: failed to resolve Unix path for linux-input binary");
            CloseHandle(shm_mapping);
            CloseHandle(shm_file);
            return None;
        };

        let cmdline = format!(
            "/bin/sh -c \"chmod +x '{unix_bin_path}' && exec '{unix_bin_path}' '{unix_shm_path}'\"\0"
        );
        let mut si: STARTUPINFOA = std::mem::zeroed();
        si.cb = std::mem::size_of::<STARTUPINFOA>() as u32;
        let mut pi: PROCESS_INFORMATION = std::mem::zeroed();
        let ok = CreateProcessA(
            b"Z:\\bin\\sh\0".as_ptr(),
            cmdline.as_bytes().as_ptr() as *mut u8,
            std::ptr::null(),
            std::ptr::null(),
            0,
            0,
            std::ptr::null(),
            std::ptr::null(),
            &si,
            &mut pi,
        );
        if ok == 0 {
            tracing::warn!("wine_bridge: failed to launch linux-input helper");
            CloseHandle(shm_mapping);
            CloseHandle(shm_file);
            return None;
        }
        CloseHandle(pi.hProcess);
        CloseHandle(pi.hThread);

        tracing::info!("wine_bridge: launched linux-input helper (unix path {unix_bin_path})");
        Some(WineBridge { shm_file, shm_mapping, view, region: &*(view as *const SharedRegion) })
    }
}

/// Spawns the thread that drains the capture ring and feeds events into the
/// same recording-queue logic the native OS hook path uses.
pub fn spawn_capture_forwarder(region: &'static SharedRegion) {
    tracing::info!("wine_bridge: capture forwarder thread starting");
    std::thread::spawn(move || {
        // A panic here would otherwise kill this thread silently (its
        // stderr output goes nowhere observable from a console-less DLL).
        // Catch per event so one bad event can't stop draining the ring.
        let mut callback = blockwork_core::recording::build_capture_callback();
        let mut buf = [0u8; wire::SLOT_SIZE - 4];
        let mut processed: u64 = 0;
        let mut last_log = std::time::Instant::now();
        loop {
            match region.capture.try_pop(&mut buf) {
                Some(len) => {
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        if let Some(WireCapture { event, ts }) = wire::decode_capture(&buf[..len]) {
                            let timestamp = blockwork_core::macros::backend::CaptureTimestamp::Hardware(ts.to_system_time());
                            // Discarded: `CaptureDecision::Suppress` never
                            // fires here — the hotkey table is always empty
                            // (see `blockwork_init`), and the Linux side
                            // doesn't grab devices anyway.
                            let _ = callback(event.into(), timestamp);
                        }
                    }));
                    if result.is_err() {
                        tracing::error!("wine_bridge: capture forwarder panicked processing one event, continuing");
                    }
                    processed += 1;
                }
                None => std::thread::sleep(std::time::Duration::from_millis(1)),
            }
            if last_log.elapsed() >= std::time::Duration::from_secs(5) {
                tracing::info!("wine_bridge: capture forwarder alive, {processed} events processed so far");
                last_log = std::time::Instant::now();
            }
        }
    });
}

/// Pushes a control command into the bridge's control ring for
/// `blockwork-linux-bridge` to act on. Retries briefly on a full ring —
/// this is only called once per `blockwork_run_macro`/`blockwork_stop_loop`
/// call, not in a tight loop, so a short retry window is enough headroom.
fn push_control(region: &SharedRegion, cmd: &WireControlCommand) -> Result<(), String> {
    let bytes = wire::encode_control(cmd);
    for _ in 0..1000 {
        if region.control.try_push(&bytes) {
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_micros(500));
    }
    Err("control ring full".to_string())
}

/// Tells `blockwork-linux-bridge` to run the macro with this id — the entire
/// timed run happens natively over there now; see `wire::WireControlCommand`'s
/// docs for why. `elapsed_overshoot_ms` is forwarded as-is.
pub fn send_run_macro(region: &SharedRegion, macro_id: &str, elapsed_overshoot_ms: f64) -> Result<(), String> {
    push_control(region, &WireControlCommand::RunMacro(macro_id.to_string(), elapsed_overshoot_ms))
}

/// Tells `blockwork-linux-bridge` to stop every run it has in flight.
pub fn send_stop_loop(region: &SharedRegion) -> Result<(), String> {
    push_control(region, &WireControlCommand::StopLoop)
}
