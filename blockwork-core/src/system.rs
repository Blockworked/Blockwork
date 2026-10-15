//! System state behind the app, idle and screen-lock triggers and their
//! reporters: running programs, time since the last input, and the lock screen.

use std::time::Duration;

/// Names of the running programs (executable names, without `.exe`).
pub fn running_apps() -> Result<Vec<String>, String> {
    let mut names = process_names()?;
    names.sort_unstable();
    names.dedup();
    Ok(names)
}

/// Whether any of `names` contains `pattern`, ignoring case.
pub fn app_matches(names: &[String], pattern: &str) -> bool {
    let pattern = pattern.trim().to_lowercase();
    !pattern.is_empty() && names.iter().any(|n| n.to_lowercase().contains(&pattern))
}

/// Whether a program whose name contains `pattern` is running.
pub fn app_running(pattern: &str) -> Result<bool, String> {
    Ok(app_matches(&running_apps()?, pattern))
}

#[cfg(target_os = "linux")]
fn process_names() -> Result<Vec<String>, String> {
    if crate::flatpak::is_flatpak() {
        // The sandbox has its own process list.
        let out = crate::flatpak::host_command("ps")
            .args(["-eo", "comm=,args="])
            .output()
            .map_err(|e| format!("couldn't list programs: {e}"))?;
        let text = String::from_utf8_lossy(&out.stdout);
        return Ok(text
            .lines()
            .flat_map(|line| {
                let mut parts = line.split_whitespace();
                let comm = parts.next().unwrap_or_default().to_string();
                let exe = parts.next().map(basename).unwrap_or_default();
                [comm, exe]
            })
            .filter(|n| !n.is_empty())
            .collect());
    }
    let dir = std::fs::read_dir("/proc").map_err(|e| format!("couldn't list programs: {e}"))?;
    let mut names = Vec::new();
    for entry in dir.flatten() {
        let path = entry.path();
        if !entry.file_name().to_string_lossy().bytes().all(|b| b.is_ascii_digit()) {
            continue;
        }
        if let Ok(comm) = std::fs::read_to_string(path.join("comm")) {
            names.push(comm.trim().to_string());
        }
        // comm is cut to 15 characters; the command line has the full name.
        if let Ok(cmdline) = std::fs::read(path.join("cmdline"))
            && let Some(first) = cmdline.split(|&b| b == 0).next()
            && !first.is_empty()
        {
            names.push(basename(&String::from_utf8_lossy(first)));
        }
    }
    Ok(names)
}

#[cfg(target_os = "macos")]
fn process_names() -> Result<Vec<String>, String> {
    let out = std::process::Command::new("ps")
        .args(["-axo", "comm="])
        .output()
        .map_err(|e| format!("couldn't list programs: {e}"))?;
    let text = String::from_utf8_lossy(&out.stdout);
    let mut names = Vec::new();
    for path in text.lines() {
        names.push(basename(path));
        // "/Applications/Visual Studio Code.app/..." is also "Visual Studio Code".
        if let Some(app) = path.split('/').find_map(|part| part.strip_suffix(".app")) {
            names.push(app.to_string());
        }
    }
    Ok(names)
}

#[cfg(windows)]
fn process_names() -> Result<Vec<String>, String> {
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
    };
    let mut names = Vec::new();
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot == INVALID_HANDLE_VALUE {
            return Err("couldn't list programs".into());
        }
        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut more = Process32FirstW(snapshot, &mut entry) != 0;
        while more {
            let len = entry.szExeFile.iter().position(|&c| c == 0).unwrap_or(entry.szExeFile.len());
            let name = String::from_utf16_lossy(&entry.szExeFile[..len]);
            names.push(strip_exe(&name).to_string());
            more = Process32NextW(snapshot, &mut entry) != 0;
        }
        CloseHandle(snapshot);
    }
    Ok(names)
}

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
fn process_names() -> Result<Vec<String>, String> {
    Err("listing programs isn't supported on this system".into())
}

#[cfg_attr(windows, allow(dead_code))]
fn basename(path: &str) -> String {
    let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
    strip_exe(name).to_string()
}

fn strip_exe(name: &str) -> &str {
    name.strip_suffix(".exe").or_else(|| name.strip_suffix(".EXE")).unwrap_or(name)
}

/// How long since the last keyboard or mouse input.
pub fn idle_time() -> Result<Duration, String> {
    let seen = crate::recording::time_since_input();
    match (os_idle_time(), seen) {
        (Some(os), Some(seen)) => Ok(os.min(seen)),
        (Some(os), None) => Ok(os),
        (None, Some(seen)) => Ok(seen),
        (None, None) => Err("can't see keyboard and mouse input, so idle time is unknown".into()),
    }
}

#[cfg(windows)]
fn os_idle_time() -> Option<Duration> {
    use windows_sys::Win32::System::SystemInformation::GetTickCount;
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
    let mut info = LASTINPUTINFO { cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32, dwTime: 0 };
    if unsafe { GetLastInputInfo(&mut info) } == 0 {
        return None;
    }
    Some(Duration::from_millis(unsafe { GetTickCount() }.wrapping_sub(info.dwTime) as u64))
}

#[cfg(target_os = "macos")]
fn os_idle_time() -> Option<Duration> {
    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGEventSourceSecondsSinceLastEventType(state: i32, event_type: u32) -> f64;
    }
    // HID system state, any input event.
    let secs = unsafe { CGEventSourceSecondsSinceLastEventType(1, u32::MAX) };
    (secs.is_finite() && secs >= 0.0).then(|| Duration::from_secs_f64(secs))
}

// Linux has no compositor-neutral idle API; the input capture covers it.
#[cfg(not(any(windows, target_os = "macos")))]
fn os_idle_time() -> Option<Duration> {
    None
}

/// Whether the lock screen is showing.
#[cfg(target_os = "linux")]
pub fn screen_locked() -> Result<bool, String> {
    use std::sync::OnceLock;
    static SESSION: OnceLock<Option<zbus::blocking::Connection>> = OnceLock::new();
    static SYSTEM: OnceLock<Option<zbus::blocking::Connection>> = OnceLock::new();
    let session = SESSION.get_or_init(|| zbus::blocking::Connection::session().ok());
    // KDE and most others, then GNOME.
    for (service, path) in [
        ("org.freedesktop.ScreenSaver", "/org/freedesktop/ScreenSaver"),
        ("org.gnome.ScreenSaver", "/org/gnome/ScreenSaver"),
    ] {
        if let Some(bus) = session
            && let Ok(reply) = bus.call_method(Some(service), path, Some(service), "GetActive", &())
            && let Ok(active) = reply.body().deserialize::<bool>()
        {
            return Ok(active);
        }
    }
    // Lockers like swaylock and hyprlock only tell logind.
    let system = SYSTEM.get_or_init(|| zbus::blocking::Connection::system().ok());
    if let Some(bus) = system
        && let Ok(reply) = bus.call_method(
            Some("org.freedesktop.login1"),
            "/org/freedesktop/login1/session/auto",
            Some("org.freedesktop.DBus.Properties"),
            "Get",
            &("org.freedesktop.login1.Session", "LockedHint"),
        )
        && let Ok(value) = reply.body().deserialize::<zbus::zvariant::OwnedValue>()
        && let Ok(locked) = bool::try_from(value)
    {
        return Ok(locked);
    }
    Err("couldn't tell whether the screen is locked (no screensaver or logind service)".into())
}

#[cfg(windows)]
pub fn screen_locked() -> Result<bool, String> {
    // The lock screen runs as LogonUI.
    Ok(process_names()?.iter().any(|n| n.eq_ignore_ascii_case("LogonUI")))
}

#[cfg(target_os = "macos")]
pub fn screen_locked() -> Result<bool, String> {
    use core_foundation::base::{CFType, TCFType};
    use core_foundation::boolean::CFBoolean;
    use core_foundation::dictionary::{CFDictionary, CFDictionaryRef};
    use core_foundation::string::CFString;
    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGSessionCopyCurrentDictionary() -> CFDictionaryRef;
    }
    let raw = unsafe { CGSessionCopyCurrentDictionary() };
    if raw.is_null() {
        return Err("no login session".into());
    }
    let session: CFDictionary<CFString, CFType> = unsafe { CFDictionary::wrap_under_create_rule(raw) };
    let key = CFString::from_static_string("CGSSessionScreenIsLocked");
    Ok(session.find(&key).and_then(|v| v.downcast::<CFBoolean>()).is_some_and(bool::from))
}

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
pub fn screen_locked() -> Result<bool, String> {
    Err("lock screen detection isn't supported on this system".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_program_names_loosely() {
        let names = vec!["firefox".to_string(), "Visual Studio Code".to_string()];
        assert!(app_matches(&names, "Firefox"));
        assert!(app_matches(&names, "studio"));
        assert!(!app_matches(&names, "chrome"));
        assert!(!app_matches(&names, " "));
        assert_eq!(basename("/usr/lib/firefox/firefox"), "firefox");
        assert_eq!(basename("C:\\Apps\\Code.exe"), "Code");
    }

    #[test]
    #[ignore = "reads the real session"]
    fn reads_the_real_session() {
        println!("locked: {:?}", screen_locked());
        println!("apps: {}", running_apps().unwrap().len());
        assert!(app_running("cargo").unwrap());
    }

    #[test]
    fn lists_this_test_program() {
        let names = running_apps().unwrap();
        assert!(!names.is_empty());
    }
}
