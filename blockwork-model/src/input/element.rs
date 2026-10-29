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
