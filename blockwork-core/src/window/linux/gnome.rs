//! GNOME on Wayland keeps windows from other apps, so a small Shell
//! extension (packaging/gnome-extension) lists them and acts on them over
//! the session bus. Settings > Integrations installs it.
use super::{Action, ActiveWindow, MatchBy, Rect, ScreenWindow, no_match, number, sized, text};
use serde_json::Value;

pub const UUID: &str = "blockwork-windows@blockworked.com";
const BUS_NAME: &str = "com.blockworked.Windows";
const PATH: &str = "/com/blockworked/Windows";
const METADATA: &str = include_str!(
    "../../../../packaging/gnome-extension/blockwork-windows@blockworked.com/metadata.json"
);
const EXTENSION: &str = include_str!(
    "../../../../packaging/gnome-extension/blockwork-windows@blockworked.com/extension.js"
);

fn missing() -> String {
    "window control on GNOME needs the Blockwork windows extension: install it in Settings > Integrations, then log out and back in".into()
}

fn call<B>(method: &str, body: &B) -> Result<zbus::Message, String>
where
    B: serde::Serialize + zbus::zvariant::DynamicType,
{
    let _runtime = crate::dbus::enter();
    let connection =
        zbus::blocking::Connection::session().map_err(|e| format!("no session bus: {e}"))?;
    connection
        .call_method(Some(BUS_NAME), PATH, Some(BUS_NAME), method, body)
        .map_err(|e| match e {
            zbus::Error::MethodError(name, _, _)
                if name.as_str() == "org.freedesktop.DBus.Error.ServiceUnknown" =>
            {
                missing()
            }
            zbus::Error::MethodError(_, Some(detail), _) => detail,
            other => other.to_string(),
        })
}

/// App windows, front to back, as the extension describes them.
fn windows() -> Result<Vec<Value>, String> {
    let reply = call("List", &())?;
    let json: String = reply
        .body()
        .deserialize()
        .map_err(|e| format!("unexpected window list: {e}"))?;
    parse(&json)
}

fn parse(json: &str) -> Result<Vec<Value>, String> {
    match serde_json::from_str(json) {
        Ok(Value::Array(list)) => Ok(list),
        _ => Err("unexpected window list from the GNOME extension".into()),
    }
}

pub(super) fn info(w: &Value) -> ActiveWindow {
    let class = text(w, "class");
    ActiveWindow {
        title: text(w, "title"),
        app: if class.is_empty() {
            text(w, "app")
        } else {
            class
        },
    }
}

fn flag(w: &Value, key: &str) -> bool {
    w.get(key).and_then(|v| v.as_bool()) == Some(true)
}

pub(super) fn active() -> Result<Option<Value>, String> {
    Ok(windows()?.into_iter().find(|w| flag(w, "focused")))
}

pub(super) fn list_for_agent() -> Result<Vec<crate::window::WindowInfo>, String> {
    Ok(windows()?
        .iter()
        .map(|w| {
            let ActiveWindow { title, app } = info(w);
            crate::window::WindowInfo {
                title,
                app,
                rect: bounds(w).unwrap_or_default(),
                focused: flag(w, "focused"),
            }
        })
        .collect())
}

pub(super) fn find(pattern: &str, by: MatchBy) -> Result<Value, String> {
    let list = windows()?;
    if pattern.trim().is_empty() {
        return list
            .into_iter()
            .find(|w| flag(w, "focused"))
            .ok_or_else(|| "no window has focus".to_string());
    }
    list.into_iter()
        .find(|w| info(w).matches(pattern, by))
        .ok_or_else(|| no_match(pattern))
}

pub(super) fn exists(pattern: &str) -> Result<bool, String> {
    Ok(windows()?
        .iter()
        .any(|w| info(w).matches(pattern, MatchBy::Any)))
}

fn send(w: &Value, action: &str, a: f64, b: f64) -> Result<(), String> {
    let id = w.get("id").and_then(|v| v.as_u64()).unwrap_or_default() as u32;
    call("Act", &(id, action, a, b)).map(drop)
}

pub(super) fn act(action: Action, w: &Value) -> Result<(), String> {
    let name = match action {
        Action::Focus => "Focus",
        Action::Minimize => "Minimize",
        Action::Maximize => "Maximize",
        Action::Restore => "Restore",
        Action::Close => "Close",
        Action::AlwaysOnTop => "AlwaysOnTop",
        Action::NotOnTop => "NotOnTop",
        Action::Hide => "Hide",
        Action::Show => "Show",
    };
    send(w, name, 0.0, 0.0)
}

pub(super) fn set_opacity(w: &Value, opacity: f64) -> Result<(), String> {
    send(w, "Opacity", opacity, 0.0)
}

pub(super) fn place(w: &Value, resize: bool, a: i32, b: i32) -> Result<(), String> {
    send(
        w,
        if resize { "Resize" } else { "Move" },
        a.into(),
        b.into(),
    )
}

pub(super) fn bounds(w: &Value) -> Result<Rect, String> {
    if flag(w, "minimized") {
        return Err("the window is minimized or hidden".into());
    }
    sized(
        number(w, "x"),
        number(w, "y"),
        number(w, "width"),
        number(w, "height"),
    )
}

pub(super) fn stacked() -> Result<Vec<ScreenWindow>, String> {
    Ok(windows()?
        .iter()
        .filter(|w| !flag(w, "minimized"))
        .filter_map(|w| {
            let ActiveWindow { title, app } = info(w);
            Some(ScreenWindow {
                title,
                app,
                pid: w.get("pid").and_then(|p| p.as_u64()).unwrap_or(0) as u32,
                bounds: bounds(w).ok()?,
            })
        })
        .collect())
}

/// Puts the extension in the user's GNOME Shell extensions folder and turns
/// it on. GNOME on Wayland loads new extensions at the next login, so this
/// answers "running", "log_out" or "turn_on" (in the Extensions app) for
/// the settings page to word.
pub fn install() -> Result<String, String> {
    let home = dirs::home_dir().ok_or("no home folder")?;
    let folder = home.join(".local/share/gnome-shell/extensions").join(UUID);
    std::fs::create_dir_all(&folder)
        .map_err(|e| format!("couldn't create {}: {e}", folder.display()))?;
    for (name, body) in [("metadata.json", METADATA), ("extension.js", EXTENSION)] {
        std::fs::write(folder.join(name), body)
            .map_err(|e| format!("couldn't write {name}: {e}"))?;
    }
    if call("List", &()).is_ok() {
        return Ok("running".into());
    }
    let enabled = super::run("gnome-extensions", &["enable", UUID]).is_ok() || enable_in_settings();
    Ok(if enabled { "log_out" } else { "turn_on" }.into())
}

/// Adds the extension to GNOME's enabled list, for when the shell doesn't
/// know it yet and `gnome-extensions enable` refuses.
fn enable_in_settings() -> bool {
    let Ok(current) = super::run(
        "gsettings",
        &["get", "org.gnome.shell", "enabled-extensions"],
    ) else {
        return false;
    };
    let Some(list) = with_enabled(&current) else {
        return true;
    };
    super::run(
        "gsettings",
        &["set", "org.gnome.shell", "enabled-extensions", &list],
    )
    .is_ok()
}

/// `current` (gsettings' `['a', 'b']` or `@as []`) with this extension added;
/// None when it's already there.
fn with_enabled(current: &str) -> Option<String> {
    let inner = current
        .trim()
        .trim_start_matches("@as")
        .trim()
        .trim_start_matches('[')
        .trim_end_matches(']');
    let mut items: Vec<String> = inner
        .split(',')
        .map(|s| s.trim().trim_matches('\'').to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if items.iter().any(|s| s == UUID) {
        return None;
    }
    items.push(UUID.to_string());
    let quoted: Vec<String> = items.iter().map(|s| format!("'{s}'")).collect();
    Some(format!("[{}]", quoted.join(", ")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enabled_list_gains_the_extension_once() {
        assert_eq!(with_enabled("@as []").unwrap(), format!("['{UUID}']"));
        assert_eq!(
            with_enabled("['a@b', 'c@d']").unwrap(),
            format!("['a@b', 'c@d', '{UUID}']")
        );
        assert!(with_enabled(&format!("['{UUID}']")).is_none());
    }

    #[test]
    fn window_list_reads_class_before_app_name() {
        let list = parse(
            r#"[{"id":7,"title":"Inbox","app":"Thunderbird Mail","class":"thunderbird","pid":42,
                 "x":10,"y":20,"width":800,"height":600,"focused":true,"minimized":false},
                {"id":8,"title":"Notes","app":"Text Editor","class":"","minimized":true,
                 "x":0,"y":0,"width":300,"height":200}]"#,
        )
        .unwrap();
        assert_eq!(info(&list[0]).app, "thunderbird");
        assert_eq!(info(&list[1]).app, "Text Editor");
        assert_eq!(bounds(&list[0]).unwrap().width, 800.0);
        assert!(bounds(&list[1]).is_err());
        assert!(parse("{}").is_err());
    }
}
