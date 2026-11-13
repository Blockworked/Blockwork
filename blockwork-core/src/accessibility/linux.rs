use super::*;
use crate::input::value::Evaluated;
use crate::window::ScreenWindow;
use atspi::proxy::{
    accessible::AccessibleProxy, action::ActionProxy, component::ComponentProxy,
    editable_text::EditableTextProxy, text::TextProxy, value::ValueProxy,
};
use atspi::{CoordType, ObjectRefOwned, Role, State};
use atspi_connection::AccessibilityConnection;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use zbus::{Connection, proxy::CacheProperties};

const LIMIT_NODES: usize = 4000;
const LIMIT_DEPTH: usize = 64;

fn bounded<T>(future: impl std::future::Future<Output = Result<T, String>>) -> Result<T, String> {
    let _runtime = crate::dbus::enter();
    futures_lite::future::block_on(async {
        match tokio::time::timeout(Duration::from_secs(3), future).await {
            Ok(result) => result,
            Err(_) => {
                forget_connection();
                Err("AT-SPI query timed out after 3 seconds".to_string())
            }
        }
    })
}
async fn connect() -> Result<AccessibilityConnection, String> {
    let _ = atspi_connection::set_session_accessibility(true).await;
    if let Ok(address) = std::env::var("AT_SPI_BUS_ADDRESS") {
        AccessibilityConnection::from_address(
            address
                .parse()
                .map_err(|e| format!("Invalid AT-SPI bus address: {e}"))?,
        )
        .await
        .map_err(|e| e.to_string())
    } else {
        AccessibilityConnection::new()
            .await
            .map_err(|e| format!("AT-SPI accessibility bus unavailable: {e}"))
    }
}

static SHARED: Mutex<Option<AccessibilityConnection>> = Mutex::new(None);
fn forget_connection() {
    *SHARED.lock().unwrap_or_else(|p| p.into_inner()) = None;
}
/// One connection kept for every query, made on the shared D-Bus runtime so
/// it outlives whichever thread asked first.
async fn shared() -> Result<AccessibilityConnection, String> {
    if let Some(connection) = SHARED.lock().unwrap_or_else(|p| p.into_inner()).clone() {
        return Ok(connection);
    }
    let connection = match crate::dbus::handle() {
        Some(handle) => handle.spawn(connect()).await.map_err(|e| e.to_string())??,
        None => connect().await?,
    };
    *SHARED.lock().unwrap_or_else(|p| p.into_inner()) = Some(connection.clone());
    Ok(connection)
}
/// The registry's applications; a failure here means a stale connection.
async fn apps(connection: &AccessibilityConnection) -> Result<Vec<ObjectRefOwned>, String> {
    let result = async {
        let root = connection
            .root_accessible_on_registry()
            .await
            .map_err(|e| e.to_string())?;
        root.get_children().await.map_err(|e| e.to_string())
    }
    .await;
    if result.is_err() {
        forget_connection();
    }
    result
}
async fn accessible<'a>(
    connection: &'a Connection,
    object: &'a ObjectRefOwned,
) -> Result<AccessibleProxy<'a>, String> {
    AccessibleProxy::builder(connection)
        .destination(object.name_as_str().unwrap_or_default())
        .map_err(|e| e.to_string())?
        .path(object.path_as_str())
        .map_err(|e| e.to_string())?
        .cache_properties(CacheProperties::No)
        .build()
        .await
        .map_err(|e| e.to_string())
}
async fn component<'a>(
    connection: &'a Connection,
    object: &'a ObjectRefOwned,
) -> Result<ComponentProxy<'a>, String> {
    ComponentProxy::builder(connection)
        .destination(object.name_as_str().unwrap_or_default())
        .map_err(|e| e.to_string())?
        .path(object.path_as_str())
        .map_err(|e| e.to_string())?
        .cache_properties(CacheProperties::No)
        .build()
        .await
        .map_err(|e| e.to_string())
}
fn key(object: &ObjectRefOwned) -> String {
    format!(
        "{}{}",
        object.name_as_str().unwrap_or_default(),
        object.path_as_str()
    )
}
fn contains(value: &str, filter: &str) -> bool {
    filter.is_empty() || value.to_lowercase().contains(&filter.to_lowercase())
}
fn is_window(role: Role) -> bool {
    matches!(
        role,
        Role::Frame | Role::Dialog | Role::Window | Role::Alert
    )
}
fn enabled_in(state: &atspi::StateSet) -> bool {
    state.contains(State::Enabled) && state.contains(State::Sensitive)
}

/// One element a search walked past.
struct Found {
    object: ObjectRefOwned,
    node: UiNode,
    showing: bool,
}
fn matches(selector: &UiSelector, window_filter: &str, node: &UiNode) -> bool {
    contains(&node.app, &selector.app)
        && contains(&node.window, window_filter)
        && contains(&node.name, &selector.name)
        && (selector.role.is_empty() || node.role.eq_ignore_ascii_case(&selector.role))
        && (selector.id.is_empty() || node.id == selector.id)
}

/// The window that has focus, by its own AT-SPI state.
async fn focused_frame(
    connection: &AccessibilityConnection,
    apps: &[ObjectRefOwned],
) -> Option<(ObjectRefOwned, String, String)> {
    for app in apps {
        let Ok(proxy) = accessible(connection.connection(), app).await else {
            continue;
        };
        let Ok(frames) = proxy.get_children().await else {
            continue;
        };
        for frame in frames {
            let Ok(frame_proxy) = accessible(connection.connection(), &frame).await else {
                continue;
            };
            if frame_proxy
                .get_state()
                .await
                .is_ok_and(|s| s.contains(State::Active))
            {
                let app_name = proxy.name().await.unwrap_or_default();
                let window = frame_proxy.name().await.unwrap_or_default();
                return Some((frame, app_name, window));
            }
        }
    }
    None
}

/// Every element matching `selector`, breadth first, at most `limit`.
/// `ids` reads each element's accessible ID even when nothing filters on it.
async fn find(
    connection: &AccessibilityConnection,
    selector: &UiSelector,
    limit: usize,
    ids: bool,
) -> Result<Vec<Found>, String> {
    let apps = apps(connection).await?;
    let mut queue = VecDeque::new();
    let mut window_filter = selector.window.clone();
    if selector.scope == UiScope::Focused {
        if let Some((frame, app, window)) = focused_frame(connection, &apps).await {
            // Searching from the frame keeps other windows out entirely.
            queue.push_back((frame, app, window, 1usize));
            window_filter.clear();
        } else {
            let title = crate::window::active().map(|w| w.title).unwrap_or_default();
            if title.is_empty() {
                return Err(
                    "Couldn't tell which window has focus; pick the element so its app is saved"
                        .into(),
                );
            }
            window_filter = title;
        }
    }
    if queue.is_empty() {
        for app in apps {
            let Ok(proxy) = accessible(connection.connection(), &app).await else {
                continue;
            };
            let name = proxy.name().await.unwrap_or_default();
            if contains(&name, &selector.app) {
                queue.push_back((app, name, String::new(), 0usize));
            }
        }
    }
    let mut seen = HashSet::new();
    let mut result = vec![];
    while let Some((object, app, mut window, depth)) = queue.pop_front() {
        if !seen.insert(key(&object)) {
            continue;
        }
        if seen.len() > LIMIT_NODES {
            return Err(
                "AT-SPI search exceeded 4,000 nodes; narrow the app or window filter".into(),
            );
        }
        if depth > LIMIT_DEPTH {
            return Err("AT-SPI search exceeded 64 levels; narrow the selector".into());
        }
        let Ok(proxy) = accessible(connection.connection(), &object).await else {
            continue;
        };
        let Ok(role) = proxy.get_role().await else {
            continue;
        };
        if role == Role::PasswordText {
            continue;
        }
        let name = proxy.name().await.unwrap_or_default();
        if is_window(role) {
            window = name.clone();
        }
        let role_name = proxy.get_role_name().await.unwrap_or_default();
        let id = if ids || !selector.id.is_empty() {
            proxy.accessible_id().await.unwrap_or_default()
        } else {
            String::new()
        };
        let mut node = UiNode {
            app: app.clone(),
            window: window.clone(),
            name,
            role: role_name,
            id,
            enabled: false,
            selector: UiSelector::default(),
            text: String::new(),
            matches: 0,
        };
        if matches(selector, &window_filter, &node) {
            let state = proxy.get_state().await.unwrap_or_default();
            node.enabled = enabled_in(&state);
            if !ids && node.id.is_empty() {
                node.id = proxy.accessible_id().await.unwrap_or_default();
            }
            node.selector = UiSelector {
                scope: UiScope::App,
                app: app.clone(),
                window: window.clone(),
                name: node.name.clone(),
                role: node.role.clone(),
                id: node.id.clone(),
            };
            node.text = node.selector.to_text();
            result.push(Found {
                object: object.clone(),
                node,
                showing: state.contains(State::Showing),
            });
            if result.len() >= limit {
                return Ok(result);
            }
        }
        if let Ok(children) = proxy.get_children().await {
            for child in children {
                queue.push_back((child, app.clone(), window.clone(), depth + 1));
            }
        }
    }
    Ok(result)
}

/// The one element a block means. Hidden copies (a closed menu's items, an
/// inactive tab) give way to the one on screen; otherwise more than one
/// match is an error, never a guess.
fn resolve(mut found: Vec<Found>) -> Result<Option<Found>, String> {
    if found.len() <= 1 {
        return Ok(found.pop());
    }
    let total = found.len();
    let mut showing: Vec<Found> = found.into_iter().filter(|f| f.showing).collect();
    if showing.len() == 1 {
        return Ok(showing.pop());
    }
    let count = if showing.is_empty() {
        total
    } else {
        showing.len()
    };
    Err(format!(
        "{count} UI elements match; narrow the selector's name, role, ID or window (Pick element writes one that matches only its element)"
    ))
}

/// What `resolve` would pick among `all` for `selector`, and how many
/// elements the selector matches.
fn resolve_in<'a>(all: &'a [Found], selector: &UiSelector) -> (Option<&'a Found>, usize) {
    let hits: Vec<&Found> = all
        .iter()
        .filter(|f| matches(selector, &selector.window, &f.node))
        .collect();
    if hits.len() == 1 {
        return (Some(hits[0]), 1);
    }
    let showing: Vec<&Found> = hits.iter().copied().filter(|f| f.showing).collect();
    if showing.len() == 1 {
        return (Some(showing[0]), 1);
    }
    let count = if showing.is_empty() {
        hits.len()
    } else {
        showing.len()
    };
    (None, count)
}

/// The shortest selector that finds `target` alone among `all`: by ID, by
/// role and name, then with the window added. Window titles often change
/// (an open document's name), so they're only added when needed.
fn shortest(all: &[Found], target: &Found) -> (UiSelector, usize) {
    let n = &target.node;
    let base = |window: bool, name: bool, role: bool, id: bool| UiSelector {
        scope: UiScope::App,
        app: n.app.clone(),
        window: if window {
            n.window.clone()
        } else {
            String::new()
        },
        name: if name { n.name.clone() } else { String::new() },
        role: if role { n.role.clone() } else { String::new() },
        id: if id { n.id.clone() } else { String::new() },
    };
    let mut candidates = vec![];
    if !n.id.is_empty() {
        candidates.push(base(false, false, false, true));
        candidates.push(base(false, false, true, true));
    }
    if !n.name.is_empty() {
        candidates.push(base(false, true, true, false));
        candidates.push(base(true, true, true, false));
    } else {
        candidates.push(base(false, false, true, false));
        candidates.push(base(true, false, true, false));
    }
    candidates.push(base(true, true, true, !n.id.is_empty()));
    let mut last = (candidates[candidates.len() - 1].clone(), 0);
    for candidate in candidates {
        if candidate.require_target().is_err() {
            continue;
        }
        let (picked, count) = resolve_in(all, &candidate);
        if picked.is_some_and(|p| key(&p.object) == key(&target.object)) {
            return (candidate, 1);
        }
        last = (candidate, count.max(1));
    }
    last
}

macro_rules! proxy {
    ($ty:ident,$connection:expr,$object:expr) => {
        $ty::builder($connection)
            .destination($object.name_as_str().unwrap_or_default())
            .map_err(|e| e.to_string())?
            .path($object.path_as_str())
            .map_err(|e| e.to_string())?
            .cache_properties(CacheProperties::No)
            .build()
            .await
            .map_err(|e| e.to_string())?
    };
}
pub fn applications() -> Result<Vec<String>, String> {
    bounded(async {
        let connection = shared().await?;
        let mut names = vec![];
        for object in apps(&connection).await? {
            if let Ok(proxy) = accessible(connection.connection(), &object).await
                && let Ok(name) = proxy.name().await
                && !name.is_empty()
            {
                names.push(name);
            }
        }
        names.sort();
        names.dedup();
        Ok(names)
    })
}
/// Elements of one app matching `selector`, each with the shortest
/// selector that finds it alone.
pub fn inspect(selector: &UiSelector) -> Result<Vec<UiNode>, String> {
    bounded(async {
        let connection = shared().await?;
        let everything = UiSelector {
            scope: UiScope::App,
            app: selector.app.clone(),
            ..UiSelector::default()
        };
        let all = find(&connection, &everything, LIMIT_NODES, true).await?;
        Ok(all
            .iter()
            .filter(|f| matches(selector, &selector.window, &f.node))
            .filter(|f| !f.node.role.is_empty() && f.node.role != "application")
            .take(100)
            .map(|f| {
                let (selector, count) = shortest(&all, f);
                let mut node = f.node.clone();
                node.text = selector.to_text();
                node.selector = selector;
                node.matches = count;
                node
            })
            .collect())
    })
}
pub fn query(selector: &UiSelector, query: Query) -> Result<Evaluated, String> {
    bounded(async {
        let connection = shared().await?;
        let target = resolve(find(&connection, selector, 20, false).await?)?;
        if matches!(query, Query::Exists) {
            return Ok(Evaluated::Bool(target.is_some()));
        }
        let Some(Found { object, node, .. }) = target else {
            return if matches!(query, Query::Enabled) {
                Ok(Evaluated::Bool(false))
            } else {
                Err("No UI element matches this selector".into())
            };
        };
        match query {
            Query::Exists => unreachable!(),
            Query::Enabled => Ok(Evaluated::Bool(node.enabled)),
            Query::Text => {
                let proxy = proxy!(TextProxy, connection.connection(), object);
                let text = proxy
                    .get_text(0, 65536)
                    .await
                    .map_err(|e| format!("Element has no readable text: {e}"))?;
                Ok(Evaluated::Text(text))
            }
            Query::Value => {
                let proxy = proxy!(ValueProxy, connection.connection(), object);
                let value = proxy.current_value().await.map_err(|e| e.to_string())?;
                if !value.is_finite() {
                    return Err("UI element returned a non-finite value".into());
                }
                Ok(Evaluated::Number(value))
            }
        }
    })
}
pub fn action(selector: &UiSelector, action: UiAction, value: &str) -> Result<(), String> {
    bounded(async {
        let connection = shared().await?;
        let Found { object, node, .. } = resolve(find(&connection, selector, 20, false).await?)?
            .ok_or("No UI element matches this selector")?;
        if !node.enabled {
            return Err("The matching UI element is disabled".into());
        }
        let success = match action {
            UiAction::Invoke => {
                let proxy = proxy!(ActionProxy, connection.connection(), object);
                // Single-action methods avoid toolkit bugs in aggregate GetActions/NActions.
                proxy
                    .get_name(0)
                    .await
                    .map_err(|e| format!("Element has no primary action: {e}"))?;
                proxy.do_action(0).await.map_err(|e| e.to_string())?
            }
            UiAction::SetText => {
                let proxy = proxy!(EditableTextProxy, connection.connection(), object);
                proxy
                    .set_text_contents(value)
                    .await
                    .map_err(|e| format!("Element isn't editable: {e}"))?
            }
            UiAction::SetValue => {
                let number: f64 = value
                    .trim()
                    .parse()
                    .map_err(|_| "UI value must be a finite number")?;
                let proxy = proxy!(ValueProxy, connection.connection(), object);
                let min = proxy.minimum_value().await.map_err(|e| e.to_string())?;
                let max = proxy.maximum_value().await.map_err(|e| e.to_string())?;
                if !number.is_finite()
                    || !min.is_finite()
                    || !max.is_finite()
                    || number < min
                    || number > max
                {
                    return Err("UI value is outside the element's numeric bounds".into());
                }
                proxy
                    .set_current_value(number)
                    .await
                    .map_err(|e| e.to_string())?;
                true
            }
            UiAction::Focus => {
                let proxy = proxy!(ComponentProxy, connection.connection(), object);
                proxy.grab_focus().await.map_err(|e| e.to_string())?
            }
        };
        if success {
            Ok(())
        } else {
            Err("The UI element refused this action".into())
        }
    })
}

// Picking from the screen.

/// The window list barely changes while the picker is open, and asking a
/// compositor costs a process or a script, so hovering reuses it briefly.
fn stacked() -> Result<Vec<ScreenWindow>, String> {
    type Listed = (Instant, Result<Vec<ScreenWindow>, String>);
    static CACHE: Mutex<Option<Listed>> = Mutex::new(None);
    let mut cache = CACHE.lock().unwrap_or_else(|p| p.into_inner());
    if let Some((at, list)) = cache.as_ref()
        && at.elapsed() < Duration::from_millis(800)
    {
        return list.clone();
    }
    let list = crate::window::stacked();
    *cache = Some((Instant::now(), list.clone()));
    list
}

/// The process behind an AT-SPI bus name, remembered per name.
async fn app_pid(connection: &Connection, bus: &str) -> u32 {
    static PIDS: Mutex<Option<HashMap<String, u32>>> = Mutex::new(None);
    if let Some(pid) = PIDS
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .get_or_insert_default()
        .get(bus)
    {
        return *pid;
    }
    let pid = async {
        let dbus = zbus::fdo::DBusProxy::new(connection).await.ok()?;
        let name = zbus::names::BusName::try_from(bus).ok()?;
        dbus.get_connection_unix_process_id(name).await.ok()
    }
    .await
    .unwrap_or(0);
    PIDS.lock()
        .unwrap_or_else(|p| p.into_inner())
        .get_or_insert_default()
        .insert(bus.to_string(), pid);
    pid
}

fn loosely(a: &str, b: &str) -> bool {
    let simple = |s: &str| {
        s.to_lowercase()
            .chars()
            .filter(|c| c.is_alphanumeric())
            .collect::<String>()
    };
    let (a, b) = (simple(a), simple(b));
    !a.is_empty() && !b.is_empty() && (a.contains(&b) || b.contains(&a))
}

/// The AT-SPI app and frame for a compositor window: by process first,
/// then by name, then the frame by its title.
async fn frame_for(
    connection: &AccessibilityConnection,
    window: &ScreenWindow,
) -> Result<Option<(String, ObjectRefOwned, String)>, String> {
    let conn = connection.connection();
    let mut by_pid = vec![];
    let mut by_name = vec![];
    for app in apps(connection).await? {
        let Ok(proxy) = accessible(conn, &app).await else {
            continue;
        };
        let name = proxy.name().await.unwrap_or_default();
        let pid = app_pid(conn, app.name_as_str().unwrap_or_default()).await;
        if window.pid != 0 && pid == window.pid {
            by_pid.push((app, name));
        } else if loosely(&name, &window.app) {
            by_name.push((app, name));
        }
    }
    for (app, name) in by_pid.into_iter().chain(by_name) {
        let Ok(proxy) = accessible(conn, &app).await else {
            continue;
        };
        let Ok(frames) = proxy.get_children().await else {
            continue;
        };
        let mut titled = vec![];
        for frame in frames {
            let Ok(frame_proxy) = accessible(conn, &frame).await else {
                continue;
            };
            let title = frame_proxy.name().await.unwrap_or_default();
            let state = frame_proxy.get_state().await.unwrap_or_default();
            titled.push((frame, title, state.contains(State::Showing)));
        }
        let exact = titled.iter().position(|(_, t, _)| *t == window.title);
        let close = || {
            titled
                .iter()
                .position(|(_, t, _)| loosely(t, &window.title))
        };
        let only = || {
            let showing: Vec<usize> = (0..titled.len()).filter(|i| titled[*i].2).collect();
            (showing.len() == 1).then(|| showing[0])
        };
        if let Some(i) = exact.or_else(close).or_else(only) {
            let (frame, title, _) = titled.swap_remove(i);
            return Ok(Some((name, frame, title)));
        }
    }
    Ok(None)
}

/// The deepest element at a point in `frame`, asking each level in turn.
async fn deepest(
    conn: &Connection,
    frame: &ObjectRefOwned,
    x: i32,
    y: i32,
    coords: CoordType,
) -> Option<ObjectRefOwned> {
    let mut current = frame.clone();
    for _ in 0..LIMIT_DEPTH {
        let proxy = component(conn, &current).await.ok()?;
        match proxy.get_accessible_at_point(x, y, coords).await {
            Ok(deeper) if !deeper.is_null() && key(&deeper) != key(&current) => current = deeper,
            _ => break,
        }
    }
    (key(&current) != key(frame)).then_some(current)
}

/// A plain X11 session, where AT-SPI's screen coordinates are true. On
/// Wayland only the compositor knows where a window is.
fn x11_session() -> bool {
    std::env::var("WAYLAND_DISPLAY")
        .unwrap_or_default()
        .is_empty()
        && std::env::var("XDG_SESSION_TYPE").unwrap_or_default() != "wayland"
}

async fn describe(
    conn: &Connection,
    object: &ObjectRefOwned,
    app: &str,
    window: &str,
    offset: (f64, f64),
    coords: CoordType,
) -> Option<UiHit> {
    let proxy = accessible(conn, object).await.ok()?;
    let role = proxy.get_role().await.ok()?;
    let (x, y, width, height) = component(conn, object)
        .await
        .ok()?
        .get_extents(coords)
        .await
        .ok()?;
    let state = proxy.get_state().await.unwrap_or_default();
    let password = role == Role::PasswordText;
    Some(UiHit {
        app: app.into(),
        window: window.into(),
        name: if password {
            String::new()
        } else {
            proxy.name().await.unwrap_or_default()
        },
        role: proxy.get_role_name().await.unwrap_or_default(),
        enabled: enabled_in(&state),
        x: x as f64 + offset.0,
        y: y as f64 + offset.1,
        width: width as f64,
        height: height as f64,
        window_only: false,
    })
}

async fn hit_at(
    connection: &AccessibilityConnection,
    x: f64,
    y: f64,
    ignore: &str,
) -> Result<Option<(UiHit, Option<ObjectRefOwned>)>, String> {
    let conn = connection.connection();
    let windows = match stacked() {
        Ok(list) => list,
        Err(_) => return hit_by_screen(connection, x, y).await,
    };
    let Some(window) = windows
        .into_iter()
        // Qt may append the app name to a title, after a dash.
        .find(|w| w.contains(x, y) && (ignore.is_empty() || !w.title.starts_with(ignore)))
    else {
        return Ok(None);
    };
    let window_hit = UiHit {
        app: window.app.clone(),
        window: window.title.clone(),
        name: String::new(),
        role: String::new(),
        enabled: true,
        x: window.bounds.x,
        y: window.bounds.y,
        width: window.bounds.width,
        height: window.bounds.height,
        window_only: true,
    };
    let Some((app, frame, title)) = frame_for(connection, &window).await? else {
        return Ok(Some((window_hit, None)));
    };
    let (px, py, offset, coords) = if x11_session() {
        (x, y, (0.0, 0.0), CoordType::Screen)
    } else {
        let b = &window.bounds;
        (x - b.x, y - b.y, (b.x, b.y), CoordType::Window)
    };
    let Some(object) = deepest(conn, &frame, px.round() as i32, py.round() as i32, coords).await
    else {
        let mut hit = window_hit;
        hit.app = app;
        hit.window = title;
        return Ok(Some((hit, None)));
    };
    Ok(describe(conn, &object, &app, &title, offset, coords)
        .await
        .map(|hit| (hit, Some(object))))
}

/// Without compositor help (GNOME on Wayland, say), trust AT-SPI's screen
/// coordinates and try the focused window first.
async fn hit_by_screen(
    connection: &AccessibilityConnection,
    x: f64,
    y: f64,
) -> Result<Option<(UiHit, Option<ObjectRefOwned>)>, String> {
    let conn = connection.connection();
    let (px, py) = (x.round() as i32, y.round() as i32);
    let mut frames = vec![];
    for app in apps(connection).await? {
        let Ok(proxy) = accessible(conn, &app).await else {
            continue;
        };
        let name = proxy.name().await.unwrap_or_default();
        let Ok(children) = proxy.get_children().await else {
            continue;
        };
        for frame in children {
            let Ok(frame_proxy) = accessible(conn, &frame).await else {
                continue;
            };
            let state = frame_proxy.get_state().await.unwrap_or_default();
            if !state.contains(State::Showing) {
                continue;
            }
            let title = frame_proxy.name().await.unwrap_or_default();
            frames.push((!state.contains(State::Active), name.clone(), frame, title));
        }
    }
    frames.sort_by_key(|f| f.0);
    for (_, app, frame, title) in frames {
        let Ok(proxy) = component(conn, &frame).await else {
            continue;
        };
        if !proxy
            .contains(px, py, CoordType::Screen)
            .await
            .unwrap_or(false)
        {
            continue;
        }
        let object = deepest(conn, &frame, px, py, CoordType::Screen)
            .await
            .unwrap_or(frame);
        return Ok(
            describe(conn, &object, &app, &title, (0.0, 0.0), CoordType::Screen)
                .await
                .map(|hit| (hit, Some(object))),
        );
    }
    Ok(None)
}

pub fn element_at(x: f64, y: f64, ignore: &str) -> Result<Option<UiHit>, String> {
    bounded(async {
        let connection = shared().await?;
        Ok(hit_at(&connection, x, y, ignore).await?.map(|(hit, _)| hit))
    })
}

pub fn pick(x: f64, y: f64, ignore: &str) -> Result<Option<UiPick>, String> {
    bounded(async {
        let connection = shared().await?;
        let Some((hit, object)) = hit_at(&connection, x, y, ignore).await? else {
            return Ok(None);
        };
        let Some(object) = object else {
            return Ok(Some(UiPick { hit, node: None }));
        };
        if hit.role == "password text" {
            return Err("Password fields can't be picked".into());
        }
        let everything = UiSelector {
            scope: UiScope::App,
            app: hit.app.clone(),
            ..UiSelector::default()
        };
        let all = find(&connection, &everything, LIMIT_NODES, true).await?;
        let Some(target) = all.iter().find(|f| key(&f.object) == key(&object)) else {
            return Ok(Some(UiPick { hit, node: None }));
        };
        let (selector, count) = shortest(&all, target);
        let mut node = target.node.clone();
        node.text = selector.to_text();
        node.selector = selector;
        node.matches = count;
        Ok(Some(UiPick {
            hit,
            node: Some(node),
        }))
    })
}

static EVENT_REVISION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static EVENT_ERROR: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(Some(String::new()));
static EVENTS_STARTED: std::sync::Once = std::sync::Once::new();
pub fn revision() -> Result<u64, String> {
    use std::sync::atomic::Ordering;
    EVENTS_STARTED.call_once(|| {
        let result = std::thread::Builder::new()
            .name("atspi-events".into())
            .spawn(|| {
                use futures_lite::StreamExt;
                let _runtime = crate::dbus::enter();
                loop {
                    if super::enabled().is_err() {
                        std::thread::sleep(Duration::from_millis(250));
                        continue;
                    }
                    let connection = bounded(async {
                        use atspi::events::object::{
                            ChildrenChangedEvent, PropertyChangeEvent, StateChangedEvent,
                            TextChangedEvent,
                        };
                        let connection = connect().await?;
                        connection
                            .register_event::<PropertyChangeEvent>()
                            .await
                            .map_err(|e| e.to_string())?;
                        connection
                            .register_event::<StateChangedEvent>()
                            .await
                            .map_err(|e| e.to_string())?;
                        connection
                            .register_event::<ChildrenChangedEvent>()
                            .await
                            .map_err(|e| e.to_string())?;
                        connection
                            .register_event::<TextChangedEvent>()
                            .await
                            .map_err(|e| e.to_string())?;
                        Ok(connection)
                    });
                    match connection {
                        Err(error) => {
                            *EVENT_ERROR.lock().unwrap_or_else(|p| p.into_inner()) = Some(error);
                            std::thread::sleep(Duration::from_secs(1));
                        }
                        Ok(connection) => {
                            *EVENT_ERROR.lock().unwrap_or_else(|p| p.into_inner()) = None;
                            EVENT_REVISION.fetch_add(1, Ordering::Release);
                            let stream = connection.event_stream();
                            futures_lite::pin!(stream);
                            while super::enabled().is_ok() {
                                match futures_lite::future::block_on(tokio::time::timeout(
                                    Duration::from_secs(1),
                                    stream.next(),
                                )) {
                                    Ok(Some(Ok(_))) => {
                                        EVENT_REVISION.fetch_add(1, Ordering::Release);
                                    }
                                    Err(_) => {}
                                    _ => {
                                        *EVENT_ERROR.lock().unwrap_or_else(|p| p.into_inner()) =
                                            Some("AT-SPI event stream disconnected".into());
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
            });
        if let Err(error) = result {
            *EVENT_ERROR.lock().unwrap_or_else(|p| p.into_inner()) =
                Some(format!("Couldn't start AT-SPI events: {error}"));
        }
    });
    if let Some(error) = EVENT_ERROR
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .as_ref()
    {
        return Err(if error.is_empty() {
            "AT-SPI event listener is starting".into()
        } else {
            error.clone()
        });
    }
    Ok(EVENT_REVISION.load(Ordering::Acquire))
}
