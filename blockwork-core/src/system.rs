//! System state behind the app, idle, screen-lock, network and USB triggers
//! and their reporters.

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

fn process_names() -> Result<Vec<String>, String> {
    Ok(process_list()?.into_iter().map(|(_, name)| name).collect())
}

/// The ID of the oldest-started program whose name contains `pattern`
/// (the lowest ID), if one is running.
pub fn app_pid(pattern: &str) -> Result<Option<u32>, String> {
    let pattern = pattern.trim().to_lowercase();
    if pattern.is_empty() {
        return Ok(None);
    }
    Ok(process_list()?
        .into_iter()
        .filter(|(pid, name)| *pid != 0 && name.to_lowercase().contains(&pattern))
        .map(|(pid, _)| pid)
        .min())
}

/// Ends the process with this ID.
pub fn kill(pid: f64) -> Result<(), String> {
    if pid.fract() != 0.0 || pid < 1.0 || pid > u32::MAX as f64 {
        return Err(format!("{pid} isn't a process ID"));
    }
    let pid = pid as u32;
    if pid == std::process::id() {
        return Err("that's Blockwork itself".into());
    }
    kill_pid(pid)
}

#[cfg(unix)]
fn kill_pid(pid: u32) -> Result<(), String> {
    if crate::flatpak::is_flatpak() {
        let ok = crate::flatpak::host_command("kill")
            .arg(pid.to_string())
            .status()
            .is_ok_and(|s| s.success());
        return if ok {
            Ok(())
        } else {
            Err(format!("couldn't end process {pid}"))
        };
    }
    // SAFETY: plain signal send; no memory is shared.
    if unsafe { libc::kill(pid as libc::pid_t, libc::SIGTERM) } == 0 {
        Ok(())
    } else {
        Err(format!(
            "couldn't end process {pid}: {}",
            std::io::Error::last_os_error()
        ))
    }
}

#[cfg(windows)]
fn kill_pid(pid: u32) -> Result<(), String> {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_TERMINATE, TerminateProcess};
    // SAFETY: the handle is checked and closed here.
    unsafe {
        let handle = OpenProcess(PROCESS_TERMINATE, 0, pid);
        if handle.is_null() {
            return Err(format!(
                "couldn't end process {pid}: {}",
                std::io::Error::last_os_error()
            ));
        }
        let ended = TerminateProcess(handle, 1) != 0;
        CloseHandle(handle);
        if ended {
            Ok(())
        } else {
            Err(format!("couldn't end process {pid}"))
        }
    }
}

#[cfg(not(any(unix, windows)))]
fn kill_pid(_: u32) -> Result<(), String> {
    Err("ending programs isn't supported on this system".into())
}

/// The commands that lock, sleep, log out, restart or shut down, in the
/// order to try them.
fn power_commands(action: blockwork_model::macros::PowerAction) -> Vec<Vec<String>> {
    use blockwork_model::macros::PowerAction as P;
    let list = |items: &[&[&str]]| {
        items
            .iter()
            .map(|c| c.iter().map(|s| s.to_string()).collect())
            .collect()
    };
    if cfg!(windows) {
        return list(match action {
            P::Lock => &[&["rundll32.exe", "user32.dll,LockWorkStation"]],
            P::Sleep => &[&["rundll32.exe", "powrprof.dll,SetSuspendState", "0,1,0"]],
            P::LogOut => &[&["shutdown", "/l"]],
            P::Restart => &[&["shutdown", "/r", "/t", "0"]],
            P::ShutDown => &[&["shutdown", "/s", "/t", "0"]],
        });
    }
    if cfg!(target_os = "macos") {
        let tell = |what: &str| format!("tell application \"System Events\" to {what}");
        return match action {
            P::Lock => list(&[&["pmset", "displaysleepnow"]]),
            P::Sleep => list(&[&["pmset", "sleepnow"]]),
            P::LogOut => vec![vec!["osascript".into(), "-e".into(), tell("log out")]],
            P::Restart => vec![vec!["osascript".into(), "-e".into(), tell("restart")]],
            P::ShutDown => vec![vec!["osascript".into(), "-e".into(), tell("shut down")]],
        };
    }
    let session = std::env::var("XDG_SESSION_ID").unwrap_or_default();
    match action {
        P::Lock => {
            let mut commands = list(&[&["loginctl", "lock-session"]]);
            if !session.is_empty() {
                commands.insert(0, vec!["loginctl".into(), "lock-session".into(), session]);
            }
            commands
        }
        P::Sleep => list(&[&["systemctl", "suspend"]]),
        P::LogOut => {
            let mut commands = list(&[
                &["qdbus", "org.kde.Shutdown", "/Shutdown", "logout"],
                &["gnome-session-quit", "--logout", "--no-prompt"],
            ]);
            if !session.is_empty() {
                commands.push(vec!["loginctl".into(), "terminate-session".into(), session]);
            }
            commands
        }
        P::Restart => list(&[&["systemctl", "reboot"]]),
        P::ShutDown => list(&[&["systemctl", "poweroff"]]),
    }
}

/// Locks the screen, sleeps, logs out, restarts or shuts down.
pub fn power(action: blockwork_model::macros::PowerAction) -> Result<(), String> {
    let mut last = String::from("no command to run");
    for command in power_commands(action) {
        #[cfg(not(windows))]
        let mut process = crate::flatpak::host_command(&command[0]);
        #[cfg(windows)]
        let mut process = {
            use std::os::windows::process::CommandExt;
            let mut process = std::process::Command::new(&command[0]);
            process.creation_flags(0x0800_0000);
            process
        };
        match process
            .args(&command[1..])
            .stdin(std::process::Stdio::null())
            .output()
        {
            Ok(out) if out.status.success() => return Ok(()),
            Ok(out) => last = String::from_utf8_lossy(&out.stderr).trim().to_string(),
            Err(e) => last = format!("{}: {e}", command[0]),
        }
    }
    Err(last)
}

/// One fact about the computer by name: `user name`, `computer name`,
/// `system`, `system version`, `home folder` or `display scale`.
pub fn info(what: &str) -> Result<String, String> {
    Ok(match what {
        "user name" => std::env::var("USER")
            .or_else(|_| std::env::var("USERNAME"))
            .unwrap_or_default(),
        "computer name" => computer_name(),
        "system" => match std::env::consts::OS {
            "macos" => "macOS".into(),
            "windows" => "Windows".into(),
            "linux" => "Linux".into(),
            other => other.into(),
        },
        "system version" => system_version(),
        "home folder" => dirs::home_dir()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default(),
        "display scale" => crate::screen::capture()?.scale.to_string(),
        other => return Err(format!("unknown computer info '{other}'")),
    })
}

fn computer_name() -> String {
    if let Ok(name) = std::env::var("COMPUTERNAME") {
        return name;
    }
    #[cfg(unix)]
    {
        let mut buf = [0u8; 256];
        // SAFETY: the buffer is large enough and its length is passed.
        if unsafe { libc::gethostname(buf.as_mut_ptr().cast(), buf.len()) } == 0 {
            let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
            return String::from_utf8_lossy(&buf[..end]).into_owned();
        }
    }
    String::new()
}

fn system_version() -> String {
    let run = |program: &str, args: &[&str]| {
        std::process::Command::new(program)
            .args(args)
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_default()
    };
    if cfg!(target_os = "macos") {
        return run("sw_vers", &["-productVersion"]);
    }
    if cfg!(windows) {
        return run("cmd", &["/C", "ver"]);
    }
    let release = std::fs::read_to_string("/etc/os-release")
        .or_else(|_| std::fs::read_to_string("/run/host/os-release"))
        .unwrap_or_default();
    release
        .lines()
        .find_map(|l| l.strip_prefix("PRETTY_NAME="))
        .map(|v| v.trim_matches('"').to_string())
        .unwrap_or_else(|| run("uname", &["-sr"]))
}

/// Whether a program whose name contains `pattern` is running.
pub fn app_running(pattern: &str) -> Result<bool, String> {
    Ok(app_matches(&running_apps()?, pattern))
}

#[cfg(target_os = "linux")]
fn process_list() -> Result<Vec<(u32, String)>, String> {
    if crate::flatpak::is_flatpak() {
        // The sandbox has its own process list.
        let out = crate::flatpak::host_command("ps")
            .args(["-eo", "pid=,comm=,args="])
            .output()
            .map_err(|e| format!("couldn't list programs: {e}"))?;
        let text = String::from_utf8_lossy(&out.stdout);
        return Ok(text
            .lines()
            .flat_map(|line| {
                let mut parts = line.split_whitespace();
                let pid = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
                let comm = parts.next().unwrap_or_default().to_string();
                let exe = parts.next().map(basename).unwrap_or_default();
                [(pid, comm), (pid, exe)]
            })
            .filter(|(_, n)| !n.is_empty())
            .collect());
    }
    let dir = std::fs::read_dir("/proc").map_err(|e| format!("couldn't list programs: {e}"))?;
    let mut names = Vec::new();
    for entry in dir.flatten() {
        let path = entry.path();
        let Ok(pid) = entry.file_name().to_string_lossy().parse::<u32>() else {
            continue;
        };
        if let Ok(comm) = std::fs::read_to_string(path.join("comm")) {
            names.push((pid, comm.trim().to_string()));
        }
        // comm is cut to 15 characters; the command line has the full name.
        if let Ok(cmdline) = std::fs::read(path.join("cmdline"))
            && let Some(first) = cmdline.split(|&b| b == 0).next()
            && !first.is_empty()
        {
            names.push((pid, basename(&String::from_utf8_lossy(first))));
        }
    }
    Ok(names)
}

#[cfg(target_os = "macos")]
fn process_list() -> Result<Vec<(u32, String)>, String> {
    let out = std::process::Command::new("ps")
        .args(["-axo", "pid=,comm="])
        .output()
        .map_err(|e| format!("couldn't list programs: {e}"))?;
    let text = String::from_utf8_lossy(&out.stdout);
    let mut names = Vec::new();
    for line in text.lines() {
        let line = line.trim_start();
        let (pid, path) = line.split_once(' ').unwrap_or((line, ""));
        let pid = pid.parse().unwrap_or(0);
        let path = path.trim_start();
        names.push((pid, basename(path)));
        // "/Applications/Visual Studio Code.app/..." is also "Visual Studio Code".
        if let Some(app) = path.split('/').find_map(|part| part.strip_suffix(".app")) {
            names.push((pid, app.to_string()));
        }
    }
    Ok(names)
}

/// Launch an app through LaunchServices, reporting a failed launcher rather
/// than merely accepting that an `open` child process was created.
#[cfg(target_os = "macos")]
pub fn open_application(command: &str) -> std::io::Result<()> {
    let output = std::process::Command::new("/usr/bin/open")
        .arg(command)
        .output()?;
    if output.status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other(format!(
            "couldn't open the application: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )))
    }
}

#[cfg(target_os = "macos")]
fn app_target_matches(command: &str, name: &str, bundle: Option<&str>, running_name: &str) -> bool {
    let command = command.trim();
    let command = command
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .unwrap_or(command);
    let target = std::path::Path::new(command);
    if target.extension().is_some_and(|ext| ext == "app") {
        // Two installed copies can share both their name and bundle id.
        let Some(bundle) = bundle else { return false };
        let canonical =
            |path: &std::path::Path| path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        return canonical(target) == canonical(std::path::Path::new(bundle));
    }
    !name.trim().is_empty() && name.trim().eq_ignore_ascii_case(running_name)
}

/// Close the selected application. It is asked to quit first, then forced
/// after a short grace period, so an unsaved-changes prompt can't keep it open
/// (Windows force-closes and Linux sends SIGTERM). NSRunningApplication is
/// thread-safe and needs no Apple Events or Automation grant.
#[cfg(target_os = "macos")]
pub fn close_application(command: &str, name: &str) -> std::io::Result<()> {
    use objc2_app_kit::NSWorkspace;
    const GRACE: std::time::Duration = std::time::Duration::from_secs(3);
    let targets = objc2::rc::autoreleasepool(|_| {
        let mut targets = Vec::new();
        for app in NSWorkspace::sharedWorkspace().runningApplications().iter() {
            let path = app
                .bundleURL()
                .and_then(|url| url.path())
                .map(|path| path.to_string());
            let running_name = app
                .localizedName()
                .map(|name| name.to_string())
                .unwrap_or_default();
            if app_target_matches(command, name, path.as_deref(), &running_name) {
                app.terminate();
                targets.push((app.processIdentifier(), app, running_name));
            }
        }
        targets
    });
    // isTerminated is only refreshed on the main run loop, so poll the PID.
    let alive = |pid: i32| pid > 0 && unsafe { libc::kill(pid, 0) } == 0;
    let deadline = std::time::Instant::now() + GRACE;
    while targets.iter().any(|(pid, ..)| alive(*pid)) && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    for (pid, app, running_name) in &targets {
        if alive(*pid) && !app.forceTerminate() && alive(*pid) {
            return Err(std::io::Error::other(format!(
                "couldn't close {running_name}"
            )));
        }
    }
    Ok(())
}

#[cfg(windows)]
fn process_list() -> Result<Vec<(u32, String)>, String> {
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
        TH32CS_SNAPPROCESS,
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
            let len = entry
                .szExeFile
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(entry.szExeFile.len());
            let name = String::from_utf16_lossy(&entry.szExeFile[..len]);
            names.push((entry.th32ProcessID, strip_exe(&name).to_string()));
            more = Process32NextW(snapshot, &mut entry) != 0;
        }
        CloseHandle(snapshot);
    }
    Ok(names)
}

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
fn process_list() -> Result<Vec<(u32, String)>, String> {
    Err("listing programs isn't supported on this system".into())
}

#[cfg_attr(windows, allow(dead_code))]
fn basename(path: &str) -> String {
    let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
    strip_exe(name).to_string()
}

fn strip_exe(name: &str) -> &str {
    name.strip_suffix(".exe")
        .or_else(|| name.strip_suffix(".EXE"))
        .unwrap_or(name)
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
    let mut info = LASTINPUTINFO {
        cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
        dwTime: 0,
    };
    if unsafe { GetLastInputInfo(&mut info) } == 0 {
        return None;
    }
    Some(Duration::from_millis(
        unsafe { GetTickCount() }.wrapping_sub(info.dwTime) as u64,
    ))
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
        (
            "org.freedesktop.ScreenSaver",
            "/org/freedesktop/ScreenSaver",
        ),
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
    Ok(process_names()?
        .iter()
        .any(|n| n.eq_ignore_ascii_case("LogonUI")))
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
    let session: CFDictionary<CFString, CFType> =
        unsafe { CFDictionary::wrap_under_create_rule(raw) };
    let key = CFString::from_static_string("CGSSessionScreenIsLocked");
    Ok(session
        .find(&key)
        .and_then(|v| v.downcast::<CFBoolean>())
        .is_some_and(bool::from))
}

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
pub fn screen_locked() -> Result<bool, String> {
    Err("lock screen detection isn't supported on this system".into())
}

/// Whether there's a route to the internet (a default route on some
/// interface other than loopback). Doesn't check that a site answers.
#[cfg(target_os = "linux")]
pub fn network_connected() -> Result<bool, String> {
    let v4 = std::fs::read_to_string("/proc/net/route")
        .map_err(|e| format!("can't read the routing table: {e}"))?;
    // Iface Destination Gateway Flags ...; flag 1 is "up".
    let default_v4 = v4.lines().skip(1).any(|line| {
        let f: Vec<&str> = line.split_whitespace().collect();
        f.len() > 3
            && f[0] != "lo"
            && f[1] == "00000000"
            && u32::from_str_radix(f[3], 16).is_ok_and(|flags| flags & 1 != 0)
    });
    // Destination, prefix length, ..., interface last.
    let default_v6 = std::fs::read_to_string("/proc/net/ipv6_route").is_ok_and(|v6| {
        v6.lines().any(|line| {
            let f: Vec<&str> = line.split_whitespace().collect();
            f.len() >= 10
                && f[0].bytes().all(|b| b == b'0')
                && f[1] == "00"
                && f[9] != "lo"
                && f[4].bytes().any(|b| b != b'0')
        })
    });
    Ok(default_v4 || default_v6)
}

#[cfg(windows)]
pub fn network_connected() -> Result<bool, String> {
    use windows_sys::Win32::NetworkManagement::IpHelper::GetBestInterface;
    let mut index = 0u32;
    // Asks for the interface that would carry traffic to 8.8.8.8; it fails without a route.
    let dest = u32::from_ne_bytes([8, 8, 8, 8]);
    Ok(unsafe { GetBestInterface(dest, &mut index) } == 0)
}

#[cfg(target_os = "macos")]
pub fn network_connected() -> Result<bool, String> {
    let has_route = |family: &[&str]| {
        std::process::Command::new("route")
            .args(["-n", "get"])
            .args(family)
            .arg("default")
            .output()
            .is_ok_and(|out| {
                out.status.success() && String::from_utf8_lossy(&out.stdout).contains("interface:")
            })
    };
    Ok(has_route(&[]) || has_route(&["-inet6"]))
}

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
pub fn network_connected() -> Result<bool, String> {
    Err("network detection isn't supported on this system".into())
}

/// A plugged-in USB device.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct UsbDevice {
    /// Stable while it stays plugged in.
    pub id: String,
    /// Maker and product, as the device reports them.
    pub name: String,
    /// The maker alone; empty when the device doesn't say.
    pub maker: String,
    /// `vvvv:pppp` vendor and product ids, in hex.
    pub ids: String,
}

impl UsbDevice {
    /// Matches name or ids, ignoring case; an empty pattern matches any device.
    pub fn matches(&self, pattern: &str) -> bool {
        let pattern = pattern.trim().to_lowercase();
        pattern.is_empty()
            || self.name.to_lowercase().contains(&pattern)
            || self.ids.contains(&pattern)
    }

    fn named(id: String, maker: &str, product: &str, vendor: u32, product_id: u32) -> UsbDevice {
        let ids = format!("{vendor:04x}:{product_id:04x}");
        let name = match (maker.trim(), product.trim()) {
            ("", "") => format!("USB device {ids}"),
            (m, "") => m.to_string(),
            (m, p) if p.to_lowercase().contains(&m.to_lowercase()) => p.to_string(),
            ("", p) => p.to_string(),
            (m, p) => format!("{m} {p}"),
        };
        UsbDevice {
            id,
            name,
            maker: maker.trim().to_string(),
            ids,
        }
    }
}

/// USB devices plugged in right now, not counting hubs.
#[cfg(target_os = "linux")]
pub fn usb_devices() -> Result<Vec<UsbDevice>, String> {
    let dir = std::fs::read_dir("/sys/bus/usb/devices")
        .map_err(|e| format!("can't list USB devices: {e}"))?;
    let mut devices = Vec::new();
    for entry in dir.flatten() {
        let path = entry.path();
        let read = |file: &str| {
            std::fs::read_to_string(path.join(file))
                .map(|s| s.trim().to_string())
                .unwrap_or_default()
        };
        let hex = |file: &str| u32::from_str_radix(&read(file), 16).ok();
        let (Some(vendor), Some(product)) = (hex("idVendor"), hex("idProduct")) else {
            continue;
        };
        // Class 09 is a hub, root hubs included.
        if read("bDeviceClass") == "09" {
            continue;
        }
        let id = format!(
            "{}/{vendor:04x}:{product:04x}",
            entry.file_name().to_string_lossy()
        );
        devices.push(UsbDevice::named(
            id,
            &read("manufacturer"),
            &read("product"),
            vendor,
            product,
        ));
    }
    devices.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(devices)
}

#[cfg(windows)]
pub fn usb_devices() -> Result<Vec<UsbDevice>, String> {
    use windows_sys::Win32::Devices::DeviceAndDriverInstallation::{
        DIGCF_ALLCLASSES, DIGCF_PRESENT, SP_DEVINFO_DATA, SPDRP_COMPATIBLEIDS, SPDRP_DEVICEDESC,
        SPDRP_FRIENDLYNAME, SPDRP_MFG, SPDRP_SERVICE, SetupDiDestroyDeviceInfoList,
        SetupDiEnumDeviceInfo, SetupDiGetClassDevsW, SetupDiGetDeviceInstanceIdW,
        SetupDiGetDevicePropertyW, SetupDiGetDeviceRegistryPropertyW,
    };
    use windows_sys::Win32::Devices::Properties::DEVPKEY_Device_BusReportedDeviceDesc;
    use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;

    let from_wide = |buf: &[u16]| {
        String::from_utf16_lossy(&buf[..buf.iter().position(|&c| c == 0).unwrap_or(buf.len())])
    };
    let enumerator: Vec<u16> = "USB\0".encode_utf16().collect();
    // SAFETY: the list is destroyed below; every buffer outlives its call.
    unsafe {
        let list = SetupDiGetClassDevsW(
            std::ptr::null(),
            enumerator.as_ptr(),
            std::ptr::null_mut(),
            DIGCF_ALLCLASSES | DIGCF_PRESENT,
        );
        if list as isize == INVALID_HANDLE_VALUE as isize {
            return Err("can't list USB devices".into());
        }
        let mut devices = Vec::new();
        let mut index = 0;
        loop {
            let mut info: SP_DEVINFO_DATA = std::mem::zeroed();
            info.cbSize = std::mem::size_of::<SP_DEVINFO_DATA>() as u32;
            if SetupDiEnumDeviceInfo(list, index, &mut info) == 0 {
                break;
            }
            index += 1;
            let mut id_buf = [0u16; 512];
            if SetupDiGetDeviceInstanceIdW(
                list,
                &info,
                id_buf.as_mut_ptr(),
                id_buf.len() as u32,
                std::ptr::null_mut(),
            ) == 0
            {
                continue;
            }
            let instance = from_wide(&id_buf);
            let upper = instance.to_uppercase();
            if upper.contains("ROOT_HUB") || upper.contains("&MI_") {
                continue;
            }
            let raw_property = |key| {
                let mut buf = [0u16; 1024];
                let ok = SetupDiGetDeviceRegistryPropertyW(
                    list,
                    &info,
                    key,
                    std::ptr::null_mut(),
                    buf.as_mut_ptr().cast(),
                    (buf.len() * 2) as u32,
                    std::ptr::null_mut(),
                );
                (ok != 0).then_some(buf)
            };
            let property = |key| {
                raw_property(key)
                    .map(|buf| from_wide(&buf))
                    .unwrap_or_default()
            };
            // Hubs: class 09, or USB\USB20_HUB style ids on the USB 3 stack,
            // or the hub driver. Compatible ids are a list of strings.
            let is_hub = raw_property(SPDRP_COMPATIBLEIDS).is_some_and(|buf| {
                String::from_utf16_lossy(&buf)
                    .split('\0')
                    .map(str::to_uppercase)
                    .any(|id| id == "USB\\CLASS_09" || id.ends_with("_HUB"))
            }) || property(SPDRP_SERVICE).to_lowercase().starts_with("usbhub");
            if is_hub {
                continue;
            }
            // The product string the device reports itself; the driver's
            // name is often just "USB Composite Device".
            let reported = {
                let mut buf = [0u16; 512];
                let mut kind = 0u32;
                let ok = SetupDiGetDevicePropertyW(
                    list,
                    &info,
                    &DEVPKEY_Device_BusReportedDeviceDesc,
                    &mut kind,
                    buf.as_mut_ptr().cast(),
                    (buf.len() * 2) as u32,
                    std::ptr::null_mut(),
                    0,
                );
                if ok == 0 {
                    String::new()
                } else {
                    from_wide(&buf)
                }
            };
            let hex_after = |tag: &str| {
                upper
                    .find(tag)
                    .and_then(|at| {
                        u32::from_str_radix(upper.get(at + tag.len()..at + tag.len() + 4)?, 16).ok()
                    })
                    .unwrap_or(0)
            };
            let product = [
                reported,
                property(SPDRP_FRIENDLYNAME),
                property(SPDRP_DEVICEDESC),
            ]
            .into_iter()
            .find(|p| !p.trim().is_empty())
            .unwrap_or_default();
            let maker = property(SPDRP_MFG);
            // "(Standard USB Host Controller)" style makers say nothing.
            let maker = if maker.starts_with('(') {
                String::new()
            } else {
                maker
            };
            devices.push(UsbDevice::named(
                instance,
                &maker,
                &product,
                hex_after("VID_"),
                hex_after("PID_"),
            ));
        }
        SetupDiDestroyDeviceInfoList(list);
        devices.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(devices)
    }
}

#[cfg(target_os = "macos")]
pub fn usb_devices() -> Result<Vec<UsbDevice>, String> {
    let out = std::process::Command::new("ioreg")
        .args(["-p", "IOUSB", "-l", "-w0"])
        .output()
        .map_err(|e| format!("can't list USB devices: {e}"))?;
    Ok(parse_ioreg(&String::from_utf8_lossy(&out.stdout)))
}

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
pub fn usb_devices() -> Result<Vec<UsbDevice>, String> {
    Err("USB detection isn't supported on this system".into())
}

/// Devices in `ioreg -p IOUSB -l` output.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn parse_ioreg(text: &str) -> Vec<UsbDevice> {
    struct Pending {
        name: String,
        location: String,
        maker: String,
        vendor: u32,
        product: u32,
    }
    let finish = |p: Pending, out: &mut Vec<UsbDevice>| {
        let id = format!("{}/{:04x}:{:04x}", p.location, p.vendor, p.product);
        out.push(UsbDevice::named(id, &p.maker, &p.name, p.vendor, p.product));
    };
    let mut devices = Vec::new();
    let mut current: Option<Pending> = None;
    for line in text.lines() {
        if let Some(at) = line.find("+-o ") {
            if let Some(done) = current.take() {
                finish(done, &mut devices);
            }
            let rest = &line[at + 4..];
            let is_device =
                rest.contains("<class IOUSBHostDevice") || rest.contains("<class IOUSBDevice");
            if let (true, Some((name, after))) = (is_device, rest.split_once('@')) {
                let location = after
                    .split_whitespace()
                    .next()
                    .unwrap_or_default()
                    .to_string();
                current = Some(Pending {
                    name: name.trim().to_string(),
                    location,
                    maker: String::new(),
                    vendor: 0,
                    product: 0,
                });
            }
            continue;
        }
        let Some(p) = current.as_mut() else { continue };
        let Some((key, value)) = line.split_once(" = ") else {
            continue;
        };
        let key = key.trim_start_matches(['|', ' ']).trim().trim_matches('"');
        let value = value.trim();
        match key {
            "USB Vendor Name" => p.maker = value.trim_matches('"').to_string(),
            "idVendor" => p.vendor = value.parse().unwrap_or(0),
            "idProduct" => p.product = value.parse().unwrap_or(0),
            _ => {}
        }
    }
    if let Some(done) = current.take() {
        finish(done, &mut devices);
    }
    devices
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "macos")]
    #[test]
    fn close_app_prefers_the_selected_bundle_over_another_copy_with_the_same_name() {
        assert!(app_target_matches(
            "/Applications/Example.app",
            "Example",
            Some("/Applications/Example.app"),
            "Example"
        ));
        assert!(!app_target_matches(
            "/Applications/Example.app",
            "Example",
            Some("/Users/test/Example.app"),
            "Example"
        ));
        assert!(app_target_matches(
            "\"/Applications/Example.app\"",
            "Example",
            Some("/Applications/Example.app"),
            "Example"
        ));
        assert!(!app_target_matches(
            "/Applications/Example.app",
            "Example",
            None,
            "Example"
        ));
        assert!(app_target_matches("", "Example", None, "example"));
        assert!(!app_target_matches("", "", None, "Example"));
    }

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
        println!("network: {:?}", network_connected());
        println!("usb: {:#?}", usb_devices());
        assert!(app_running("cargo").unwrap());
    }

    #[test]
    fn names_usb_devices_and_reads_ioreg() {
        let d = UsbDevice::named("3-2".into(), "Logitech", "USB Receiver", 0x046d, 0xc52b);
        assert_eq!(
            (d.name.as_str(), d.maker.as_str(), d.ids.as_str()),
            ("Logitech USB Receiver", "Logitech", "046d:c52b")
        );
        assert!(
            d.matches("receiver")
                && d.matches("046D:C52B")
                && d.matches("")
                && !d.matches("keyboard")
        );
        assert_eq!(
            UsbDevice::named("x".into(), "Lenovo", "Lenovo Camera", 1, 2).name,
            "Lenovo Camera"
        );
        assert_eq!(
            UsbDevice::named("x".into(), "", "", 1, 2).name,
            "USB device 0001:0002"
        );
        let ioreg = r#"+-o Root  <class IORegistryEntry, id 0x100000100, retain 26>
  +-o AppleT8103USBXHCI@00000000  <class AppleT8103USBXHCI, id 0x1000002c5>
  | +-o USB Receiver@00100000  <class IOUSBHostDevice, id 0x100000a1b, registered>
  | |   {
  | |     "idProduct" = 50475
  | |     "USB Vendor Name" = "Logitech"
  | |     "idVendor" = 1133
  | |   }
  | +-o Keyboard@00200000  <class IOUSBHostDevice, id 0x100000a1c>
  | |     "idVendor" = 1452
"#;
        let devices = parse_ioreg(ioreg);
        assert_eq!(devices.len(), 2);
        assert_eq!(devices[0].name, "Logitech USB Receiver");
        assert_eq!(devices[0].ids, "046d:c52b");
        assert_eq!(devices[1].id, "00200000/05ac:0000");
    }

    #[test]
    fn lists_this_test_program() {
        let names = running_apps().unwrap();
        assert!(!names.is_empty());
    }
}
