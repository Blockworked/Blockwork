pub use blockwork_model::input::element::*;
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
pub use reporting::{query, set_query_handler};
