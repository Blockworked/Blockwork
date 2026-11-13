//! Persisted accessibility selectors contain descriptions, never live handles.
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum UiScope {
    #[default]
    Focused,
    App,
    Window,
}
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct UiSelector {
    pub scope: UiScope,
    pub app: String,
    pub window: String,
    pub name: String,
    pub role: String,
    pub id: String,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum UiAction {
    Invoke,
    SetText,
    SetValue,
    Focus,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum UiProperty {
    Exists,
    Enabled,
    Text,
    Value,
}
impl UiSelector {
    pub fn parse(text: &str) -> Result<Self, String> {
        if text.len() > 8192 {
            return Err("UI selector exceeds 8 KiB".into());
        }
        let text = text.trim();
        let mut result = if text.starts_with('{') {
            serde_json::from_str(text).map_err(|e| format!("Invalid UI selector: {e}"))?
        } else {
            let mut result = Self::default();
            let mut explicit = false;
            let mut seen = std::collections::HashSet::new();
            for part in text.split(';').filter(|p| !p.trim().is_empty()) {
                let (key, value) = part
                    .split_once('=')
                    .ok_or("Use name=value filters separated by semicolons, or a JSON selector")?;
                let (key, value) = (key.trim(), value.trim());
                if !seen.insert(key) {
                    return Err(format!("UI selector repeats {key}"));
                }
                match key {
                    "scope" => {
                        result.scope = match value {
                            "Focused" => UiScope::Focused,
                            "App" => UiScope::App,
                            "Window" => UiScope::Window,
                            _ => return Err("UI scope must be Focused, App or Window".into()),
                        };
                        explicit = true;
                    }
                    "app" => result.app = value.into(),
                    "window" => result.window = value.into(),
                    "name" => result.name = value.into(),
                    "role" => result.role = value.into(),
                    "id" => result.id = value.into(),
                    _ => return Err(format!("Unknown UI filter {key}")),
                }
            }
            if !explicit {
                if !result.window.is_empty() {
                    result.scope = UiScope::Window;
                } else if !result.app.is_empty() {
                    result.scope = UiScope::App;
                }
            }
            result
        };
        result.app = result.app.trim().into();
        result.window = result.window.trim().into();
        result.name = result.name.trim().into();
        result.role = result.role.trim().into();
        result.id = result.id.trim().into();
        if (result.scope == UiScope::App && result.app.is_empty())
            || (result.scope == UiScope::Window && result.window.is_empty())
        {
            return Err("App and Window scopes require their matching filter".into());
        }
        Ok(result)
    }
    /// The readable `key=value; ...` form, or JSON when a value holds a
    /// semicolon. Scope is left to what the filters imply, so it must be App
    /// or Window here (a Focused selector is written with `scope=Focused`).
    pub fn to_text(&self) -> String {
        let fields = [
            ("app", &self.app),
            ("window", &self.window),
            ("name", &self.name),
            ("role", &self.role),
            ("id", &self.id),
        ];
        let plain = fields
            .iter()
            .all(|(_, v)| !v.contains(';') && v.trim() == v.as_str());
        if !plain {
            return serde_json::to_string(self).unwrap_or_default();
        }
        let mut parts = vec![];
        let implied = if !self.window.is_empty() {
            UiScope::Window
        } else if !self.app.is_empty() {
            UiScope::App
        } else {
            UiScope::Focused
        };
        if self.scope != implied && !(self.scope == UiScope::App && implied == UiScope::Window) {
            parts.push(format!("scope={:?}", self.scope));
        }
        for (key, value) in fields {
            if !value.is_empty() {
                parts.push(format!("{key}={value}"));
            }
        }
        parts.join("; ")
    }
    pub fn require_target(&self) -> Result<(), String> {
        if self.name.is_empty() && self.role.is_empty() && self.id.is_empty() {
            Err("Choose an element by name, role or accessible ID".into())
        } else {
            Ok(())
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selectors_are_portable_and_reject_unknown_or_duplicate_filters() {
        let selector = UiSelector::parse("app=Calculator; name=Plus; role=push button").unwrap();
        assert_eq!(selector.scope, UiScope::App);
        selector.require_target().unwrap();
        assert_eq!(
            UiSelector::parse(&serde_json::to_string(&selector).unwrap()).unwrap(),
            selector
        );
        assert!(UiSelector::parse("name=A;name=B").is_err());
        assert!(UiSelector::parse("scope=Window;name=Save").is_err());
        assert!(UiSelector::parse("command=anything").is_err());
        assert!(
            UiSelector::parse("scope=Focused")
                .unwrap()
                .require_target()
                .is_err()
        );
        assert!(
            UiSelector::parse(r#"{"name":"Save; as","role":"push button","extra":1}"#).is_err()
        );
        for text in [
            "app=Calculator; name=Plus; role=push button",
            "window=Preferences; name=Dark mode",
            "scope=Focused; name=Save",
            "scope=App; app=Editor; window=Doc; id=save",
            r#"{"scope":"App","app":"A","window":"","name":"Save; as","role":"","id":""}"#,
        ] {
            let selector = UiSelector::parse(text).unwrap();
            let again = UiSelector::parse(&selector.to_text()).unwrap();
            assert_eq!(selector.app, again.app);
            assert_eq!(selector.name, again.name);
            assert_eq!(selector.window, again.window);
        }
        assert_eq!(
            UiSelector::parse("scope=App; app=Editor; name=Save")
                .unwrap()
                .to_text(),
            "app=Editor; name=Save"
        );
    }
}
