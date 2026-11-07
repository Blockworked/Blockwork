//! The canvas commands: the shared ones in `blockwork_core::editing`, run
//! against the app's state, plus the preview that reads live values.

use super::*;
use blockwork_core::editing::{self, Document, Parts};

/// The app's state as an editing document: saves on every change and keeps
/// the live variable and list stores in step.
pub(crate) struct AppDoc<'a>(pub(crate) &'a mut AppState);

impl Document for AppDoc<'_> {
    fn parts(&mut self) -> Parts<'_> {
        Parts {
            open: self.0.current_macro.as_mut(),
            history: &mut self.0.history,
            buffers: &mut self.0.invalid_field_buffers,
        }
    }

    fn variable_values(&self) -> HashMap<String, Evaluated> {
        self.0
            .variable_values
            .lock()
            .map(|g| g.clone())
            .unwrap_or_default()
    }

    fn variable_set(&mut self, name: &str, value: Option<Evaluated>) {
        if let Ok(mut store) = self.0.variable_values.lock() {
            match value {
                Some(value) => store.insert(name.to_owned(), value),
                None => store.remove(name),
            };
        }
    }

    fn variable_renamed(&mut self, old: &str, new: &str) {
        if let Ok(mut store) = self.0.variable_values.lock()
            && let Some(value) = store.remove(old)
        {
            store.insert(new.to_owned(), value);
        }
    }

    fn lists_changed(&mut self) {
        sync_list_values(self.0);
    }

    fn restored(&mut self) {
        sync_variable_values(self.0);
        sync_list_values(self.0);
    }

    fn changed(&mut self) {
        auto_save(self.0);
    }
}

/// Runs a canvas command, or `None` when `cmd` isn't one. A block that moves
/// the mouse to a screen position asks for that permission first, unlocked.
pub(crate) async fn edit_command(
    state: &SharedState,
    app: &AppHandle,
    cmd: &str,
    args: &serde_json::Value,
) -> Option<Result<serde_json::Value, String>> {
    if !editing::handles(cmd, args) {
        return None;
    }
    if let Some(ins) = editing::instruction_written(cmd, args)
        && matches!(
            ins.kind,
            InstructionKind::Token(InputToken::MoveMouse(
                _,
                _,
                blockwork_core::input::types::Coordinate::Abs
            ))
        )
        && let Err(e) = request_absolute_mouse_support(state, app).await
    {
        return Some(Err(e));
    }
    let mut s = match state.lock() {
        Ok(s) => s,
        Err(e) => return Some(Err(e.to_string())),
    };
    let result = editing::dispatch(&mut AppDoc(&mut s), cmd, args)?;
    if !editing::READS.contains(&cmd) {
        emit_state_updated(app, &s);
    }
    Some(result)
}

/// One-shot sample evaluation of a value tree, for the click-to-preview
/// tooltip on operator blocks - stateless. Uses `eval_text` so text-only
/// ops (`Join`/`NewLine`/`Tab`) preview without erroring as "not a number".
pub(crate) fn preview_value(state: &SharedState, value: Value) -> Result<String, String> {
    // A click shouldn't run a command or hit the network.
    if blockwork_core::input::value::has_side_effect_operator(&value) {
        return Err("Preview is off for blocks that run commands or go online".into());
    }
    let (env, lists) = {
        let s = state.lock().map_err(|e| e.to_string())?;
        let env: HashMap<String, Evaluated> = s
            .variable_values
            .lock()
            .map(|g| g.clone())
            .unwrap_or_default();
        let lists = s.list_values.lock().map(|g| g.clone()).unwrap_or_default();
        (env, lists)
    };
    // Evaluated unlocked: a file or window read may be slow.
    preview_value_with_env_and_lists(&value, &env, &lists)
        .map(|text| blockwork_core::secrets::redact(&text))
}

/// The actual evaluation logic behind `preview_value`, factored out so it's
/// testable without backend state locking.
#[cfg(test)]
pub(super) fn preview_value_with_env(
    value: &Value,
    env: &HashMap<String, Evaluated>,
) -> Result<String, String> {
    preview_value_with_env_and_lists(value, env, &HashMap::new())
}

pub(super) fn preview_value_with_env_and_lists(
    value: &Value,
    env: &HashMap<String, Evaluated>,
    lists: &HashMap<String, Vec<ListItem>>,
) -> Result<String, String> {
    resolve_list_reporters(&value.resolve_vars(env), lists)?.eval_text()
}

/// A canvas command that needs no permission, from code that can't wait.
fn edit_now(state: &SharedState, app: &AppHandle, cmd: &str) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    let result = editing::dispatch(&mut AppDoc(&mut s), cmd, &serde_json::Value::Null);
    emit_state_updated(app, &s);
    result.unwrap_or_else(|| Err(format!("unknown command `{cmd}`")))?;
    Ok(())
}

pub(crate) fn undo(state: &SharedState, app: &AppHandle) -> Result<(), String> {
    edit_now(state, app, "undo")
}

pub(crate) fn redo(state: &SharedState, app: &AppHandle) -> Result<(), String> {
    edit_now(state, app, "redo")
}
