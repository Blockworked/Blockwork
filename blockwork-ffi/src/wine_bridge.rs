//! Wine detection and launch for `blockwork-linux-bridge`.
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

/// True under Wine on a Linux host, via `wine_get_host_version`.
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

/// Windows path to Unix path via `wine_get_unix_file_name`. None on failure.
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

/// Shared-memory mapping + handles. Lives in static state; no shutdown path.
#[allow(dead_code)] // fields exist to keep the handles/mapping alive, never read again
pub struct WineBridge {
    shm_file: HANDLE,
    shm_mapping: HANDLE,
    view: *mut c_void,
    pub region: &'static SharedRegion,
}

// SAFETY: handles never mutate after setup; only the heartbeat is bumped
// and `region` itself is lock-free.
unsafe impl Send for WineBridge {}
unsafe impl Sync for WineBridge {}

/// Bumps `windows_heartbeat` so the helper doesn't exit while we're alive.
pub fn spawn_heartbeat_thread(region: &'static SharedRegion) {
    std::thread::spawn(move || loop {
        region.windows_heartbeat.fetch_add(1, Ordering::Relaxed);
        std::thread::sleep(std::time::Duration::from_secs(1));
    });
}

/// Creates the shm region and launches the Linux helper as a Unix process.
/// None on failure; each step logs why.
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

/// Drains the capture ring into the normal recording queue.
pub fn spawn_capture_forwarder(region: &'static SharedRegion) {
    tracing::info!("wine_bridge: capture forwarder thread starting");
    std::thread::spawn(move || {
        // Catch per event so one bad event can't kill the forwarder thread.
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
                            // Suppress never fires here: hotkey table is empty, no device grab.
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

/// Pushes a control command to the helper. Retries briefly if the ring is full.
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

/// Asks the helper to run the macro with this id.
pub fn send_run_macro(region: &SharedRegion, macro_id: &str, elapsed_overshoot_ms: f64) -> Result<(), String> {
    push_control(region, &WireControlCommand::RunMacro(macro_id.to_string(), elapsed_overshoot_ms))
}

/// Asks the helper to stop all in-flight runs.
pub fn send_stop_loop(region: &SharedRegion) -> Result<(), String> {
    push_control(region, &WireControlCommand::StopLoop)
}
