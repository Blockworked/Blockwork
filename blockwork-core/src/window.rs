//! Window info and control: the active-window reporters, the `WhenWindow`
//! trigger and the window action blocks. Wayland has no common API for
//! this, so Linux asks the compositor directly where it can (Hyprland, Sway,
//! KDE via kdotool) and falls back to X11 off Wayland.

pub use crate::screen::Rect;

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

/// The client area of a matching window (the active one when `pattern` is
/// empty), in screen coordinates.
#[cfg(target_os = "linux")]
pub fn bounds(pattern: &str, by: MatchBy) -> Result<Rect, String> {
    linux::bounds(pattern.trim(), by)
}

/// Whether any window's title or app contains `pattern`.
#[cfg(target_os = "linux")]
pub fn exists(pattern: &str) -> Result<bool, String> {
    linux::exists(pattern.trim())
}

#[cfg(windows)]
pub use windows::{act, active, bounds, exists, place};

#[cfg(target_os = "macos")]
pub use macos::{act, active, bounds, exists, place};

#[cfg(not(any(target_os = "linux", windows, target_os = "macos")))]
mod unsupported {
    use super::{Action, ActiveWindow, MatchBy, Rect};
    const MSG: &str = "window info isn't supported on this platform";
    pub fn bounds(_: &str, _: MatchBy) -> Result<Rect, String> {
        Err(MSG.into())
    }
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
pub use unsupported::{act, active, bounds, exists, place};

#[cfg(target_os = "linux")]
mod linux {
    use super::{no_match, Action, ActiveWindow, MatchBy, Rect};
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

    pub(super) fn bounds(pattern: &str, by: MatchBy) -> Result<Rect, String> {
        match desktop()? {
            Desktop::Hyprland => hypr::bounds(&hypr::find(pattern, by)?),
            Desktop::Sway => sway::bounds(&sway::find(pattern, by)?),
            Desktop::Kde => kde::bounds(&kde::find(pattern, by)?),
            Desktop::X11 => {
                let x = x11::X11::connect()?;
                let w = x.find(pattern, by)?;
                x.bounds(w)
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

    fn number(node: &serde_json::Value, key: &str) -> f64 {
        node.get(key).and_then(|v| v.as_f64()).unwrap_or_default()
    }

    /// A window's size, refusing empty ones (minimized or hidden).
    fn sized(x: f64, y: f64, width: f64, height: f64) -> Result<Rect, String> {
        if width < 1.0 || height < 1.0 {
            return Err("the window is minimized or hidden".into());
        }
        Ok(Rect { x, y, width, height })
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

        pub(super) fn bounds(client: &serde_json::Value) -> Result<Rect, String> {
            let pair = |key: &str| {
                let v = client.get(key).and_then(|v| v.as_array());
                let at = |i: usize| v.and_then(|v| v.get(i)).and_then(|n| n.as_f64()).unwrap_or_default();
                (at(0), at(1))
            };
            if client.get("hidden").and_then(|h| h.as_bool()) == Some(true) {
                return Err("the window is hidden".into());
            }
            let ((x, y), (w, h)) = (pair("at"), pair("size"));
            sized(x, y, w, h)
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

    pub(super) mod sway {
        use super::*;

        /// The workspace a window is on isn't in its own node; it's noted as
        /// `workspace` so `act_command` can tell a minimized window.
        fn walk(node: &serde_json::Value, workspace: &str, out: &mut Vec<serde_json::Value>) {
            let workspace = match node.get("type").and_then(|t| t.as_str()) {
                Some("workspace") => node.get("name").and_then(|n| n.as_str()).unwrap_or(workspace),
                _ => workspace,
            };
            if node.get("pid").is_some() {
                let mut window = node.clone();
                if let Some(fields) = window.as_object_mut() {
                    fields.remove("nodes");
                    fields.remove("floating_nodes");
                    fields.insert("workspace".into(), workspace.into());
                }
                out.push(window);
            }
            for key in ["nodes", "floating_nodes"] {
                for child in node.get(key).and_then(|n| n.as_array()).into_iter().flatten() {
                    walk(child, workspace, out);
                }
            }
        }

        fn windows() -> Result<Vec<serde_json::Value>, String> {
            let tree = json("swaymsg", &["-t", "get_tree", "-r"])?;
            let mut out = Vec::new();
            walk(&tree, "", &mut out);
            Ok(out)
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
            command(node, act_command(action, node))
        }

        /// Sway has no minimize: Minimize parks the window in the scratchpad,
        /// so Restore has to bring it back out (and tile it again) as well as
        /// leave fullscreen.
        pub(crate) fn act_command(action: Action, node: &serde_json::Value) -> &'static str {
            // Sway parks scratchpad windows on the hidden `__i3_scratch`
            // workspace; i3-style trees also say so in `scratchpad_state`.
            let in_scratchpad = node.get("workspace").and_then(|w| w.as_str()) == Some("__i3_scratch")
                || node.get("scratchpad_state").and_then(|s| s.as_str()).is_some_and(|s| s != "none");
            match action {
                Action::Focus => "focus",
                Action::Close => "kill",
                Action::Minimize => "move scratchpad",
                Action::Maximize => "fullscreen enable",
                Action::Restore if in_scratchpad => "scratchpad show, floating disable",
                Action::Restore => "fullscreen disable",
            }
        }

        /// `rect` is the container; `window_rect` is the content inside it.
        pub(super) fn bounds(node: &serde_json::Value) -> Result<Rect, String> {
            let empty = serde_json::Value::Null;
            let (outer, inner) = (node.get("rect").unwrap_or(&empty), node.get("window_rect").unwrap_or(&empty));
            sized(
                number(outer, "x") + number(inner, "x"),
                number(outer, "y") + number(inner, "y"),
                number(inner, "width"),
                number(inner, "height"),
            )
        }

        pub(super) fn place(node: &serde_json::Value, resize: bool, a: i32, b: i32) -> Result<(), String> {
            let verb = if resize { format!("resize set {a} {b}") } else { format!("move absolute position {a} {b}") };
            command(node, &format!("floating enable, {verb}"))
        }
    }

    pub(super) mod kde {
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

        /// `getwindowgeometry` prints "Position: x,y" and "Geometry: WxH" lines.
        pub(crate) fn parse_geometry(out: &str) -> Result<Rect, String> {
            let field = |name: &str, sep: char| -> Option<(f64, f64)> {
                let line = out.lines().find_map(|l| l.trim().strip_prefix(name))?;
                let (a, b) = line.trim().split_once(sep)?;
                Some((a.trim().parse().ok()?, b.trim().parse().ok()?))
            };
            let bad = || format!("unexpected kdotool output: {}", out.trim());
            let (x, y) = field("Position:", ',').ok_or_else(bad)?;
            let (w, h) = field("Geometry:", 'x').ok_or_else(bad)?;
            sized(x, y, w, h)
        }

        /// The window's content area. kdotool's geometry is the frame,
        /// title bar included, so ask KWin for the client rect first.
        pub(super) fn bounds(id: &str) -> Result<Rect, String> {
            client_geometry(id).or_else(|_| parse_geometry(&run("kdotool", &["getwindowgeometry", id])?))
        }

        fn client_geometry(id: &str) -> Result<Rect, String> {
            let script = format!(
                "const w = workspace.windowList().find(w => w.internalId.toString() === {});\n\
                 const g = w && w.clientGeometry;\n\
                 reply(g ? [g.x, g.y, g.width, g.height].join(\",\") : \"\");",
                crate::kwin::js_string(id)
            );
            parse_rect(&crate::kwin::script(&script)?)
        }

        /// "x,y,width,height", as the client geometry script replies.
        pub(crate) fn parse_rect(text: &str) -> Result<Rect, String> {
            let n: Vec<f64> = text.split(',').filter_map(|v| v.trim().parse().ok()).collect();
            match n[..] {
                [x, y, w, h] => sized(x, y, w, h),
                _ => Err(format!("unexpected window geometry '{text}'")),
            }
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
        use super::{no_match, sized, Action, ActiveWindow, MatchBy, Rect};
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

            /// The window's own area (without the frame), translated to root coordinates.
            pub(crate) fn bounds(&self, window: Window) -> Result<Rect, String> {
                let geometry = self.conn.get_geometry(window).map_err(|e| e.to_string())?.reply().map_err(|e| e.to_string())?;
                let origin = self
                    .conn
                    .translate_coordinates(window, self.root, 0, 0)
                    .map_err(|e| e.to_string())?
                    .reply()
                    .map_err(|e| e.to_string())?;
                sized(origin.dst_x as f64, origin.dst_y as f64, geometry.width as f64, geometry.height as f64)
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
    use super::{no_match, Action, ActiveWindow, MatchBy, Rect};
    use std::time::{Duration, Instant};
    use windows_sys::Win32::Foundation::{CloseHandle, HWND, LPARAM, POINT, RECT};
    use windows_sys::Win32::Graphics::Dwm::{DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS, DwmGetWindowAttribute};
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, SendInput};
    use windows_sys::Win32::Graphics::Gdi::ClientToScreen;
    use windows_sys::Win32::System::Threading::{
        AttachThreadInput, GetCurrentThreadId, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        BringWindowToTop, EnumChildWindows, EnumWindows, GA_ROOTOWNER, GWL_EXSTYLE, GetAncestor, GetClientRect,
        GetForegroundWindow, GetWindowLongW, GetWindowRect, GetWindowTextW, GetWindowThreadProcessId, IsIconic,
        IsWindowVisible, IsZoomed, PostMessageW, SW_MAXIMIZE, SW_MINIMIZE, SW_RESTORE, SWP_NOACTIVATE, SWP_NOMOVE,
        SWP_NOSIZE, SWP_NOZORDER, SetForegroundWindow, SetWindowPos, ShowWindow, WM_CLOSE, WS_EX_TOOLWINDOW,
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
        unsafe { EnumChildWindows(hwnd, Some(first_foreign), &mut out as *mut Option<String> as LPARAM) };
        out
    }

    fn info(hwnd: HWND) -> ActiveWindow {
        let mut title = [0u16; 512];
        // SAFETY: the buffer's length is passed alongside it.
        let len = unsafe { GetWindowTextW(hwnd, title.as_mut_ptr(), title.len() as i32) }.max(0) as usize;
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
        Ok(if hwnd.is_null() { ActiveWindow::default() } else { info(hwnd) })
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
            let ok = DwmGetWindowAttribute(hwnd, DWMWA_CLOAKED as u32, (&mut cloaked as *mut u32).cast(), 4) >= 0;
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
                    Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: 0xE8, wScan: 0, dwFlags: flags, time: 0, dwExtraInfo: 0 } },
                };
                let taps = [key(0), key(KEYEVENTF_KEYUP)];
                SendInput(taps.len() as u32, taps.as_ptr(), std::mem::size_of::<INPUT>() as i32);
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
                || DwmGetWindowAttribute(hwnd, DWMWA_EXTENDED_FRAME_BOUNDS as u32, (&mut frame as *mut RECT).cast(), size) < 0
            {
                return (0, 0, 0, 0);
            }
            let border = (frame.left - rect.left, frame.top - rect.top, rect.right - frame.right, rect.bottom - frame.bottom);
            // A bogus answer (another DPI context, mid-animation) is worse than none.
            if [border.0, border.1, border.2, border.3].iter().all(|b| (0..=32).contains(b)) { border } else { (0, 0, 0, 0) }
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
            let (width, height) = ((rect.right - rect.left) as f64, (rect.bottom - rect.top) as f64);
            if width < 1.0 || height < 1.0 {
                return Err("the window is minimized or hidden".into());
            }
            Ok(Rect { x: origin.x as f64, y: origin.y as f64, width, height })
        }
    }

    pub fn exists(pattern: &str) -> Result<bool, String> {
        Ok(pattern.trim().is_empty() || find(pattern, MatchBy::Any).is_ok())
    }
}

#[cfg(target_os = "macos")]
#[path = "window/macos.rs"]
mod macos;

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "linux")]
    #[test]
    fn sway_restore_brings_minimized_windows_back() {
        let hidden = serde_json::json!({ "workspace": "__i3_scratch" });
        let tiled = serde_json::json!({ "workspace": "1", "scratchpad_state": "none" });
        assert_eq!(
            linux::sway::act_command(
                Action::Restore,
                &serde_json::json!({ "scratchpad_state": "fresh" })
            ),
            "scratchpad show, floating disable"
        );
        assert_eq!(
            linux::sway::act_command(Action::Minimize, &tiled),
            "move scratchpad"
        );
        assert_eq!(
            linux::sway::act_command(Action::Restore, &hidden),
            "scratchpad show, floating disable"
        );
        assert_eq!(
            linux::sway::act_command(Action::Restore, &tiled),
            "fullscreen disable"
        );
    }

    #[test]
    fn matches_title_or_app_ignoring_case() {
        let w = ActiveWindow {
            title: "Inbox - Mail".into(),
            app: "Thunderbird".into(),
        };
        assert!(w.matches("", MatchBy::Title));
        assert!(w.matches("inbox", MatchBy::Any));
        assert!(w.matches("THUNDER", MatchBy::Any));
        assert!(!w.matches("firefox", MatchBy::Any));
        assert!(w.matches("inbox", MatchBy::Title) && !w.matches("inbox", MatchBy::App));
        assert!(w.matches("thunder", MatchBy::App) && !w.matches("thunder", MatchBy::Title));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn kdotool_geometry_parses() {
        let out = "Window {5f1e}\n  Position: 120,-40\n  Geometry: 800x600\n";
        assert_eq!(
            linux::kde::parse_geometry(out),
            Ok(Rect {
                x: 120.0,
                y: -40.0,
                width: 800.0,
                height: 600.0
            })
        );
        assert!(linux::kde::parse_geometry("Position: 1,2\n  Geometry: 0x0").is_err());
        assert!(linux::kde::parse_geometry("nonsense").is_err());
        assert_eq!(
            linux::kde::parse_rect("2427,374,480,360"),
            Ok(Rect {
                x: 2427.0,
                y: 374.0,
                width: 480.0,
                height: 360.0
            })
        );
        assert_eq!(
            linux::kde::parse_rect("0,13,1707.5,1067").map(|r| r.width),
            Ok(1707.5)
        );
        assert!(linux::kde::parse_rect("").is_err());
        assert!(linux::kde::parse_rect("1,2,0,0").is_err());
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
