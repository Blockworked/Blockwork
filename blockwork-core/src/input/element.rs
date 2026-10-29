//! Stable selectors, never retained accessibility node handles.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ElementSelector {
    pub package: String,
    #[serde(default)]
    pub resource_id: String,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub class: String,
}
impl ElementSelector {
    pub fn validate(&self) -> Result<(), String> {
        if self.package.trim().is_empty() {
            return Err("Choose the target app package".into());
        }
        if [
            &self.resource_id,
            &self.text,
            &self.description,
            &self.class,
        ]
        .iter()
        .all(|s| s.is_empty())
        {
            return Err("Supply a resource ID, text, description or class for the element".into());
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ElementAction {
    Click,
    LongClick,
    ScrollForward,
    ScrollBackward,
    SetText,
    Exists,
    ReadText,
    Wait,
}

#[cfg(feature = "system")]
mod reporting {
    use super::*;
    use std::sync::{Arc, RwLock};
    pub type Handler =
        dyn Fn(&ElementSelector, ElementAction) -> Result<serde_json::Value, String> + Send + Sync;
    static HANDLER: RwLock<Option<Arc<Handler>>> = RwLock::new(None);
    pub fn set_query_handler(handler: Arc<Handler>) {
        *HANDLER.write().unwrap_or_else(|e| e.into_inner()) = Some(handler);
    }
    pub fn query(
        selector: &ElementSelector,
        action: ElementAction,
    ) -> Result<serde_json::Value, String> {
        selector.validate()?;
        if !matches!(action, ElementAction::Exists | ElementAction::ReadText) {
            return Err("reporters only read elements".into());
        }
        let handler = HANDLER
            .read()
            .map_err(|e| e.to_string())?
            .clone()
            .ok_or("accessibility elements are unavailable on this platform")?;
        handler(selector, action)
    }
}
#[cfg(feature = "system")]
pub use reporting::{query, set_query_handler};
