//! Accessibility-tree queries/actions via AT-SPI2, over D-Bus. Unlike
//! `window::linux` (split between KWin-scripting and X11 depending on
//! session type), AT-SPI is a desktop-agnostic accessibility bus — the same
//! query works under GNOME/KDE, X11/Wayland, with no session detection
//! needed, as long as the target app's toolkit has an AT-SPI bridge active
//! (GTK/Qt do by default once accessibility is requested — see
//! `set_session_accessibility` below).
//!
//! Every public function here drives the crate's async proxies with a
//! single `futures_lite::block_on` per call — no tokio runtime, same
//! tokio-free "one-shot async client" spirit as `window::linux::kwin_dbus`
//! (see its module doc for why a fresh connection per call, rather than a
//! cached handle, is this codebase's deliberate choice here).

use super::{ElementLocator, ElementScope, PickedElement};
use atspi::proxy::accessible::{AccessibleProxy, ObjectRefExt};
use atspi::proxy::action::ActionProxy;
use atspi::proxy::component::ComponentProxy;
use atspi::proxy::editable_text::EditableTextProxy;
use atspi::proxy::text::TextProxy;
use atspi::proxy::value::ValueProxy;
use atspi::{CoordType, ObjectRefOwned, State};
use atspi_connection::AccessibilityConnection;
use zbus::Connection;

/// Hard cap on how many accessible nodes one locator search will visit
/// before giving up. A tree walk here is network-bound (one or more D-Bus
/// round trips per node), not CPU-bound, so a node budget is a far more
/// predictable backstop against a pathologically large tree (a complex
/// browser window, say) than a wall-clock timeout would be — same
/// protective spirit as `kwin_dbus.rs`'s `SCRIPT_TIMEOUT`.
const MAX_NODES_VISITED: usize = 4000;

/// `Some(text)` for a non-blank filter, `None` for "no filter" — mirrors
/// `window::non_empty`, just kept local since this module doesn't depend on
/// `input::value`.
fn non_empty(s: Option<&str>) -> Option<&str> {
    s.filter(|s| !s.is_empty())
}

/// Case-insensitive substring match; a blank/absent `needle` matches
/// anything (consistent with every app/title filter elsewhere in this
/// codebase — see `window::window_exists`'s doc comment).
fn matches_needle(haystack: &str, needle: Option<&str>) -> bool {
    match non_empty(needle) {
        None => true,
        Some(n) => haystack.to_lowercase().contains(&n.to_lowercase()),
    }
}

/// Case-insensitive substring match in *either* direction — used only to
/// compare an AT-SPI application's `Name` against `window::focused_application`'s
/// window-manager-reported class (`ElementScope::Focused`). The two
/// vocabularies don't agree on style: AT-SPI tends to report a bare app name
/// ("kcalc") while KWin reports a reverse-DNS resource class
/// ("org.kde.kcalc") — confirmed live, one contains the other but a plain
/// one-directional `haystack.contains(needle)` (right for every *user-typed*
/// app filter, so kept as `matches_needle` for those) picks the wrong
/// direction here and never matches.
fn matches_focused_app(atspi_name: &str, focused_class: &str) -> bool {
    if focused_class.is_empty() {
        return true;
    }
    let (a, b) = (atspi_name.to_lowercase(), focused_class.to_lowercase());
    a.contains(&b) || b.contains(&a)
}

/// Case-insensitive *exact* match (after trimming) — unlike the other
/// filters, role names are a small fixed vocabulary (`"push button"`,
/// `"check box"`, ...), so a substring match would risk one role
/// accidentally matching another (e.g. `"button"` inside `"toggle
/// button"`). A blank/absent filter matches any role.
fn role_matches(role_name: &str, wanted: Option<&str>) -> bool {
    match non_empty(wanted) {
        None => true,
        Some(w) => role_name.trim().eq_ignore_ascii_case(w.trim()),
    }
}

async fn connect() -> Result<AccessibilityConnection, String> {
    // Best-effort: tells already-running GTK/Qt apps accessibility is
    // wanted so their AT-SPI bridge activates, but isn't fatal if it fails
    // (e.g. a locked-down session) — `AccessibilityConnection::new` below
    // still works against apps whose bridge is already active regardless.
    let _ = atspi_connection::set_session_accessibility(true).await;
    AccessibilityConnection::new()
        .await
        .map_err(|e| format!("AT-SPI bus unavailable: {e}"))
}

async fn as_accessible<'a>(conn: &'a Connection, obj: &'a ObjectRefOwned) -> Option<AccessibleProxy<'a>> {
    obj.as_accessible_proxy(conn).await.ok()
}

async fn text_proxy<'a>(conn: &'a Connection, obj: &'a ObjectRefOwned) -> zbus::Result<TextProxy<'a>> {
    TextProxy::builder(conn)
        .destination(obj.name_as_str().unwrap_or_default())?
        .path(obj.path_as_str())?
        .build()
        .await
}

async fn editable_text_proxy<'a>(conn: &'a Connection, obj: &'a ObjectRefOwned) -> zbus::Result<EditableTextProxy<'a>> {
    EditableTextProxy::builder(conn)
        .destination(obj.name_as_str().unwrap_or_default())?
        .path(obj.path_as_str())?
        .build()
        .await
}

async fn value_proxy<'a>(conn: &'a Connection, obj: &'a ObjectRefOwned) -> zbus::Result<ValueProxy<'a>> {
    ValueProxy::builder(conn)
        .destination(obj.name_as_str().unwrap_or_default())?
        .path(obj.path_as_str())?
        .build()
        .await
}

async fn action_proxy<'a>(conn: &'a Connection, obj: &'a ObjectRefOwned) -> zbus::Result<ActionProxy<'a>> {
    ActionProxy::builder(conn)
        .destination(obj.name_as_str().unwrap_or_default())?
        .path(obj.path_as_str())?
        .build()
        .await
}

async fn component_proxy<'a>(conn: &'a Connection, obj: &'a ObjectRefOwned) -> zbus::Result<ComponentProxy<'a>> {
    ComponentProxy::builder(conn)
        .destination(obj.name_as_str().unwrap_or_default())?
        .path(obj.path_as_str())?
        .build()
        .await
}

/// Depth-first search of `root`'s descendants (never `root` itself) for the
/// first node whose name/role match `locator`. `budget` is shared across
/// however many subtrees `resolve` searches, so the *total* work across an
/// entire locator resolution is bounded, not just one subtree's.
async fn find_in_subtree(
    conn: &Connection,
    root: &AccessibleProxy<'_>,
    locator: &ElementLocator<'_>,
    budget: &mut usize,
) -> Option<ObjectRefOwned> {
    let mut stack: Vec<ObjectRefOwned> = root
        .get_children()
        .await
        .ok()?
        .into_iter()
        .filter(|c| !c.is_null())
        .collect();

    while let Some(candidate) = stack.pop() {
        if *budget == 0 {
            return None;
        }
        *budget -= 1;

        let Some(node) = as_accessible(conn, &candidate).await else { continue };
        let name = node.name().await.unwrap_or_default();
        let role_name = node.get_role_name().await.unwrap_or_default();
        if matches_needle(&name, locator.name) && role_matches(&role_name, locator.role) {
            return Some(candidate);
        }

        if let Ok(children) = node.get_children().await {
            stack.extend(children.into_iter().filter(|c| !c.is_null()));
        }
    }
    None
}

/// Resolves a full locator to the first matching element: finds a running,
/// AT-SPI-registered application matching the effective app filter (blank
/// matches any), narrows to one of its top-level windows by the effective
/// title filter when given (searches every window the app owns otherwise),
/// then searches that scope — including the window/application node itself
/// — for a descendant matching `name`/`role`.
///
/// `locator.scope == Focused` overrides the effective app/title filters
/// with whichever window currently has OS focus (`locator.app`/
/// `title_contains` are ignored in that case) rather than searching every
/// running app; `App`/`Window` each consult only their own one field (the
/// other is ignored) — see `ElementScope`'s doc comment.
async fn resolve(locator: &ElementLocator<'_>) -> Option<(Connection, ObjectRefOwned)> {
    let (focused_app, focused_title);
    let (app_filter, title_filter) = match locator.scope {
        ElementScope::Focused => {
            focused_app = crate::window::focused_application()?;
            focused_title = crate::window::focused_window_title();
            (Some(focused_app.as_str()), focused_title.as_deref())
        }
        ElementScope::App => (locator.app, None),
        ElementScope::Window => (None, locator.title_contains),
    };

    let a11y = connect().await.ok()?;
    let conn = a11y.connection().clone();
    let root = a11y.root_accessible_on_registry().await.ok()?;
    let apps = root.get_children().await.ok()?;

    let mut budget = MAX_NODES_VISITED;
    for app_ref in apps.into_iter().filter(|a| !a.is_null()) {
        let Some(app) = as_accessible(&conn, &app_ref).await else { continue };
        let app_name = app.name().await.unwrap_or_default();
        let app_matches = match locator.scope {
            // See `matches_focused_app`'s doc comment — AT-SPI's app `Name`
            // and the window manager's resource class disagree on style
            // often enough that a plain one-directional substring check
            // (right for a user-typed filter) misses real matches here.
            ElementScope::Focused => matches_focused_app(&app_name, app_filter.unwrap_or_default()),
            // `Window` scope leaves `app_filter` at `None` above, so this is
            // an unconditional match here — every app's windows are searched
            // and only `title_filter` narrows things down.
            ElementScope::App | ElementScope::Window => matches_needle(&app_name, app_filter),
        };
        if !app_matches {
            continue;
        }

        let windows: Vec<ObjectRefOwned> = if non_empty(title_filter).is_some() {
            let Ok(children) = app.get_children().await else { continue };
            let mut matched = Vec::new();
            for child in children.into_iter().filter(|c| !c.is_null()) {
                if let Some(w) = as_accessible(&conn, &child).await {
                    let title = w.name().await.unwrap_or_default();
                    if matches_needle(&title, title_filter) {
                        matched.push(child);
                    }
                }
            }
            matched
        } else {
            vec![app_ref]
        };

        for window_ref in windows {
            let Some(window) = as_accessible(&conn, &window_ref).await else { continue };
            let name = window.name().await.unwrap_or_default();
            let role_name = window.get_role_name().await.unwrap_or_default();
            if matches_needle(&name, locator.name) && role_matches(&role_name, locator.role) {
                return Some((conn, window_ref));
            }
            if let Some(found) = find_in_subtree(&conn, &window, locator, &mut budget).await {
                return Some((conn, found));
            }
            if budget == 0 {
                return None;
            }
        }
    }
    None
}

pub fn element_exists(locator: &ElementLocator) -> bool {
    futures_lite::future::block_on(async { resolve(locator).await.is_some() })
}

pub fn element_enabled(locator: &ElementLocator) -> bool {
    futures_lite::future::block_on(async {
        let Some((conn, obj)) = resolve(locator).await else { return false };
        let Some(accessible) = as_accessible(&conn, &obj).await else { return false };
        accessible.get_state().await.map(|s| s.contains(State::Enabled)).unwrap_or(false)
    })
}

pub fn element_text(locator: &ElementLocator) -> Option<String> {
    futures_lite::future::block_on(async {
        let (conn, obj) = resolve(locator).await?;

        // `get_text` (a method) rather than `character_count` (a property)
        // + a bounded range — same "avoid this interface's properties"
        // reasoning as `click_element`'s `get_actions` choice; `-1` is
        // AT-SPI's documented "to the end of the text" sentinel for
        // `end_offset`.
        if let Ok(text) = text_proxy(&conn, &obj).await {
            if let Ok(s) = text.get_text(0, -1).await {
                if !s.is_empty() {
                    return Some(s);
                }
            }
        }
        if let Ok(value) = value_proxy(&conn, &obj).await {
            if let Ok(v) = value.current_value().await {
                return Some(format_number(v));
            }
        }
        let accessible = as_accessible(&conn, &obj).await?;
        accessible.name().await.ok().filter(|s| !s.is_empty())
    })
}

pub fn click_element(locator: &ElementLocator) -> Result<bool, String> {
    futures_lite::future::block_on(async {
        let Some((conn, obj)) = resolve(locator).await else { return Ok(false) };
        let action = action_proxy(&conn, &obj).await.map_err(|e| format!("element has no Action interface: {e}"))?;
        // Deliberately `get_actions` (a method) rather than the `NActions`
        // property + per-index `get_name` calls the AT-SPI spec suggests:
        // at least one real-world AT-SPI2/Qt6 bridge observed during
        // development implements this interface's methods but answers its
        // *properties* with `UnknownInterface`, so leaning on `NActions`
        // would make `ClickElement` spuriously fail against otherwise
        // perfectly clickable elements.
        let actions = action.get_actions().await.map_err(|e| format!("failed to read element's actions: {e}"))?;
        if actions.is_empty() {
            return Err("element exposes no actions to invoke".to_string());
        }

        let index = actions
            .iter()
            .position(|a| matches!(a.name.to_lowercase().as_str(), "click" | "press" | "activate" | "jump" | "toggle"))
            .unwrap_or(0);
        action.do_action(index as i32).await.map_err(|e| format!("failed to invoke action: {e}"))
    })
}

pub fn set_element_value(locator: &ElementLocator, value: &str) -> Result<bool, String> {
    futures_lite::future::block_on(async {
        let Some((conn, obj)) = resolve(locator).await else { return Ok(false) };

        if let Ok(editable) = editable_text_proxy(&conn, &obj).await {
            if let Ok(ok) = editable.set_text_contents(value).await {
                return Ok(ok);
            }
        }
        if let Ok(numeric) = value_proxy(&conn, &obj).await {
            let parsed: f64 = value
                .trim()
                .parse()
                .map_err(|_| format!("\"{value}\" isn't a number, and the element has no editable text"))?;
            return numeric.set_current_value(parsed).await.map(|()| true).map_err(|e| format!("failed to set value: {e}"));
        }
        Err("element supports neither editable text nor a numeric value".to_string())
    })
}

/// Backs the "pick from screen" element picker — see its doc comment in
/// `accessibility::mod` for the caching contract around `known_window`.
///
/// Combines two systems because neither alone is enough: a native Wayland
/// client can't know its own on-screen position (confirmed live: AT-SPI's
/// `Component.GetExtents(Screen)` on one reports a made-up, self-relative
/// origin, e.g. a window actually at screen x≈1056 reporting its own frame
/// extents as x=0), so `crate::window::window_at_point` — backed by KWin's
/// own compositor state on Wayland, real X11 geometry on X11 — is the only
/// trustworthy source for *which* window is at a screen point and *where it
/// really is*. AT-SPI's `CoordType::Window` coordinates stay self-consistent
/// even though its `Screen` ones lie, so the screen point is converted to
/// that window's local coordinates before asking AT-SPI to hit-test within
/// it (`find_leaf_at_point`).
pub fn element_at_point(x: i32, y: i32, known_window: Option<crate::window::WindowAtPoint>) -> Option<PickedElement> {
    futures_lite::future::block_on(async {
        let win = match known_window {
            Some(w) if x >= w.x && x < w.x + w.width && y >= w.y && y < w.y + w.height => w,
            _ => crate::window::window_at_point_excluding(x, y, crate::window::ELEMENT_PICKER_OVERLAY_TITLE)?,
        };
        tracing::warn!(x, y, app = %win.app, title = %win.title, window_x = win.x, window_y = win.y, width = win.width, height = win.height, "element picker selected compositor window");
        let (local_x, local_y) = (x - win.x, y - win.y);

        // Window-level fallback: many on-screen targets (CEF/Electron apps
        // like this one, video surfaces, the desktop itself) expose no
        // AT-SPI element at the cursor. Returning the compositor window
        // itself — with empty name/role and window bounds — keeps the
        // overlay highlightable and the click meaningful (window-action
        // blocks only need app/title) instead of silently cancelling the
        // pick, which is what a bare `None` here used to do.
        let fallback = PickedElement {
            app: win.app.clone(),
            window_title: win.title.clone(),
            name: String::new(),
            role: String::new(),
            x: win.x,
            y: win.y,
            width: win.width,
            height: win.height,
            window: win.clone(),
        };

        let Ok(a11y) = connect().await else {
            return Some(fallback);
        };
        let conn = a11y.connection().clone();
        let Ok(root) = a11y.root_accessible_on_registry().await else {
            return Some(fallback);
        };
        let Ok(apps) = root.get_children().await else {
            return Some(fallback);
        };

        for app_ref in apps.into_iter().filter(|a| !a.is_null()) {
            let Some(app) = as_accessible(&conn, &app_ref).await else { continue };
            let app_name = app.name().await.unwrap_or_default();
            // See `matches_focused_app`'s doc comment — `win.app` comes from
            // KWin's `resourceClass`, the same vocabulary mismatch
            // `ElementScope::Focused` already has to handle.
            if !matches_focused_app(&app_name, &win.app) {
                continue;
            }
            tracing::warn!(app = %app_name, "element picker matched accessibility application");
            let Ok(windows) = app.get_children().await else { continue };
            for window_ref in windows.into_iter().filter(|w| !w.is_null()) {
                let Some(window) = as_accessible(&conn, &window_ref).await else { continue };
                let window_name = window.name().await.unwrap_or_default();
                if !win.title.is_empty() && !matches_needle(&window_name, Some(&win.title)) {
                    continue;
                }
                tracing::warn!(title = %window_name, local_x, local_y, "element picker matched accessibility window");
                let Ok(component) = component_proxy(&conn, &window_ref).await else { continue };
                let Ok(mut current) = component.get_accessible_at_point(local_x, local_y, CoordType::Window).await else { continue };
                if current.is_null() {
                    continue;
                }

                // Descend further with the SAME window-relative coordinates
                // — `CoordType::Window` means "relative to the containing
                // top-level frame" for every descendant, not relative to
                // each node's own bounds — until a call stops finding
                // something deeper (confirmed live: some elements are
                // already the leaf on the first call, others need this).
                loop {
                    let Ok(deeper_component) = component_proxy(&conn, &current).await else { break };
                    match deeper_component.get_accessible_at_point(local_x, local_y, CoordType::Window).await {
                        Ok(deeper) if !deeper.is_null() && deeper.path_as_str() != current.path_as_str() => current = deeper,
                        _ => break,
                    }
                }

                let Ok(leaf_component) = component_proxy(&conn, &current).await else { continue };
                let Ok((lx, ly, lw, lh)) = leaf_component.get_extents(CoordType::Window).await else { continue };
                let Some(accessible) = as_accessible(&conn, &current).await else { continue };
                let name = accessible.name().await.unwrap_or_default();
                let role = accessible.get_role_name().await.unwrap_or_default();
                return Some(PickedElement {
                    app: win.app.clone(),
                    window_title: win.title.clone(),
                    name,
                    role,
                    x: win.x + lx,
                    y: win.y + ly,
                    width: lw,
                    height: lh,
                    window: win,
                });
            }
        }
        Some(fallback)
    })
}

/// Matches `Evaluated::Number`'s own text formatting closely enough for a
/// slider/progress-bar reading to look natural (no `"3.0"` for a whole
/// number) without pulling in that formatting logic from `input::value`.
fn format_number(n: f64) -> String {
    if n.fract() == 0.0 && n.is_finite() {
        format!("{n:.0}")
    } else {
        n.to_string()
    }
}
