use super::{Action, ActiveWindow, MatchBy, Rect, no_match};
use std::time::{Duration, Instant};
use windows_sys::Win32::Foundation::{CloseHandle, HWND, LPARAM, POINT, RECT};
use windows_sys::Win32::Graphics::Dwm::{
    DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS, DwmGetWindowAttribute,
};
use windows_sys::Win32::Graphics::Gdi::ClientToScreen;
use windows_sys::Win32::System::Threading::{
    AttachThreadInput, GetCurrentThreadId, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    QueryFullProcessImageNameW,
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, SendInput,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    BringWindowToTop, EnumChildWindows, EnumWindows, GA_ROOTOWNER, GWL_EXSTYLE, GetAncestor,
    GetClientRect, GetForegroundWindow, GetWindowLongW, GetWindowRect, GetWindowTextW,
    GetWindowThreadProcessId, IsIconic, IsWindowVisible, IsZoomed, PostMessageW, SW_MAXIMIZE,
    SW_MINIMIZE, SW_RESTORE, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER,
    SetForegroundWindow, SetWindowPos, ShowWindow, WM_CLOSE, WS_EX_TOOLWINDOW,
};

/// Why Windows refuses to touch a window, almost always.
const ELEVATED: &str = "Windows blocked it; the window may belong to a program running as administrator, \
                        which Blockwork can only control when it runs as administrator too";

/// The executable name (without `.exe`) of the process owning `hwnd`.
fn process_name(hwnd: HWND) -> String {
    // SAFETY: plain Win32 queries; the buffer's length is passed with it
    // and the process handle is closed below.
    unsafe {
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, &mut pid);
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if process.is_null() {
            return String::new();
        }
        let mut path = [0u16; 1024];
        let mut size = path.len() as u32;
        let mut name = String::new();
        if QueryFullProcessImageNameW(process, 0, path.as_mut_ptr(), &mut size) != 0 {
            let full = String::from_utf16_lossy(&path[..size as usize]);
            name = std::path::Path::new(&full)
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or(full);
        }
        CloseHandle(process);
        name
    }
}

/// Store apps draw inside an ApplicationFrameHost window; the app is the
/// process behind its child window.
fn hosted_app(hwnd: HWND) -> Option<String> {
    unsafe extern "system" fn first_foreign(child: HWND, out: LPARAM) -> i32 {
        let name = process_name(child);
        if name.is_empty() || name.eq_ignore_ascii_case("ApplicationFrameHost") {
            return 1;
        }
        // SAFETY: `out` is the Option passed to EnumChildWindows below,
        // alive for the whole enumeration.
        unsafe { *(out as *mut Option<String>) = Some(name) };
        0
    }
    let mut out: Option<String> = None;
    // SAFETY: the callback only writes `out`, which outlives the call.
    unsafe {
        EnumChildWindows(
            hwnd,
            Some(first_foreign),
            &mut out as *mut Option<String> as LPARAM,
        )
    };
    out
}

fn info(hwnd: HWND) -> ActiveWindow {
    let mut title = [0u16; 512];
    // SAFETY: the buffer's length is passed alongside it.
    let len =
        unsafe { GetWindowTextW(hwnd, title.as_mut_ptr(), title.len() as i32) }.max(0) as usize;
    let title = String::from_utf16_lossy(&title[..len]);
    let mut app = process_name(hwnd);
    if app.eq_ignore_ascii_case("ApplicationFrameHost")
        && let Some(hosted) = hosted_app(hwnd)
    {
        app = hosted;
    }
    ActiveWindow { title, app }
}

pub fn active() -> Result<ActiveWindow, String> {
    // SAFETY: no arguments; may return null.
    let hwnd = unsafe { GetForegroundWindow() };
    Ok(if hwnd.is_null() {
        ActiveWindow::default()
    } else {
        info(hwnd)
    })
}

/// Whether `hwnd` is a window the user can see in the taskbar or
/// Alt+Tab: visible, not cloaked (suspended Store apps and windows on
/// other virtual desktops are), not a tool window, not owned.
fn is_app_window(hwnd: HWND) -> bool {
    // SAFETY: plain Win32 queries on a handle EnumWindows gave us;
    // `cloaked` is a plain out-parameter of the size passed.
    unsafe {
        if IsWindowVisible(hwnd) == 0 || GetAncestor(hwnd, GA_ROOTOWNER) != hwnd {
            return false;
        }
        if GetWindowLongW(hwnd, GWL_EXSTYLE) as u32 & WS_EX_TOOLWINDOW != 0 {
            return false;
        }
        let mut cloaked = 0u32;
        let ok = DwmGetWindowAttribute(
            hwnd,
            DWMWA_CLOAKED as u32,
            (&mut cloaked as *mut u32).cast(),
            4,
        ) >= 0;
        !(ok && cloaked != 0)
    }
}

/// The user's top-level windows, front to back.
fn windows() -> Vec<HWND> {
    unsafe extern "system" fn collect(hwnd: HWND, out: LPARAM) -> i32 {
        if is_app_window(hwnd) {
            // SAFETY: `out` is the Vec passed to EnumWindows below, alive
            // for the whole enumeration.
            unsafe { (*(out as *mut Vec<HWND>)).push(hwnd) };
        }
        1
    }
    let mut out: Vec<HWND> = Vec::new();
    // SAFETY: the callback only pushes into `out`, which outlives the call.
    unsafe { EnumWindows(Some(collect), &mut out as *mut Vec<HWND> as LPARAM) };
    out
}

fn find(pattern: &str, by: MatchBy) -> Result<HWND, String> {
    let pattern = pattern.trim();
    if pattern.is_empty() {
        // SAFETY: no arguments; may return null.
        let hwnd = unsafe { GetForegroundWindow() };
        return if hwnd.is_null() {
            Err("no window is focused".into())
        } else {
            Ok(hwnd)
        };
    }
    windows()
        .into_iter()
        .find(|w| {
            let info = info(*w);
            !info.title.is_empty() && info.matches(pattern, by)
        })
        .ok_or_else(|| no_match(pattern))
}

/// Brings `hwnd` to the front. Windows only lets the process the user
/// last interacted with take focus, so borrow the foreground window's
/// input queue for the call when asking plainly isn't enough.
fn focus(hwnd: HWND) -> Result<(), String> {
    // SAFETY: Win32 window and thread calls; the thread input is
    // detached again before returning.
    unsafe {
        if IsIconic(hwnd) != 0 {
            ShowWindow(hwnd, SW_RESTORE);
        }
        if SetForegroundWindow(hwnd) == 0 || GetForegroundWindow() != hwnd {
            // The process that sent the last input may switch focus, so
            // send a key no keyboard has (0xE8 is unassigned).
            let key = |flags| INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: 0xE8,
                        wScan: 0,
                        dwFlags: flags,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            };
            let taps = [key(0), key(KEYEVENTF_KEYUP)];
            SendInput(
                taps.len() as u32,
                taps.as_ptr(),
                std::mem::size_of::<INPUT>() as i32,
            );
            let foreground = GetForegroundWindow();
            let theirs = GetWindowThreadProcessId(foreground, std::ptr::null_mut());
            let ours = GetCurrentThreadId();
            let attached = theirs != 0 && theirs != ours && AttachThreadInput(ours, theirs, 1) != 0;
            BringWindowToTop(hwnd);
            SetForegroundWindow(hwnd);
            if attached {
                AttachThreadInput(ours, theirs, 0);
            }
        }
        // The switch can land a moment later.
        let deadline = Instant::now() + Duration::from_millis(200);
        while GetForegroundWindow() != hwnd {
            if Instant::now() > deadline {
                return Err("Windows refused to bring the window to the front".into());
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    Ok(())
}

pub fn act(action: Action, pattern: &str, by: MatchBy) -> Result<(), String> {
    let hwnd = find(pattern, by)?;
    if action == Action::Focus {
        return focus(hwnd);
    }
    // SAFETY: Win32 window calls on a handle EnumWindows/GetForegroundWindow gave us.
    unsafe {
        match action {
            Action::Minimize => ShowWindow(hwnd, SW_MINIMIZE),
            Action::Maximize => ShowWindow(hwnd, SW_MAXIMIZE),
            Action::Restore => ShowWindow(hwnd, SW_RESTORE),
            Action::Close => {
                if PostMessageW(hwnd, WM_CLOSE, 0, 0) == 0 {
                    return Err(format!("couldn't close the window: {ELEVATED}"));
                }
                0
            }
            Action::Focus => unreachable!(),
        };
    }
    Ok(())
}

/// How far the visible frame sits inside the window rectangle on each
/// side (left, top, right, bottom). Windows 10 and later give windows
/// invisible resize borders, so without this a window moved to 0,0
/// shows a gap.
fn invisible_border(hwnd: HWND) -> (i32, i32, i32, i32) {
    // SAFETY: `rect` and `frame` are plain out-parameters of the size passed.
    unsafe {
        let mut rect: RECT = std::mem::zeroed();
        let mut frame: RECT = std::mem::zeroed();
        let size = std::mem::size_of::<RECT>() as u32;
        if GetWindowRect(hwnd, &mut rect) == 0
            || DwmGetWindowAttribute(
                hwnd,
                DWMWA_EXTENDED_FRAME_BOUNDS as u32,
                (&mut frame as *mut RECT).cast(),
                size,
            ) < 0
        {
            return (0, 0, 0, 0);
        }
        let border = (
            frame.left - rect.left,
            frame.top - rect.top,
            rect.right - frame.right,
            rect.bottom - frame.bottom,
        );
        // A bogus answer (another DPI context, mid-animation) is worse than none.
        if [border.0, border.1, border.2, border.3]
            .iter()
            .all(|b| (0..=32).contains(b))
        {
            border
        } else {
            (0, 0, 0, 0)
        }
    }
}

pub fn place(pattern: &str, by: MatchBy, resize: bool, a: i32, b: i32) -> Result<(), String> {
    let hwnd = find(pattern, by)?;
    // SAFETY: as in `act`.
    unsafe {
        if IsZoomed(hwnd) != 0 || IsIconic(hwnd) != 0 {
            ShowWindow(hwnd, SW_RESTORE);
        }
        let (left, top, right, bottom) = invisible_border(hwnd);
        let (x, y, width, height) = if resize {
            (0, 0, a.max(1) + left + right, b.max(1) + top + bottom)
        } else {
            (a - left, b - top, 0, 0)
        };
        let flags = SWP_NOZORDER | SWP_NOACTIVATE | if resize { SWP_NOMOVE } else { SWP_NOSIZE };
        if SetWindowPos(hwnd, std::ptr::null_mut(), x, y, width, height, flags) == 0 {
            return Err(format!("couldn't move the window: {ELEVATED}"));
        }
    }
    Ok(())
}

/// The client area, so title bars and borders don't shift the center.
pub fn bounds(pattern: &str, by: MatchBy) -> Result<Rect, String> {
    let hwnd = find(pattern, by)?;
    // SAFETY: as in `act`; `rect` and `origin` are plain out-parameters.
    unsafe {
        if IsIconic(hwnd) != 0 {
            return Err("the window is minimized".into());
        }
        let mut rect: RECT = std::mem::zeroed();
        let mut origin = POINT { x: 0, y: 0 };
        if GetClientRect(hwnd, &mut rect) == 0 || ClientToScreen(hwnd, &mut origin) == 0 {
            return Err("couldn't read the window's position".into());
        }
        let (width, height) = (
            (rect.right - rect.left) as f64,
            (rect.bottom - rect.top) as f64,
        );
        if width < 1.0 || height < 1.0 {
            return Err("the window is minimized or hidden".into());
        }
        Ok(Rect {
            x: origin.x as f64,
            y: origin.y as f64,
            width,
            height,
        })
    }
}

pub fn exists(pattern: &str) -> Result<bool, String> {
    Ok(pattern.trim().is_empty() || find(pattern, MatchBy::Any).is_ok())
}
