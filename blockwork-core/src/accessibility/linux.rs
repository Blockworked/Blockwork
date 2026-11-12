use super::*;
use crate::input::value::Evaluated;
use atspi::proxy::{
    accessible::AccessibleProxy, action::ActionProxy, component::ComponentProxy,
    editable_text::EditableTextProxy, text::TextProxy, value::ValueProxy,
};
use atspi::{ObjectRefOwned, Role, State};
use atspi_connection::AccessibilityConnection;
use std::collections::{HashSet, VecDeque};
use std::time::Duration;
use zbus::{Connection, proxy::CacheProperties};
fn bounded<T>(future: impl std::future::Future<Output = Result<T, String>>) -> Result<T, String> {
    let _runtime = crate::dbus::enter();
    futures_lite::future::block_on(async {
        tokio::time::timeout(Duration::from_secs(3), future)
            .await
            .map_err(|_| "AT-SPI query timed out after 3 seconds".to_string())?
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
fn contains(value: &str, filter: &str) -> bool {
    filter.is_empty() || value.to_lowercase().contains(&filter.to_lowercase())
}
async fn find(
    connection: &AccessibilityConnection,
    selector: &UiSelector,
    limit: usize,
) -> Result<Vec<(ObjectRefOwned, UiNode)>, String> {
    let root = connection
        .root_accessible_on_registry()
        .await
        .map_err(|e| e.to_string())?;
    let apps = root.get_children().await.map_err(|e| e.to_string())?;
    let focused = if selector.scope == UiScope::Focused {
        Some(crate::window::active()?)
    } else {
        None
    };
    let window_filter = focused
        .as_ref()
        .map_or(selector.window.as_str(), |w| w.title.as_str());
    if selector.scope == UiScope::Focused && window_filter.is_empty() {
        return Err(
            "The focused window has no accessible title; choose an app or window explicitly".into(),
        );
    }
    let mut queue = VecDeque::new();
    for app in apps {
        let proxy = match accessible(connection.connection(), &app).await {
            Ok(p) => p,
            Err(_) => continue,
        };
        let name = proxy.name().await.unwrap_or_default();
        if contains(&name, &selector.app) {
            queue.push_back((app, name, String::new(), 0usize));
        }
    }
    let mut seen = HashSet::new();
    let mut result = vec![];
    while let Some((object, app, mut window, depth)) = queue.pop_front() {
        let key = format!(
            "{}{}",
            object.name_as_str().unwrap_or_default(),
            object.path_as_str()
        );
        if !seen.insert(key) {
            continue;
        }
        if seen.len() > 4000 {
            return Err(
                "AT-SPI search exceeded 4,000 nodes; narrow the app or window filter".into(),
            );
        }
        if depth > 64 {
            return Err("AT-SPI search exceeded 64 levels; narrow the selector".into());
        }
        let proxy = match accessible(connection.connection(), &object).await {
            Ok(p) => p,
            Err(_) => continue,
        };
        let role = match proxy.get_role().await {
            Ok(r) => r,
            Err(_) => continue,
        };
        if role == Role::PasswordText {
            continue;
        }
        let name = proxy.name().await.unwrap_or_default();
        if matches!(
            role,
            Role::Frame | Role::Dialog | Role::Window | Role::Alert
        ) {
            window = name.clone();
        }
        let role_name = proxy.get_role_name().await.unwrap_or_default();
        let id = if selector.id.is_empty() {
            String::new()
        } else {
            proxy.accessible_id().await.unwrap_or_default()
        };
        if contains(&window, window_filter)
            && contains(&name, &selector.name)
            && (selector.role.is_empty() || role_name.eq_ignore_ascii_case(&selector.role))
            && (selector.id.is_empty() || id == selector.id)
        {
            let id = if id.is_empty() {
                proxy.accessible_id().await.unwrap_or_default()
            } else {
                id
            };
            let enabled = proxy
                .get_state()
                .await
                .map(|s| s.contains(State::Enabled) && s.contains(State::Sensitive))
                .unwrap_or(false);
            let saved = UiSelector {
                scope: UiScope::App,
                app: app.clone(),
                window: window.clone(),
                name: name.clone(),
                role: role_name.clone(),
                id: id.clone(),
            };
            result.push((
                object.clone(),
                UiNode {
                    app: app.clone(),
                    window: window.clone(),
                    name,
                    role: role_name,
                    id,
                    enabled,
                    selector: saved,
                },
            ));
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
fn unique(
    mut nodes: Vec<(ObjectRefOwned, UiNode)>,
) -> Result<Option<(ObjectRefOwned, UiNode)>, String> {
    if nodes.len() > 1 {
        return Err("More than one UI element matches; narrow its name, role, ID or window".into());
    }
    Ok(nodes.pop())
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
        let connection = connect().await?;
        let root = connection
            .root_accessible_on_registry()
            .await
            .map_err(|e| e.to_string())?;
        let mut apps = vec![];
        for object in root.get_children().await.map_err(|e| e.to_string())? {
            if let Ok(proxy) = accessible(connection.connection(), &object).await
                && let Ok(name) = proxy.name().await
                && !name.is_empty()
            {
                apps.push(name);
            }
        }
        apps.sort();
        apps.dedup();
        Ok(apps)
    })
}
pub fn inspect(selector: &UiSelector) -> Result<Vec<UiNode>, String> {
    bounded(async {
        let connection = connect().await?;
        Ok(find(&connection, selector, 100)
            .await?
            .into_iter()
            .map(|(_, node)| node)
            .collect())
    })
}
pub fn query(selector: &UiSelector, query: Query) -> Result<Evaluated, String> {
    bounded(async {
        let connection = connect().await?;
        let target = unique(find(&connection, selector, 2).await?)?;
        if matches!(query, Query::Exists) {
            return Ok(Evaluated::Bool(target.is_some()));
        }
        let Some((object, node)) = target else {
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
        let connection = connect().await?;
        let (object, node) = unique(find(&connection, selector, 2).await?)?
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
