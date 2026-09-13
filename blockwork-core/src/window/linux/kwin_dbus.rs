//! Window sensing/control on Plasma Wayland via KWin's own scripting D-Bus
//! interface (`org.kde.KWin`, object `/Scripting`, interface
//! `org.kde.kwin.Scripting`).
//!
//! KWin has no *stable* public D-Bus method for arbitrary window
//! enumeration/control — the interface below only loads/runs/unloads a
//! script file, which then runs inside KWin's own JS engine with full access
//! to `workspace`/`Window` objects. That's not a workaround; it's the
//! documented, version-independent way to do this (the same technique the
//! third-party tool `kdotool` uses), and the only one that sees native
//! Wayland toplevels at all — an XWayland/X11 query from outside KWin would
//! only ever see rootless XWayland clients.
//!
//! Loading and running a script are two separate D-Bus calls against two
//! separate objects: `loadScript` (on `/Scripting`) replies with an integer
//! script id, and *that* id addresses a second, per-script object
//! (`/Scripting/Script{id}`, interface `org.kde.kwin.Script`) whose `run`
//! method is what actually executes the script — see `run_script`.
//!
//! Since a KWin script can't return a value directly, each script reports
//! its result by calling back over D-Bus (`callDBus`, a global KWin scripts
//! can call) into a tiny callback object this process serves for the
//! duration of one call — see `run_script`.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use zbus::blocking::Connection;
use zbus::interface;

const KWIN_SERVICE: &str = "org.kde.KWin";
const SCRIPTING_PATH: &str = "/Scripting";
const SCRIPTING_INTERFACE: &str = "org.kde.kwin.Scripting";
/// Interface of the per-script object `loadScript` addresses via the id it
/// returns (`/Scripting/Script{id}`) — see `run_script`'s `run`/`stop` calls.
const SCRIPT_INTERFACE: &str = "org.kde.kwin.Script";
const CALLBACK_PATH: &str = "/org/blockwork/WindowSensing";
const CALLBACK_INTERFACE: &str = "org.blockwork.WindowSensing.Callback";
const SCRIPT_TIMEOUT: Duration = Duration::from_secs(2);
const POLL_INTERVAL: Duration = Duration::from_millis(20);

struct Callback {
    result: Arc<Mutex<Option<String>>>,
}

#[interface(name = "org.blockwork.WindowSensing.Callback")]
impl Callback {
    // Without this, zbus's macro exposes the method as D-Bus member
    // "Report" (PascalCase, its default from the Rust fn name) — but the
    // generated script's `callDBus(..., "report", value)` call names the
    // lowercase member literally, so the call would otherwise arrive as an
    // unknown-method error every time (confirmed live: this exact mismatch
    // was why every call through this module silently timed out).
    #[zbus(name = "report")]
    fn report(&self, value: String) {
        *self.result.lock().unwrap_or_else(|e| e.into_inner()) = Some(value);
    }
}

/// JS snippet computing `target` (a matching `Window`, or `null`) from
/// `workspace.stackingOrder`, filtered by an optional lowercased app-name
/// substring (checked against both `resourceClass` and `resourceName`) and
/// an optional lowercased title substring — either blank matches anything.
/// Skips non-`normalWindow`s (KWin's own shell/overview surfaces), which is
/// also why a bare `workspace.activeWindow` read isn't trusted directly for
/// focused-window queries below.
fn matcher_prelude(app: Option<&str>, title_contains: Option<&str>) -> String {
    let app_json = serde_json::to_string(&app.unwrap_or("").to_lowercase()).unwrap_or_else(|_| "\"\"".to_string());
    let title_json = serde_json::to_string(&title_contains.unwrap_or("").to_lowercase()).unwrap_or_else(|_| "\"\"".to_string());
    format!(
        r#"var appNeedle = {app_json};
    var titleNeedle = {title_json};
    var list = workspace.stackingOrder;
    var target = null;
    for (var i = list.length - 1; i >= 0; i--) {{
        var w = list[i];
        if (!w.normalWindow) continue;
        if (appNeedle && (w.resourceClass || '').toLowerCase().indexOf(appNeedle) === -1
            && (w.resourceName || '').toLowerCase().indexOf(appNeedle) === -1) continue;
        if (titleNeedle && (w.caption || '').toLowerCase().indexOf(titleNeedle) === -1) continue;
        target = w;
        break;
    }}"#
    )
}

/// Loads a KWin script whose body is `script_body` (JS assuming a `report(value)`
/// function is in scope), waits up to `SCRIPT_TIMEOUT` for it to call back, then
/// unloads the script. `Ok(None)` means the script never reported (timed out or
/// KWin/D-Bus itself is unreachable in a way that didn't already error).
fn run_script(script_body: &str) -> Result<Option<String>, String> {
    let connection = Connection::session().map_err(|e| format!("D-Bus session bus unavailable: {e}"))?;
    let result: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    connection
        .object_server()
        .at(CALLBACK_PATH, Callback { result: Arc::clone(&result) })
        .map_err(|e| format!("failed to register D-Bus callback object: {e}"))?;
    let destination = connection
        .unique_name()
        .ok_or_else(|| "D-Bus connection has no unique name".to_string())?
        .to_string();

    let plugin_name = format!("blockwork-window-sensing-{}", uuid::Uuid::new_v4().simple());
    let script = format!(
        r#"(function() {{
    function report(value) {{
        callDBus("{destination}", "{CALLBACK_PATH}", "{CALLBACK_INTERFACE}", "report", value);
    }}
    try {{
        {script_body}
    }} catch (e) {{
        report("__error__:" + e);
    }}
}})();
"#
    );
    let script_path = std::env::temp_dir().join(format!("{plugin_name}.js"));
    std::fs::write(&script_path, &script).map_err(|e| format!("failed to write temporary KWin script: {e}"))?;
    let script_path_str = script_path.to_string_lossy().to_string();

    // `loadScript` replies with an `i32` script id — *not* just a
    // load/no-load signal. That id addresses a second, per-script D-Bus
    // object (`/Scripting/Script{id}`, interface `org.kde.kwin.Script`)
    // whose `run` method is what actually executes the script; the
    // `org.kde.kwin.Scripting` interface used for `loadScript`/`unloadScript`
    // has no "run this script" method of its own; there's a `start` method
    // there, but see `run_script`'s module-level context — the third-party
    // `kdotool` (this technique's reference implementation) never calls it,
    // and calling it instead of the per-script `run` leaves a loaded script
    // sitting inert, causing every call here to silently time out. See
    // <https://github.com/jinliu/kdotool>'s `run_script` for the reference
    // sequence this mirrors.
    let load_reply = connection.call_method(
        Some(KWIN_SERVICE),
        SCRIPTING_PATH,
        Some(SCRIPTING_INTERFACE),
        "loadScript",
        &(script_path_str.as_str(), plugin_name.as_str()),
    );
    let script_id = match load_reply.and_then(|msg| msg.body().deserialize::<i32>()) {
        Ok(id) if id >= 0 => id,
        Ok(_) => {
            let _ = std::fs::remove_file(&script_path);
            return Err("failed to load KWin script: a script with this name may already exist".to_string());
        }
        Err(e) => {
            let _ = std::fs::remove_file(&script_path);
            return Err(format!("failed to load KWin script: {e}"));
        }
    };

    let script_path_dbus = format!("{SCRIPTING_PATH}/Script{script_id}");
    if let Err(e) = connection.call_method(Some(KWIN_SERVICE), script_path_dbus.as_str(), Some(SCRIPT_INTERFACE), "run", &()) {
        let _ = connection.call_method(Some(KWIN_SERVICE), SCRIPTING_PATH, Some(SCRIPTING_INTERFACE), "unloadScript", &(plugin_name.as_str(),));
        let _ = std::fs::remove_file(&script_path);
        return Err(format!("failed to run KWin script: {e}"));
    }
    // Matches the reference implementation's sequence; without it the
    // script instance stays resident (registered as "running") even after
    // its body has already finished and reported back.
    let _ = connection.call_method(Some(KWIN_SERVICE), script_path_dbus.as_str(), Some(SCRIPT_INTERFACE), "stop", &());

    let deadline = Instant::now() + SCRIPT_TIMEOUT;
    let reported = loop {
        if let Some(value) = result.lock().unwrap_or_else(|e| e.into_inner()).take() {
            break Some(value);
        }
        if Instant::now() >= deadline {
            break None;
        }
        std::thread::sleep(POLL_INTERVAL);
    };

    let _ = connection.call_method(Some(KWIN_SERVICE), SCRIPTING_PATH, Some(SCRIPTING_INTERFACE), "unloadScript", &(plugin_name.as_str(),));
    let _ = connection.object_server().remove::<Callback, _>(CALLBACK_PATH);
    let _ = std::fs::remove_file(&script_path);

    match reported {
        Some(value) if value.starts_with("__error__:") => Err(format!("KWin script error: {}", &value["__error__:".len()..])),
        other => Ok(other),
    }
}

/// `Some(text)` if non-blank, `None` otherwise — a blank `report()` means
/// "nothing focused"/"not a normal window", not literally an empty name.
fn non_blank(text: Option<String>) -> Option<String> {
    text.filter(|s| !s.is_empty())
}

pub fn focused_application() -> Option<String> {
    let body = r#"var w = workspace.activeWindow;
        if (w && w.normalWindow) { report(w.resourceClass || w.resourceName || ""); } else { report(""); }"#;
    non_blank(run_script(body).ok().flatten())
}

pub fn focused_window_title() -> Option<String> {
    let body = r#"var w = workspace.activeWindow;
        if (w && w.normalWindow) { report(w.caption || ""); } else { report(""); }"#;
    non_blank(run_script(body).ok().flatten())
}

fn reported_bool(result: Result<Option<String>, String>) -> bool {
    matches!(result.ok().flatten().as_deref(), Some("true"))
}

pub fn window_exists(app: Option<&str>, title_contains: Option<&str>) -> bool {
    let body = format!("{}\n        report(target !== null ? \"true\" : \"false\");", matcher_prelude(app, title_contains));
    reported_bool(run_script(&body))
}

pub fn focus_window(app: Option<&str>, title_contains: Option<&str>) -> Result<bool, String> {
    let body = format!(
        "{}\n        if (target !== null) {{ workspace.activeWindow = target; }}\n        report(target !== null ? \"true\" : \"false\");",
        matcher_prelude(app, title_contains)
    );
    run_script(&body).map(|r| r.as_deref() == Some("true"))
}

pub fn minimize_window(app: Option<&str>, title_contains: Option<&str>) -> Result<bool, String> {
    let body = format!(
        "{}\n        if (target !== null) {{ target.minimized = true; }}\n        report(target !== null ? \"true\" : \"false\");",
        matcher_prelude(app, title_contains)
    );
    run_script(&body).map(|r| r.as_deref() == Some("true"))
}

pub fn maximize_window(app: Option<&str>, title_contains: Option<&str>) -> Result<bool, String> {
    let body = format!(
        "{}\n        if (target !== null) {{ target.setMaximize(true, true); }}\n        report(target !== null ? \"true\" : \"false\");",
        matcher_prelude(app, title_contains)
    );
    run_script(&body).map(|r| r.as_deref() == Some("true"))
}

/// Sets/clears the "always keep above other windows" flag on the first
/// window matching both filters — used by the element picker to force its
/// overlay window above whatever app is being inspected, since the CEF/winit
/// runtime's own `always_on_top` has no effect on Wayland (see
/// `window::window_at_point`'s doc comment for the same
/// clients-can't-see-the-compositor's-state family of Wayland limitation;
/// this one instead is "clients can't ask to be raised", not "clients can't
/// know their position" — related, both worked around via KWin scripting).
pub fn set_keep_above(app: Option<&str>, title_contains: Option<&str>, keep_above: bool) -> Result<bool, String> {
    let body = format!(
        "{}\n        if (target !== null) {{ target.keepAbove = {keep_above}; }}\n        report(target !== null ? \"true\" : \"false\");",
        matcher_prelude(app, title_contains)
    );
    run_script(&body).map(|r| r.as_deref() == Some("true"))
}

pub fn move_window(app: Option<&str>, title_contains: Option<&str>, x: i32, y: i32) -> Result<bool, String> {
    let body = format!(
        r#"{}
        if (target !== null) {{
            var g = target.frameGeometry;
            target.frameGeometry = {{ x: {x}, y: {y}, width: g.width, height: g.height }};
        }}
        report(target !== null ? "true" : "false");"#,
        matcher_prelude(app, title_contains)
    );
    run_script(&body).map(|r| r.as_deref() == Some("true"))
}

/// Forces both the position *and* size of the first window matching both
/// filters — same Wayland limitation `set_keep_above`'s doc comment
/// describes, but for geometry: a plain `xdg_toplevel` never gets to pick
/// its own on-screen position (the compositor alone decides that), and KWin
/// additionally clamps a freshly-mapped toplevel's initial size to whichever
/// single output it lands the window on. That's the "the element picker
/// overlay only covers the current screen" bug — the overlay's own
/// `position`/`inner_size` builder calls are honored only as size hints, not
/// as a request to span the whole multi-monitor desktop. KWin itself has no
/// such restriction: as the compositor, it can put any window's frame at any
/// rectangle, including one spanning multiple outputs, so this is called
/// right after the overlay window is created to force it across the full
/// union of monitors that `start_element_picker` already computed.
pub fn set_window_geometry(app: Option<&str>, title_contains: Option<&str>, x: i32, y: i32, width: i32, height: i32) -> Result<bool, String> {
    let body = format!(
        r#"{}
        if (target !== null) {{
            target.frameGeometry = {{ x: {x}, y: {y}, width: {width}, height: {height} }};
        }}
        report(target !== null ? "true" : "false");"#,
        matcher_prelude(app, title_contains)
    );
    run_script(&body).map(|r| r.as_deref() == Some("true"))
}

/// Finds the topmost normal window whose frame contains screen point
/// `(x, y)`, with its *true* geometry as KWin itself sees it.
///
/// This exists because a native Wayland client cannot know its own
/// on-screen position (Wayland deliberately never tells clients this, for
/// isolation) — confirmed live: AT-SPI's `Component` interface on such a
/// client reports made-up/self-relative "screen" coordinates instead
/// (e.g. a window positioned at screen x≈966 reporting its own AT-SPI frame
/// extents as x=0). KWin, as the compositor, has no such blind spot. The
/// element picker combines this window-level truth with AT-SPI's
/// window-*relative* coordinates (which stay self-consistent even though
/// its "screen" ones lie) — see `accessibility::linux::element_at_point`.
pub fn window_at_point(x: i32, y: i32, excluded_title: Option<&str>) -> Option<crate::window::WindowAtPoint> {
    let excluded_title_json =
        serde_json::to_string(&excluded_title.unwrap_or("").to_lowercase()).unwrap_or_else(|_| "\"\"".to_string());
    let body = format!(
        r#"var excludedTitle = {excluded_title_json};
        var list = workspace.stackingOrder;
        var target = null;
        for (var i = list.length - 1; i >= 0; i--) {{
            var w = list[i];
            if (!w.normalWindow) continue;
            // A minimized window keeps its last geometry but isn't visible —
            // without this the picker would "see through" to windows the
            // user can't see or click.
            if (w.minimized) continue;
            if (excludedTitle && (w.caption || '').toLowerCase() === excludedTitle) continue;
            var g = w.frameGeometry;
            if ({x} >= g.x && {x} < g.x + g.width && {y} >= g.y && {y} < g.y + g.height) {{
                target = w;
                break;
            }}
        }}
        if (target !== null) {{
            var g = target.frameGeometry;
            report(JSON.stringify({{
                app: target.resourceClass || target.resourceName || "",
                title: target.caption || "",
                x: g.x, y: g.y, width: g.width, height: g.height
            }}));
        }} else {{
            report("");
        }}"#
    );
    let reported = run_script(&body).ok().flatten()?;
    if reported.is_empty() {
        return None;
    }
    // KWin's `frameGeometry` can report fractional coordinates (observed
    // live under fractional display scaling — a window's placement math can
    // land on a non-integer logical pixel even though its own width/height
    // come out as clean integers), so this parses as `f64` and rounds,
    // rather than `i32` directly (which silently fails to deserialize a
    // fractional value and made every call here return `None`).
    #[derive(serde::Deserialize)]
    struct RawHit {
        app: String,
        title: String,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    }
    let raw: RawHit = serde_json::from_str(&reported).ok()?;
    Some(crate::window::WindowAtPoint {
        app: raw.app,
        title: raw.title,
        x: raw.x.round() as i32,
        y: raw.y.round() as i32,
        width: raw.width.round() as i32,
        height: raw.height.round() as i32,
    })
}
