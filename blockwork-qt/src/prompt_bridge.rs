//! `blockwork --prompt`: one macro dialog in its own window, started by the
//! daemon. The request arrives as JSON on stdin and the answer leaves as
//! JSON on stdout.

use std::io::{Read, Write};
use std::sync::OnceLock;

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(QString, request_json, cxx_name = "requestJson")]
        type PromptBridge = super::PromptBridgeRust;

        /// Prints the answer and quits.
        #[qinvokable]
        fn finish(self: &PromptBridge, answer: &QString, text: &QString);

        /// Takes keyboard focus from whatever app has it. The daemon starts
        /// this process in the background, and Windows keeps those behind.
        #[qinvokable]
        #[cxx_name = "bringToFront"]
        fn bring_to_front(self: &PromptBridge);

        #[qinvokable]
        #[cxx_name = "parentAlive"]
        fn parent_alive(self: &PromptBridge) -> bool;
    }
}

static REQUEST: OnceLock<String> = OnceLock::new();
#[cfg(unix)]
static PARENT: OnceLock<libc::pid_t> = OnceLock::new();

/// Whether this launch is a prompt; reads the request if so. Call before Qt starts.
pub fn requested() -> bool {
    if std::env::args().nth(1).as_deref() != Some("--prompt") {
        return false;
    }
    #[cfg(unix)]
    PARENT.get_or_init(|| std::env::var("BLOCKWORK_PROMPT_PARENT_PID")
        .ok().and_then(|value| value.parse().ok())
        .unwrap_or_else(|| unsafe { libc::getppid() }));
    let mut text = String::new();
    let _ = std::io::stdin().read_to_string(&mut text);
    let _ = REQUEST.set(text);
    true
}

/// The dialog's language from the request; empty means the system's.
pub fn language() -> String {
    REQUEST
        .get()
        .and_then(|text| serde_json::from_str::<serde_json::Value>(text).ok())
        .and_then(|v| v.get("language")?.as_str().map(str::to_string))
        .unwrap_or_default()
}

pub struct PromptBridgeRust {
    request_json: cxx_qt_lib::QString,
}

impl Default for PromptBridgeRust {
    fn default() -> Self {
        Self { request_json: cxx_qt_lib::QString::from(REQUEST.get().map(String::as_str).unwrap_or("{}")) }
    }
}

impl qobject::PromptBridge {
    fn finish(&self, answer: &cxx_qt_lib::QString, text: &cxx_qt_lib::QString) {
        let reply = serde_json::json!({ "answer": answer.to_string(), "text": text.to_string() });
        let mut out = std::io::stdout().lock();
        let _ = writeln!(out, "{reply}");
        let _ = out.flush();
        std::process::exit(0);
    }

    fn parent_alive(&self) -> bool {
        #[cfg(unix)]
        if let Some(parent) = PARENT.get() {
            // Reparenting also covers a parent that died before this dialog
            // finished launching, without relying on PID liveness/reuse.
            return unsafe { libc::getppid() } == *parent;
        }
        true
    }

    fn bring_to_front(&self) {
        #[cfg(windows)]
        front::bring_to_front();
    }
}

#[cfg(windows)]
mod front {
    use windows_sys::Win32::Foundation::{HWND, LPARAM};
    use windows_sys::Win32::System::Threading::{AttachThreadInput, GetCurrentProcessId, GetCurrentThreadId};
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, SendInput};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        BringWindowToTop, EnumWindows, GetForegroundWindow, GetWindowThreadProcessId, IsWindowVisible, SetForegroundWindow,
    };

    /// Windows only lets the process that sent the last input take focus, so
    /// send a key no keyboard has (0xE8 is unassigned) first, and borrow the
    /// foreground window's input queue for the switch.
    pub(super) fn bring_to_front() {
        unsafe extern "system" fn first_own(hwnd: HWND, out: LPARAM) -> i32 {
            let mut pid = 0;
            // SAFETY: `out` is the HWND passed to EnumWindows below.
            unsafe {
                GetWindowThreadProcessId(hwnd, &mut pid);
                if pid == GetCurrentProcessId() && IsWindowVisible(hwnd) != 0 {
                    *(out as *mut HWND) = hwnd;
                    return 0;
                }
            }
            1
        }
        // SAFETY: plain Win32 calls; the callback only writes `hwnd`, and the
        // thread input is detached again before returning.
        unsafe {
            let mut hwnd: HWND = std::ptr::null_mut();
            EnumWindows(Some(first_own), &mut hwnd as *mut HWND as LPARAM);
            if hwnd.is_null() || GetForegroundWindow() == hwnd {
                return;
            }
            let key = |flags| INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: 0xE8, wScan: 0, dwFlags: flags, time: 0, dwExtraInfo: 0 } },
            };
            let taps = [key(0), key(KEYEVENTF_KEYUP)];
            SendInput(taps.len() as u32, taps.as_ptr(), std::mem::size_of::<INPUT>() as i32);
            let theirs = GetWindowThreadProcessId(GetForegroundWindow(), std::ptr::null_mut());
            let ours = GetCurrentThreadId();
            let attached = theirs != 0 && theirs != ours && AttachThreadInput(ours, theirs, 1) != 0;
            BringWindowToTop(hwnd);
            SetForegroundWindow(hwnd);
            if attached {
                AttachThreadInput(ours, theirs, 0);
            }
        }
    }
}
