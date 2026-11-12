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
    }
}
