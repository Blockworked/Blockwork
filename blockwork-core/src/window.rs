//! Window info and control: the active-window reporters, the `WhenWindow`
//! trigger and the window action blocks. Wayland has no common API for
//! this, so Linux asks the compositor directly where it can (Hyprland, Sway,
//! KDE via kdotool) and falls back to X11 off Wayland.

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ActiveWindow {
    pub title: String,
    /// App or process name (window class on Linux).
    pub app: String,
}

/// Which part of a window a pattern is matched against.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MatchBy {
    /// The title or the app.
    #[default]
    Any,
    Title,
    App,
}

impl std::str::FromStr for MatchBy {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "" | "Any" => Ok(MatchBy::Any),
            "Title" => Ok(MatchBy::Title),
            "App" => Ok(MatchBy::App),
            _ => Err(format!("unknown window match '{s}'")),
        }
    }
}

impl ActiveWindow {
    /// Case-insensitive substring match on the title and/or app; an empty
    /// pattern matches any window.
    pub fn matches(&self, pattern: &str, by: MatchBy) -> bool {
        let pattern = pattern.trim().to_lowercase();
        pattern.is_empty()
            || (by != MatchBy::App && self.title.to_lowercase().contains(&pattern))
            || (by != MatchBy::Title && self.app.to_lowercase().contains(&pattern))
    }
}

/// What the `WindowAction` block can do, by its saved name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Focus,
    Minimize,
    Maximize,
    Restore,
    Close,
}

pub const ACTIONS: [&str; 5] = ["Focus", "Minimize", "Maximize", "Restore", "Close"];

impl std::str::FromStr for Action {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "Focus" => Ok(Action::Focus),
            "Minimize" => Ok(Action::Minimize),
            "Maximize" => Ok(Action::Maximize),
            "Restore" => Ok(Action::Restore),
            "Close" => Ok(Action::Close),
            _ => Err(format!("unknown window action '{s}'")),
        }
    }
}

#[cfg(any(target_os = "linux", windows))]
fn no_match(pattern: &str) -> String {
    format!("no window matches '{}'", pattern.trim())
}

#[cfg(target_os = "linux")]
pub fn active() -> Result<ActiveWindow, String> {
    linux::active()
}

/// Runs `action` on the first window whose title or app (per `by`) contains
/// `pattern`, or on the active window when `pattern` is empty.
#[cfg(target_os = "linux")]
pub fn act(action: Action, pattern: &str, by: MatchBy) -> Result<(), String> {
    linux::act(action, pattern.trim(), by)
}

/// Moves a matching window to (`a`, `b`), or with `resize` sizes it to `a`
/// by `b` pixels.
#[cfg(target_os = "linux")]
pub fn place(pattern: &str, by: MatchBy, resize: bool, a: i32, b: i32) -> Result<(), String> {
    linux::place(pattern.trim(), by, resize, a, b)
}

/// Whether any window's title or app contains `pattern`.
#[cfg(target_os = "linux")]
pub fn exists(pattern: &str) -> Result<bool, String> {
    linux::exists(pattern.trim())
}

#[cfg(windows)]
pub use windows::{act, active, exists, place};

#[cfg(target_os = "macos")]
pub use macos::{act, active, exists, place};

#[cfg(not(any(target_os = "linux", windows, target_os = "macos")))]
mod unsupported {
    use super::{Action, ActiveWindow, MatchBy};
    const MSG: &str = "window info isn't supported on this platform";
    pub fn active() -> Result<ActiveWindow, String> {
        Err(MSG.into())
    }
    pub fn act(_: Action, _: &str, _: MatchBy) -> Result<(), String> {
        Err(MSG.into())
    }
    pub fn place(_: &str, _: MatchBy, _: bool, _: i32, _: i32) -> Result<(), String> {
        Err(MSG.into())
    }
    pub fn exists(_: &str) -> Result<bool, String> {
        Err(MSG.into())
    }
}
#[cfg(not(any(target_os = "linux", windows, target_os = "macos")))]
pub use unsupported::{act, active, exists, place};

#[cfg(target_os = "linux")]
mod linux {
    use super::{no_match, Action, ActiveWindow, MatchBy};
    use crate::flatpak::host_command;

    enum Desktop {
        Hyprland,
        Sway,
        Kde,
        X11,
    }

    fn desktop() -> Result<Desktop, String> {
        let env = |k: &str| std::env::var(k).unwrap_or_default();
        if !env("HYPRLAND_INSTANCE_SIGNATURE").is_empty() {
            return Ok(Desktop::Hyprland);
        }
        if !env("SWAYSOCK").is_empty() {
            return Ok(Desktop::Sway);
        }
        let wayland = !env("WAYLAND_DISPLAY").is_empty() || env("XDG_SESSION_TYPE") == "wayland";
        if !wayland {
            return Ok(Desktop::X11);
        }
        if env("XDG_CURRENT_DESKTOP").to_uppercase().contains("KDE") {
            return Ok(Desktop::Kde);
        }
        Err("window info isn't available on this Wayland desktop (supported: Hyprland, Sway, KDE with kdotool, or X11)".into())
    }

    pub(super) fn active() -> Result<ActiveWindow, String> {
        match desktop()? {
            Desktop::Hyprland => Ok(hypr::info(&hypr::active()?)),
            Desktop::Sway => Ok(sway::active()?.map(|n| sway::info(&n)).unwrap_or_default()),
            Desktop::Kde => kde::info(&kde::active()?),
            Desktop::X11 => {
                let x = x11::X11::connect()?;
                Ok(x.active()?.map(|w| x.info(w)).unwrap_or_default())
            }
        }
    }

    pub(super) fn act(action: Action, pattern: &str, by: MatchBy) -> Result<(), String> {
        match desktop()? {
            Desktop::Hyprland => hypr::act(action, &hypr::find(pattern, by)?),
            Desktop::Sway => sway::act(action, &sway::find(pattern, by)?),
            Desktop::Kde => kde::act(action, &kde::find(pattern, by)?),
            Desktop::X11 => {
                let x = x11::X11::connect()?;
                let w = x.find(pattern, by)?;
                x.act(action, w)
            }
        }
    }

    pub(super) fn place(pattern: &str, by: MatchBy, resize: bool, a: i32, b: i32) -> Result<(), String> {
        match desktop()? {
            Desktop::Hyprland => hypr::place(&hypr::find(pattern, by)?, resize, a, b),
            Desktop::Sway => sway::place(&sway::find(pattern, by)?, resize, a, b),
            Desktop::Kde => {
                let id = kde::find(pattern, by)?;
                let (a, b) = (a.to_string(), b.to_string());
                run("kdotool", &[if resize { "windowsize" } else { "windowmove" }, &id, &a, &b]).map(drop)
            }
            Desktop::X11 => {
                let x = x11::X11::connect()?;
                let w = x.find(pattern, by)?;
                x.place(w, resize, a, b)
            }
        }
    }

    pub(super) fn exists(pattern: &str) -> Result<bool, String> {
        if pattern.is_empty() {
            return Ok(true);
        }
        match desktop()? {
            Desktop::Hyprland => Ok(hypr::find(pattern, MatchBy::Any).is_ok()),
            Desktop::Sway => Ok(sway::find(pattern, MatchBy::Any).is_ok()),
            Desktop::Kde => Ok(kde::search(pattern, MatchBy::Any)?.is_some()),
            Desktop::X11 => Ok(x11::X11::connect()?.find(pattern, MatchBy::Any).is_ok()),
        }
    }

    fn run(program: &str, args: &[&str]) -> Result<String, String> {
        let output = host_command(program)
            .args(args)
            .output()
            .map_err(|e| format!("couldn't run {program}: {e}"))?;
        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            let err = err.trim();
            return Err(if err.is_empty() { format!("{program} failed") } else { format!("{program}: {err}") });
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim_end().to_string())
    }

    fn json(program: &str, args: &[&str]) -> Result<serde_json::Value, String> {
        serde_json::from_str(&run(program, args)?).map_err(|e| format!("unexpected {program} output: {e}"))
    }

    fn text(node: &serde_json::Value, key: &str) -> String {
        node.get(key).and_then(|v| v.as_str()).unwrap_or_default().to_string()
    }

    mod hypr {
        use super::*;

        pub(super) fn active() -> Result<serde_json::Value, String> {
            json("hyprctl", &["activewindow", "-j"])
        }

        pub(super) fn info(client: &serde_json::Value) -> ActiveWindow {
            ActiveWindow { title: text(client, "title"), app: text(client, "class") }
        }

        pub(super) fn find(pattern: &str, by: MatchBy) -> Result<serde_json::Value, String> {
            if pattern.is_empty() {
                return active();
            }
            let clients = json("hyprctl", &["clients", "-j"])?;
            clients
                .as_array()
                .into_iter()
                .flatten()
                .find(|c| info(c).matches(pattern, by))
                .cloned()
                .ok_or_else(|| no_match(pattern))
        }

        /// hyprctl prints "ok" for a dispatch that worked, else the reason.
        fn dispatch(args: &[&str]) -> Result<(), String> {
            let mut all = vec!["dispatch"];
            all.extend_from_slice(args);
            let out = run("hyprctl", &all)?;
            if out.trim() == "ok" { Ok(()) } else { Err(format!("hyprctl: {}", out.trim())) }
        }

        fn address(client: &serde_json::Value) -> Result<String, String> {
            let a = text(client, "address");
            if a.is_empty() { Err("no window is focused".into()) } else { Ok(format!("address:{a}")) }
        }

        /// 0 none, 1 maximized, 2 fullscreen; older versions report a bool.
        fn fullscreen(client: &serde_json::Value) -> u64 {
            match client.get("fullscreen") {
                Some(serde_json::Value::Bool(b)) => (*b).into(),
                Some(v) => v.as_u64().unwrap_or(0),
                None => 0,
            }
        }

        pub(super) fn act(action: Action, client: &serde_json::Value) -> Result<(), String> {
            let target = address(client)?;
            match action {
                Action::Focus => dispatch(&["focuswindow", &target]),
                Action::Close => dispatch(&["closewindow", &target]),
                Action::Minimize => Err("Hyprland has no minimized windows".into()),
                Action::Maximize => {
                    dispatch(&["focuswindow", &target])?;
                    if fullscreen(client) == 0 { dispatch(&["fullscreen", "1"]) } else { Ok(()) }
                }
                Action::Restore => {
                    dispatch(&["focuswindow", &target])?;
                    // `fullscreen N` toggles mode N (0 fullscreen, 1 maximize).
                    match fullscreen(client) {
                        0 => Ok(()),
                        1 => dispatch(&["fullscreen", "1"]),
                        _ => dispatch(&["fullscreen", "0"]),
                    }
                }
            }
        }

        pub(super) fn place(client: &serde_json::Value, resize: bool, a: i32, b: i32) -> Result<(), String> {
            let target = address(client)?;
            // Only floating windows have a free position or size.
            if client.get("floating").and_then(|f| f.as_bool()) != Some(true) {
                dispatch(&["setfloating", &target])?;
            }
            let verb = if resize { "resizewindowpixel" } else { "movewindowpixel" };
            dispatch(&[verb, &format!("exact {a} {b},{target}")])
        }
    }

    mod sway {
        use super::*;

        fn walk<'a>(node: &'a serde_json::Value, out: &mut Vec<&'a serde_json::Value>) {
            if node.get("pid").is_some() {
                out.push(node);
            }
            for key in ["nodes", "floating_nodes"] {
                for child in node.get(key).and_then(|n| n.as_array()).into_iter().flatten() {
                    walk(child, out);
                }
            }
        }

        fn windows() -> Result<Vec<serde_json::Value>, String> {
            let tree = json("swaymsg", &["-t", "get_tree", "-r"])?;
            let mut out = Vec::new();
            walk(&tree, &mut out);
            Ok(out.into_iter().cloned().collect())
        }

        pub(super) fn info(node: &serde_json::Value) -> ActiveWindow {
            let app = node
                .get("app_id")
                .and_then(|v| v.as_str())
                .map(str::to_string)
                .unwrap_or_else(|| {
                    node.get("window_properties").map(|p| text(p, "class")).unwrap_or_default()
                });
            ActiveWindow { title: text(node, "name"), app }
        }

        pub(super) fn active() -> Result<Option<serde_json::Value>, String> {
            Ok(windows()?.into_iter().find(|n| n.get("focused").and_then(|f| f.as_bool()) == Some(true)))
        }

        pub(super) fn find(pattern: &str, by: MatchBy) -> Result<serde_json::Value, String> {
            if pattern.is_empty() {
                return active()?.ok_or_else(|| "no window is focused".to_string());
            }
            windows()?.into_iter().find(|n| info(n).matches(pattern, by)).ok_or_else(|| no_match(pattern))
        }

        fn command(node: &serde_json::Value, command: &str) -> Result<(), String> {
            let id = node.get("id").and_then(|i| i.as_u64()).ok_or("window has no id")?;
            run("swaymsg", &[&format!("[con_id={id}] {command}")]).map(drop)
        }

        pub(super) fn act(action: Action, node: &serde_json::Value) -> Result<(), String> {
            command(
                node,
                match action {
                    Action::Focus => "focus",
                    Action::Close => "kill",
                    Action::Minimize => "move scratchpad",
                    Action::Maximize => "fullscreen enable",
                    Action::Restore => "fullscreen disable",
                },
            )
        }

        pub(super) fn place(node: &serde_json::Value, resize: bool, a: i32, b: i32) -> Result<(), String> {
            let verb = if resize { format!("resize set {a} {b}") } else { format!("move absolute position {a} {b}") };
            command(node, &format!("floating enable, {verb}"))
        }
    }

    mod kde {
        use super::*;

        /// kdotool searches by regex, case-insensitively by default.
        fn escape(pattern: &str) -> String {
            let mut out = String::with_capacity(pattern.len());
            for c in pattern.chars() {
                if "\\.+*?()|[]{}^$".contains(c) {
                    out.push('\\');
                }
                out.push(c);
            }
            out
        }

        pub(super) fn search(pattern: &str, by: MatchBy) -> Result<Option<String>, String> {
            // An app is its window class (what `getwindowclassname` prints);
            // the instance name is a fallback for apps that only set that.
            let fields: &[Option<&str>] = match by {
                MatchBy::Any => &[None],
                MatchBy::Title => &[Some("--name")],
                MatchBy::App => &[Some("--class"), Some("--classname")],
            };
            let pattern = escape(pattern);
            for field in fields {
                let mut args = vec!["search", "--limit", "1"];
                args.extend(*field);
                args.push(&pattern);
                let out = host_command("kdotool")
                    .args(&args)
                    .output()
                    .map_err(|_| "window control on KDE Wayland needs kdotool installed".to_string())?;
                let id = String::from_utf8_lossy(&out.stdout).lines().next().unwrap_or_default().trim().to_string();
                if !id.is_empty() {
                    return Ok(Some(id));
                }
            }
            Ok(None)
        }

        pub(super) fn active() -> Result<String, String> {
            run("kdotool", &["getactivewindow"])
                .map_err(|_| "window info on KDE Wayland needs kdotool installed".to_string())
        }

        pub(super) fn find(pattern: &str, by: MatchBy) -> Result<String, String> {
            if pattern.is_empty() {
                return active();
            }
            search(pattern, by)?.ok_or_else(|| no_match(pattern))
        }

        pub(super) fn info(id: &str) -> Result<ActiveWindow, String> {
            Ok(ActiveWindow {
                title: run("kdotool", &["getwindowname", id]).unwrap_or_default(),
                app: run("kdotool", &["getwindowclassname", id]).unwrap_or_default(),
            })
        }

        pub(super) fn act(action: Action, id: &str) -> Result<(), String> {
            let args: &[&str] = match action {
                Action::Focus => &["windowactivate", id],
                Action::Close => &["windowclose", id],
                Action::Minimize => &["windowminimize", id],
                Action::Maximize => &["windowstate", "--add", "MAXIMIZED", id],
                Action::Restore => &["windowstate", "--remove", "MAXIMIZED", "--remove", "FULLSCREEN", "--remove", "MINIMIZED", id],
            };
            run("kdotool", args).map(drop)
        }
    }

    pub(super) mod x11 {
        use super::{no_match, Action, ActiveWindow, MatchBy};
        use x11rb::connection::Connection;
        use x11rb::protocol::xproto::{AtomEnum, ClientMessageEvent, ConnectionExt, EventMask, Window};
        use x11rb::rust_connection::RustConnection;

        pub(crate) struct X11 {
            conn: RustConnection,
            root: Window,
        }

        impl X11 {
            pub(crate) fn connect() -> Result<X11, String> {
                let (conn, screen) = x11rb::connect(None).map_err(|e| format!("no X11 display: {e}"))?;
                let root = conn.setup().roots[screen].root;
                Ok(X11 { conn, root })
            }

            fn atom(&self, name: &str) -> Result<u32, String> {
                Ok(self
                    .conn
                    .intern_atom(false, name.as_bytes())
                    .map_err(|e| e.to_string())?
                    .reply()
                    .map_err(|e| e.to_string())?
                    .atom)
            }

            fn property(&self, window: Window, prop: u32, kind: u32) -> Option<Vec<u8>> {
                Some(self.conn.get_property(false, window, prop, kind, 0, 4096).ok()?.reply().ok()?.value)
            }

            fn windows_in(&self, prop: &str) -> Result<Vec<Window>, String> {
                let reply = self
                    .conn
                    .get_property(false, self.root, self.atom(prop)?, AtomEnum::WINDOW, 0, 4096)
                    .map_err(|e| e.to_string())?
                    .reply()
                    .map_err(|e| e.to_string())?;
                Ok(reply.value32().map(|v| v.collect()).unwrap_or_default())
            }

            pub(crate) fn active(&self) -> Result<Option<Window>, String> {
                Ok(self.windows_in("_NET_ACTIVE_WINDOW")?.into_iter().find(|w| *w != 0))
            }

            pub(crate) fn info(&self, window: Window) -> ActiveWindow {
                let name = self.atom("_NET_WM_NAME").unwrap_or(0);
                let utf8 = self.atom("UTF8_STRING").unwrap_or(0);
                let title = self
                    .property(window, name, utf8)
                    .filter(|v| !v.is_empty())
                    .or_else(|| self.property(window, AtomEnum::WM_NAME.into(), AtomEnum::STRING.into()))
                    .map(|v| String::from_utf8_lossy(&v).into_owned())
                    .unwrap_or_default();
                // WM_CLASS is "instance\0class\0"; the class is the readable app name.
                let app = self
                    .property(window, AtomEnum::WM_CLASS.into(), AtomEnum::STRING.into())
                    .and_then(|v| {
                        v.split(|b| *b == 0)
                            .filter(|p| !p.is_empty())
                            .map(|p| String::from_utf8_lossy(p).into_owned())
                            .last()
                    })
                    .unwrap_or_default();
                ActiveWindow { title, app }
            }

            pub(crate) fn find(&self, pattern: &str, by: MatchBy) -> Result<Window, String> {
                if pattern.is_empty() {
                    return self.active()?.ok_or_else(|| "no window is focused".to_string());
                }
                self.windows_in("_NET_CLIENT_LIST")?
                    .into_iter()
                    .find(|w| self.info(*w).matches(pattern, by))
                    .ok_or_else(|| no_match(pattern))
            }

            /// An EWMH request to the window manager about `window`.
            fn message(&self, window: Window, kind: &str, data: [u32; 5]) -> Result<(), String> {
                let event = ClientMessageEvent::new(32, window, self.atom(kind)?, data);
                self.conn
                    .send_event(false, self.root, EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY, event)
                    .map_err(|e| e.to_string())?;
                self.conn.flush().map_err(|e| e.to_string())
            }

            pub(crate) fn act(&self, action: Action, window: Window) -> Result<(), String> {
                // Source indication 2: a pager or tool acting for the user.
                let max = [self.atom("_NET_WM_STATE_MAXIMIZED_VERT")?, self.atom("_NET_WM_STATE_MAXIMIZED_HORZ")?];
                match action {
                    Action::Focus => self.message(window, "_NET_ACTIVE_WINDOW", [2, 0, 0, 0, 0]),
                    Action::Close => self.message(window, "_NET_CLOSE_WINDOW", [0, 2, 0, 0, 0]),
                    // ICCCM IconicState.
                    Action::Minimize => self.message(window, "WM_CHANGE_STATE", [3, 0, 0, 0, 0]),
                    Action::Maximize => self.message(window, "_NET_WM_STATE", [1, max[0], max[1], 2, 0]),
                    Action::Restore => {
                        self.message(window, "_NET_WM_STATE", [0, max[0], max[1], 2, 0])?;
                        self.message(window, "_NET_ACTIVE_WINDOW", [2, 0, 0, 0, 0])
                    }
                }
            }

            pub(crate) fn place(&self, window: Window, resize: bool, a: i32, b: i32) -> Result<(), String> {
                // Flags: which of x/y/width/height apply, plus source 2.
                let (flags, data) = if resize {
                    ((1 << 10) | (1 << 11), [0, 0, a.max(1) as u32, b.max(1) as u32])
                } else {
                    ((1 << 8) | (1 << 9), [a as u32, b as u32, 0, 0])
                };
                self.message(window, "_NET_MOVERESIZE_WINDOW", [flags | (2 << 12), data[0], data[1], data[2], data[3]])
            }
        }
    }
}

#[cfg(windows)]
mod windows {
    use super::{no_match, Action, ActiveWindow, MatchBy};
    use windows_sys::Win32::Foundation::{CloseHandle, HWND, LPARAM, RECT};
    use windows_sys::Win32::System::Threading::{
        OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetForegroundWindow, GetWindowRect, GetWindowTextW, GetWindowThreadProcessId, IsIconic,
        IsWindowVisible, IsZoomed, PostMessageW, SW_MAXIMIZE, SW_MINIMIZE, SW_RESTORE, SWP_NOACTIVATE, SWP_NOMOVE,
        SWP_NOSIZE, SWP_NOZORDER, SetForegroundWindow, SetWindowPos, ShowWindow, WM_CLOSE,
    };

    fn info(hwnd: HWND) -> ActiveWindow {
        // SAFETY: plain Win32 queries on a window handle; every buffer is
        // sized and its length passed alongside it.
        unsafe {
            let mut title = [0u16; 512];
            let len = GetWindowTextW(hwnd, title.as_mut_ptr(), title.len() as i32).max(0) as usize;
            let title = String::from_utf16_lossy(&title[..len]);

            let mut pid = 0u32;
            GetWindowThreadProcessId(hwnd, &mut pid);
            let mut app = String::new();
            let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if !process.is_null() {
                let mut path = [0u16; 1024];
                let mut size = path.len() as u32;
                if QueryFullProcessImageNameW(process, 0, path.as_mut_ptr(), &mut size) != 0 {
                    let full = String::from_utf16_lossy(&path[..size as usize]);
                    app = std::path::Path::new(&full)
                        .file_stem()
                        .map(|s| s.to_string_lossy().into_owned())
                        .unwrap_or(full);
                }
                CloseHandle(process);
            }
            ActiveWindow { title, app }
        }
    }

    pub fn active() -> Result<ActiveWindow, String> {
        // SAFETY: no arguments; may return null.
        let hwnd = unsafe { GetForegroundWindow() };
        Ok(if hwnd.is_null() { ActiveWindow::default() } else { info(hwnd) })
    }

    /// Visible top-level windows with a title, front to back.
    fn windows() -> Vec<HWND> {
        unsafe extern "system" fn collect(hwnd: HWND, out: LPARAM) -> i32 {
            // SAFETY: `out` is the Vec passed to EnumWindows below, alive
            // for the whole enumeration.
            unsafe {
                if IsWindowVisible(hwnd) != 0 {
                    (*(out as *mut Vec<HWND>)).push(hwnd);
                }
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
            return if hwnd.is_null() { Err("no window is focused".into()) } else { Ok(hwnd) };
        }
        windows()
            .into_iter()
            .find(|w| {
                let info = info(*w);
                !info.title.is_empty() && info.matches(pattern, by)
            })
            .ok_or_else(|| no_match(pattern))
    }

    pub fn act(action: Action, pattern: &str, by: MatchBy) -> Result<(), String> {
        let hwnd = find(pattern, by)?;
        // SAFETY: Win32 window calls on a handle EnumWindows/GetForegroundWindow gave us.
        unsafe {
            match action {
                Action::Focus => {
                    if IsIconic(hwnd) != 0 {
                        ShowWindow(hwnd, SW_RESTORE);
                    }
                    if SetForegroundWindow(hwnd) == 0 {
                        return Err("Windows refused to bring the window to the front".into());
                    }
                }
                Action::Minimize => {
                    ShowWindow(hwnd, SW_MINIMIZE);
                }
                Action::Maximize => {
                    ShowWindow(hwnd, SW_MAXIMIZE);
                }
                Action::Restore => {
                    ShowWindow(hwnd, SW_RESTORE);
                }
                Action::Close => {
                    PostMessageW(hwnd, WM_CLOSE, 0, 0);
                }
            }
        }
        Ok(())
    }

    pub fn place(pattern: &str, by: MatchBy, resize: bool, a: i32, b: i32) -> Result<(), String> {
        let hwnd = find(pattern, by)?;
        // SAFETY: as in `act`; `rect` is a plain out-parameter.
        unsafe {
            if IsZoomed(hwnd) != 0 || IsIconic(hwnd) != 0 {
                ShowWindow(hwnd, SW_RESTORE);
            }
            let mut rect: RECT = std::mem::zeroed();
            GetWindowRect(hwnd, &mut rect);
            let flags = SWP_NOZORDER | SWP_NOACTIVATE | if resize { SWP_NOMOVE } else { SWP_NOSIZE };
            if SetWindowPos(hwnd, std::ptr::null_mut(), a, b, a.max(1), b.max(1), flags) == 0 {
                return Err("couldn't move the window".into());
            }
        }
        Ok(())
    }

    pub fn exists(pattern: &str) -> Result<bool, String> {
        Ok(pattern.trim().is_empty() || find(pattern, MatchBy::Any).is_ok())
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use super::{Action, ActiveWindow, MatchBy};

    // Finds the window by app or title, then does `verb` to it. Arguments
    // travel as argv so a pattern can't break out of the script.
    const SCRIPT: &str = r#"on run argv
    set pat to item 1 of argv
    set mode to item 2 of argv
    set verb to item 3 of argv
    set targetProc to missing value
    set targetWin to missing value
    tell application "System Events"
        if pat is "" then
            set targetProc to first application process whose frontmost is true
            if (count of windows of targetProc) > 0 then set targetWin to front window of targetProc
        else
            repeat with p in (application processes whose background only is false)
                repeat with w in windows of p
                    set wn to ""
                    try
                        set wn to name of w
                    end try
                    if (mode is not "Title" and name of p contains pat) or (mode is not "App" and wn contains pat) then
                        set targetProc to p
                        set targetWin to w
                        exit repeat
                    end if
                end repeat
                if targetWin is not missing value then exit repeat
            end repeat
        end if
        if verb is "Exists" then return (targetWin is not missing value) as text
        if verb is "Active" then
            set t to ""
            if targetWin is not missing value then set t to name of targetWin
            return (name of targetProc) & linefeed & t
        end if
        if targetWin is missing value then error "no window matches '" & pat & "'"
        if verb is "Focus" then
            set value of attribute "AXMinimized" of targetWin to false
            set frontmost of targetProc to true
            perform action "AXRaise" of targetWin
        else if verb is "Minimize" then
            set value of attribute "AXMinimized" of targetWin to true
        else if verb is "Maximize" then
            set value of attribute "AXFullScreen" of targetWin to true
        else if verb is "Restore" then
            set value of attribute "AXMinimized" of targetWin to false
            set value of attribute "AXFullScreen" of targetWin to false
        else if verb is "Close" then
            click (first button of targetWin whose subrole is "AXCloseButton")
        else if verb is "Move" then
            set position of targetWin to {(item 4 of argv) as integer, (item 5 of argv) as integer}
        else if verb is "Resize" then
            set size of targetWin to {(item 4 of argv) as integer, (item 5 of argv) as integer}
        end if
    end tell
    return ""
end run"#;

    fn run(args: &[&str]) -> Result<String, String> {
        let output = std::process::Command::new("osascript")
            .args(["-e", SCRIPT])
            .args(args)
            .output()
            .map_err(|e| format!("couldn't ask System Events about windows: {e}"))?;
        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(if err.contains("matches") {
                err.trim().to_string()
            } else {
                "couldn't control windows (Blockwork needs Accessibility access)".into()
            });
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim_end().to_string())
    }

    pub fn active() -> Result<ActiveWindow, String> {
        let text = run(&["", "Any", "Active"])?;
        let mut lines = text.splitn(2, '\n');
        Ok(ActiveWindow {
            app: lines.next().unwrap_or_default().to_string(),
            title: lines.next().unwrap_or_default().to_string(),
        })
    }

    fn mode(by: MatchBy) -> &'static str {
        match by {
            MatchBy::Any => "Any",
            MatchBy::Title => "Title",
            MatchBy::App => "App",
        }
    }

    pub fn act(action: Action, pattern: &str, by: MatchBy) -> Result<(), String> {
        let verb = super::ACTIONS[action as usize];
        run(&[pattern.trim(), mode(by), verb]).map(drop)
    }

    pub fn place(pattern: &str, by: MatchBy, resize: bool, a: i32, b: i32) -> Result<(), String> {
        let (a, b) = (a.to_string(), b.to_string());
        run(&[pattern.trim(), mode(by), if resize { "Resize" } else { "Move" }, &a, &b]).map(drop)
    }

    pub fn exists(pattern: &str) -> Result<bool, String> {
        Ok(pattern.trim().is_empty() || run(&[pattern.trim(), "Any", "Exists"])? == "true")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_title_or_app_ignoring_case() {
        let w = ActiveWindow { title: "Inbox - Mail".into(), app: "Thunderbird".into() };
        assert!(w.matches("", MatchBy::Title));
        assert!(w.matches("inbox", MatchBy::Any));
        assert!(w.matches("THUNDER", MatchBy::Any));
        assert!(!w.matches("firefox", MatchBy::Any));
        assert!(w.matches("inbox", MatchBy::Title) && !w.matches("inbox", MatchBy::App));
        assert!(w.matches("thunder", MatchBy::App) && !w.matches("thunder", MatchBy::Title));
    }

    #[test]
    fn action_names_round_trip() {
        for (i, name) in ACTIONS.iter().enumerate() {
            let action: Action = name.parse().unwrap();
            assert_eq!(action as usize, i);
        }
        assert!("Explode".parse::<Action>().is_err());
    }
}
