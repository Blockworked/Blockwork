use super::{Action, ActiveWindow, MatchBy, Rect, ScreenWindow, no_match};
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

pub(super) fn place(
    pattern: &str,
    by: MatchBy,
    resize: bool,
    a: i32,
    b: i32,
) -> Result<(), String> {
    match desktop()? {
        Desktop::Hyprland => hypr::place(&hypr::find(pattern, by)?, resize, a, b),
        Desktop::Sway => sway::place(&sway::find(pattern, by)?, resize, a, b),
        Desktop::Kde => {
            let id = kde::find(pattern, by)?;
            let (a, b) = (a.to_string(), b.to_string());
            run(
                "kdotool",
                &[
                    if resize { "windowsize" } else { "windowmove" },
                    &id,
                    &a,
                    &b,
                ],
            )
            .map(drop)
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

pub(super) fn stacked() -> Result<Vec<ScreenWindow>, String> {
    match desktop()? {
        Desktop::Hyprland => hypr::stacked(),
        Desktop::Sway => sway::stacked(),
        Desktop::Kde => kde::stacked(),
        Desktop::X11 => x11::X11::connect()?.stacked(),
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
        return Err(if err.is_empty() {
            format!("{program} failed")
        } else {
            format!("{program}: {err}")
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .trim_end()
        .to_string())
}

fn json(program: &str, args: &[&str]) -> Result<serde_json::Value, String> {
    serde_json::from_str(&run(program, args)?)
        .map_err(|e| format!("unexpected {program} output: {e}"))
}

fn text(node: &serde_json::Value, key: &str) -> String {
    node.get(key)
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string()
}

fn number(node: &serde_json::Value, key: &str) -> f64 {
    node.get(key).and_then(|v| v.as_f64()).unwrap_or_default()
}

/// A window's size, refusing empty ones (minimized or hidden).
fn sized(x: f64, y: f64, width: f64, height: f64) -> Result<Rect, String> {
    if width < 1.0 || height < 1.0 {
        return Err("the window is minimized or hidden".into());
    }
    Ok(Rect {
        x,
        y,
        width,
        height,
    })
}

mod hypr {
    use super::*;

    pub(super) fn active() -> Result<serde_json::Value, String> {
        json("hyprctl", &["activewindow", "-j"])
    }

    pub(super) fn info(client: &serde_json::Value) -> ActiveWindow {
        ActiveWindow {
            title: text(client, "title"),
            app: text(client, "class"),
        }
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
        if out.trim() == "ok" {
            Ok(())
        } else {
            Err(format!("hyprctl: {}", out.trim()))
        }
    }

    fn address(client: &serde_json::Value) -> Result<String, String> {
        let a = text(client, "address");
        if a.is_empty() {
            Err("no window is focused".into())
        } else {
            Ok(format!("address:{a}"))
        }
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
                if fullscreen(client) == 0 {
                    dispatch(&["fullscreen", "1"])
                } else {
                    Ok(())
                }
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
            let at = |i: usize| {
                v.and_then(|v| v.get(i))
                    .and_then(|n| n.as_f64())
                    .unwrap_or_default()
            };
            (at(0), at(1))
        };
        if client.get("hidden").and_then(|h| h.as_bool()) == Some(true) {
            return Err("the window is hidden".into());
        }
        let ((x, y), (w, h)) = (pair("at"), pair("size"));
        sized(x, y, w, h)
    }

    /// Clients on a shown workspace, floating ones first, then by focus.
    pub(super) fn stacked() -> Result<Vec<ScreenWindow>, String> {
        let monitors = json("hyprctl", &["monitors", "-j"])?;
        let mut shown = Vec::new();
        for m in monitors.as_array().into_iter().flatten() {
            for key in ["activeWorkspace", "specialWorkspace"] {
                if let Some(id) = m
                    .get(key)
                    .and_then(|w| w.get("id"))
                    .and_then(|i| i.as_i64())
                    && id != 0
                {
                    shown.push(id);
                }
            }
        }
        let clients = json("hyprctl", &["clients", "-j"])?;
        let mut list: Vec<&serde_json::Value> = clients
            .as_array()
            .into_iter()
            .flatten()
            .filter(|c| c.get("mapped").and_then(|m| m.as_bool()) != Some(false))
            .filter(|c| {
                c.get("workspace")
                    .and_then(|w| w.get("id"))
                    .and_then(|i| i.as_i64())
                    .is_some_and(|id| shown.contains(&id))
            })
            .collect();
        let rank = |c: &serde_json::Value| {
            let floating = c.get("floating").and_then(|f| f.as_bool()) == Some(true);
            let full = fullscreen(c) > 0;
            let focus = c
                .get("focusHistoryID")
                .and_then(|f| f.as_i64())
                .unwrap_or(i64::MAX);
            (!full, !floating, focus)
        };
        list.sort_by_key(|c| rank(c));
        Ok(list
            .into_iter()
            .filter_map(|c| {
                Some(ScreenWindow {
                    title: text(c, "title"),
                    app: text(c, "class"),
                    pid: c.get("pid").and_then(|p| p.as_u64()).unwrap_or(0) as u32,
                    bounds: bounds(c).ok()?,
                })
            })
            .collect())
    }

    pub(super) fn place(
        client: &serde_json::Value,
        resize: bool,
        a: i32,
        b: i32,
    ) -> Result<(), String> {
        let target = address(client)?;
        // Only floating windows have a free position or size.
        if client.get("floating").and_then(|f| f.as_bool()) != Some(true) {
            dispatch(&["setfloating", &target])?;
        }
        let verb = if resize {
            "resizewindowpixel"
        } else {
            "movewindowpixel"
        };
        dispatch(&[verb, &format!("exact {a} {b},{target}")])
    }
}

pub(super) mod sway {
    use super::*;

    /// The workspace a window is on isn't in its own node; it's noted as
    /// `workspace` so `act_command` can tell a minimized window.
    fn walk(node: &serde_json::Value, workspace: &str, out: &mut Vec<serde_json::Value>) {
        let workspace = match node.get("type").and_then(|t| t.as_str()) {
            Some("workspace") => node
                .get("name")
                .and_then(|n| n.as_str())
                .unwrap_or(workspace),
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
            for child in node
                .get(key)
                .and_then(|n| n.as_array())
                .into_iter()
                .flatten()
            {
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

    /// Windows on shown workspaces: fullscreen, then floating (last drawn
    /// on top), then tiled. `visible` can't be used, since the picker's own
    /// fullscreen overlay hides everything under it.
    pub(super) fn stacked() -> Result<Vec<ScreenWindow>, String> {
        let workspaces = json("swaymsg", &["-t", "get_workspaces", "-r"])?;
        let shown: Vec<String> = workspaces
            .as_array()
            .into_iter()
            .flatten()
            .filter(|w| w.get("visible").and_then(|v| v.as_bool()) == Some(true))
            .map(|w| text(w, "name"))
            .collect();
        let tree = json("swaymsg", &["-t", "get_tree", "-r"])?;
        let mut all = Vec::new();
        walk(&tree, "", &mut all);
        let mut list: Vec<(usize, serde_json::Value)> = all
            .into_iter()
            .filter(|n| shown.iter().any(|w| *w == text(n, "workspace")))
            .enumerate()
            .collect();
        let rank = |i: usize, n: &serde_json::Value| {
            let full = number(n, "fullscreen_mode") > 0.0;
            let floating = n
                .get("type")
                .and_then(|t| t.as_str())
                .is_some_and(|t| t == "floating_con");
            (!full, !floating, std::cmp::Reverse(i))
        };
        list.sort_by_key(|(i, n)| rank(*i, n));
        Ok(list
            .into_iter()
            .filter_map(|(_, n)| {
                let info = info(&n);
                Some(ScreenWindow {
                    title: info.title,
                    app: info.app,
                    pid: n.get("pid").and_then(|p| p.as_u64()).unwrap_or(0) as u32,
                    bounds: bounds(&n).ok()?,
                })
            })
            .collect())
    }

    pub(super) fn info(node: &serde_json::Value) -> ActiveWindow {
        let app = node
            .get("app_id")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .unwrap_or_else(|| {
                node.get("window_properties")
                    .map(|p| text(p, "class"))
                    .unwrap_or_default()
            });
        ActiveWindow {
            title: text(node, "name"),
            app,
        }
    }

    pub(super) fn active() -> Result<Option<serde_json::Value>, String> {
        Ok(windows()?
            .into_iter()
            .find(|n| n.get("focused").and_then(|f| f.as_bool()) == Some(true)))
    }

    pub(super) fn find(pattern: &str, by: MatchBy) -> Result<serde_json::Value, String> {
        if pattern.is_empty() {
            return active()?.ok_or_else(|| "no window is focused".to_string());
        }
        windows()?
            .into_iter()
            .find(|n| info(n).matches(pattern, by))
            .ok_or_else(|| no_match(pattern))
    }

    fn command(node: &serde_json::Value, command: &str) -> Result<(), String> {
        let id = node
            .get("id")
            .and_then(|i| i.as_u64())
            .ok_or("window has no id")?;
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
            || node
                .get("scratchpad_state")
                .and_then(|s| s.as_str())
                .is_some_and(|s| s != "none");
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
        let (outer, inner) = (
            node.get("rect").unwrap_or(&empty),
            node.get("window_rect").unwrap_or(&empty),
        );
        sized(
            number(outer, "x") + number(inner, "x"),
            number(outer, "y") + number(inner, "y"),
            number(inner, "width"),
            number(inner, "height"),
        )
    }

    pub(super) fn place(
        node: &serde_json::Value,
        resize: bool,
        a: i32,
        b: i32,
    ) -> Result<(), String> {
        let verb = if resize {
            format!("resize set {a} {b}")
        } else {
            format!("move absolute position {a} {b}")
        };
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
            let id = String::from_utf8_lossy(&out.stdout)
                .lines()
                .next()
                .unwrap_or_default()
                .trim()
                .to_string();
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
        client_geometry(id)
            .or_else(|_| parse_geometry(&run("kdotool", &["getwindowgeometry", id])?))
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

    /// KWin's own stacking order, topmost first.
    pub(super) fn stacked() -> Result<Vec<ScreenWindow>, String> {
        let script = "const out = [];\n\
             const list = workspace.stackingOrder;\n\
             for (let i = list.length - 1; i >= 0; i--) {\n\
               const w = list[i];\n\
               if (!w || w.minimized || w.deleted || !(w.normalWindow || w.dialog || w.utility)) continue;\n\
               const desks = w.desktops || [];\n\
               if (!w.onAllDesktops && desks.length && desks.indexOf(workspace.currentDesktop) < 0) continue;\n\
               const g = w.clientGeometry;\n\
               out.push({title: w.caption, app: w.resourceClass, pid: w.pid, x: g.x, y: g.y, width: g.width, height: g.height});\n\
             }\n\
             reply(JSON.stringify(out));";
        let reply: serde_json::Value = serde_json::from_str(&crate::kwin::script(script)?)
            .map_err(|e| format!("unexpected KWin window list: {e}"))?;
        Ok(reply
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|w| {
                Some(ScreenWindow {
                    title: text(w, "title"),
                    app: text(w, "app"),
                    pid: w.get("pid").and_then(|p| p.as_u64()).unwrap_or(0) as u32,
                    bounds: sized(
                        number(w, "x"),
                        number(w, "y"),
                        number(w, "width"),
                        number(w, "height"),
                    )
                    .ok()?,
                })
            })
            .collect())
    }

    /// "x,y,width,height", as the client geometry script replies.
    pub(crate) fn parse_rect(text: &str) -> Result<Rect, String> {
        let n: Vec<f64> = text
            .split(',')
            .filter_map(|v| v.trim().parse().ok())
            .collect();
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
            Action::Restore => &[
                "windowstate",
                "--remove",
                "MAXIMIZED",
                "--remove",
                "FULLSCREEN",
                "--remove",
                "MINIMIZED",
                id,
            ],
        };
        run("kdotool", args).map(drop)
    }
}

pub(super) mod x11 {
    use super::{Action, ActiveWindow, MatchBy, Rect, ScreenWindow, no_match, sized};
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::{AtomEnum, ClientMessageEvent, ConnectionExt, EventMask, Window};
    use x11rb::rust_connection::RustConnection;

    pub(crate) struct X11 {
        conn: RustConnection,
        root: Window,
    }

    impl X11 {
        pub(crate) fn connect() -> Result<X11, String> {
            let (conn, screen) =
                x11rb::connect(None).map_err(|e| format!("no X11 display: {e}"))?;
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
            Some(
                self.conn
                    .get_property(false, window, prop, kind, 0, 4096)
                    .ok()?
                    .reply()
                    .ok()?
                    .value,
            )
        }

        fn windows_in(&self, prop: &str) -> Result<Vec<Window>, String> {
            let reply = self
                .conn
                .get_property(
                    false,
                    self.root,
                    self.atom(prop)?,
                    AtomEnum::WINDOW,
                    0,
                    4096,
                )
                .map_err(|e| e.to_string())?
                .reply()
                .map_err(|e| e.to_string())?;
            Ok(reply.value32().map(|v| v.collect()).unwrap_or_default())
        }

        pub(crate) fn active(&self) -> Result<Option<Window>, String> {
            Ok(self
                .windows_in("_NET_ACTIVE_WINDOW")?
                .into_iter()
                .find(|w| *w != 0))
        }

        pub(crate) fn info(&self, window: Window) -> ActiveWindow {
            let name = self.atom("_NET_WM_NAME").unwrap_or(0);
            let utf8 = self.atom("UTF8_STRING").unwrap_or(0);
            let title = self
                .property(window, name, utf8)
                .filter(|v| !v.is_empty())
                .or_else(|| {
                    self.property(window, AtomEnum::WM_NAME.into(), AtomEnum::STRING.into())
                })
                .map(|v| String::from_utf8_lossy(&v).into_owned())
                .unwrap_or_default();
            // WM_CLASS is "instance\0class\0"; the class is the readable app name.
            let app = self
                .property(window, AtomEnum::WM_CLASS.into(), AtomEnum::STRING.into())
                .and_then(|v| {
                    v.split(|b| *b == 0)
                        .filter(|p| !p.is_empty())
                        .map(|p| String::from_utf8_lossy(p).into_owned())
                        .next_back()
                })
                .unwrap_or_default();
            ActiveWindow { title, app }
        }

        pub(crate) fn find(&self, pattern: &str, by: MatchBy) -> Result<Window, String> {
            if pattern.is_empty() {
                return self
                    .active()?
                    .ok_or_else(|| "no window is focused".to_string());
            }
            self.windows_in("_NET_CLIENT_LIST")?
                .into_iter()
                .find(|w| self.info(*w).matches(pattern, by))
                .ok_or_else(|| no_match(pattern))
        }

        /// Mapped client windows, topmost first. Without a window manager's
        /// stacking list, the root's own children (bottom to top) stand in.
        pub(crate) fn stacked(&self) -> Result<Vec<ScreenWindow>, String> {
            use x11rb::protocol::xproto::MapState;
            let mut list = self
                .windows_in("_NET_CLIENT_LIST_STACKING")
                .unwrap_or_default();
            if list.is_empty() {
                list = self
                    .conn
                    .query_tree(self.root)
                    .map_err(|e| e.to_string())?
                    .reply()
                    .map_err(|e| e.to_string())?
                    .children;
            }
            let pid_atom = self.atom("_NET_WM_PID")?;
            let hidden = self.atom("_NET_WM_STATE_HIDDEN").unwrap_or(0);
            let state = self.atom("_NET_WM_STATE").unwrap_or(0);
            let mut out = Vec::new();
            for window in list.into_iter().rev() {
                let viewable = self
                    .conn
                    .get_window_attributes(window)
                    .ok()
                    .and_then(|c| c.reply().ok())
                    .is_some_and(|a| a.map_state == MapState::VIEWABLE);
                if !viewable {
                    continue;
                }
                let states = self
                    .property(window, state, AtomEnum::ATOM.into())
                    .unwrap_or_default();
                if states
                    .chunks_exact(4)
                    .any(|c| u32::from_ne_bytes([c[0], c[1], c[2], c[3]]) == hidden)
                {
                    continue;
                }
                let Ok(bounds) = self.bounds(window) else {
                    continue;
                };
                let pid = self
                    .property(window, pid_atom, AtomEnum::CARDINAL.into())
                    .filter(|v| v.len() >= 4)
                    .map(|v| u32::from_ne_bytes([v[0], v[1], v[2], v[3]]))
                    .unwrap_or(0);
                let info = self.info(window);
                out.push(ScreenWindow {
                    title: info.title,
                    app: info.app,
                    pid,
                    bounds,
                });
            }
            Ok(out)
        }

        /// An EWMH request to the window manager about `window`.
        fn message(&self, window: Window, kind: &str, data: [u32; 5]) -> Result<(), String> {
            let event = ClientMessageEvent::new(32, window, self.atom(kind)?, data);
            self.conn
                .send_event(
                    false,
                    self.root,
                    EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
                    event,
                )
                .map_err(|e| e.to_string())?;
            self.conn.flush().map_err(|e| e.to_string())
        }

        pub(crate) fn act(&self, action: Action, window: Window) -> Result<(), String> {
            // Source indication 2: a pager or tool acting for the user.
            let max = [
                self.atom("_NET_WM_STATE_MAXIMIZED_VERT")?,
                self.atom("_NET_WM_STATE_MAXIMIZED_HORZ")?,
            ];
            match action {
                Action::Focus => self.message(window, "_NET_ACTIVE_WINDOW", [2, 0, 0, 0, 0]),
                Action::Close => self.message(window, "_NET_CLOSE_WINDOW", [0, 2, 0, 0, 0]),
                // ICCCM IconicState.
                Action::Minimize => self.message(window, "WM_CHANGE_STATE", [3, 0, 0, 0, 0]),
                Action::Maximize => {
                    self.message(window, "_NET_WM_STATE", [1, max[0], max[1], 2, 0])
                }
                Action::Restore => {
                    self.message(window, "_NET_WM_STATE", [0, max[0], max[1], 2, 0])?;
                    self.message(window, "_NET_ACTIVE_WINDOW", [2, 0, 0, 0, 0])
                }
            }
        }

        /// The window's own area (without the frame), translated to root coordinates.
        pub(crate) fn bounds(&self, window: Window) -> Result<Rect, String> {
            let geometry = self
                .conn
                .get_geometry(window)
                .map_err(|e| e.to_string())?
                .reply()
                .map_err(|e| e.to_string())?;
            let origin = self
                .conn
                .translate_coordinates(window, self.root, 0, 0)
                .map_err(|e| e.to_string())?
                .reply()
                .map_err(|e| e.to_string())?;
            sized(
                origin.dst_x as f64,
                origin.dst_y as f64,
                geometry.width as f64,
                geometry.height as f64,
            )
        }

        pub(crate) fn place(
            &self,
            window: Window,
            resize: bool,
            a: i32,
            b: i32,
        ) -> Result<(), String> {
            // Flags: which of x/y/width/height apply, plus source 2.
            let (flags, data) = if resize {
                (
                    (1 << 10) | (1 << 11),
                    [0, 0, a.max(1) as u32, b.max(1) as u32],
                )
            } else {
                ((1 << 8) | (1 << 9), [a as u32, b as u32, 0, 0])
            };
            self.message(
                window,
                "_NET_MOVERESIZE_WINDOW",
                [flags | (2 << 12), data[0], data[1], data[2], data[3]],
            )
        }
    }
}
