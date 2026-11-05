//! A macro's variables, signals, globals and lists.

use super::*;

/// Declares a new macro-wide variable, starting at `0`. No `push_undo` -
/// a naming/creation action, not an undoable structural edit.
pub(crate) fn create_variable(
    state: &SharedState,
    app: &AppHandle,
    name: String,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    let mac = s.current_macro.as_mut().ok_or("No macro selected")?;
    let trimmed = mac.create_variable(&name)?;
    if let Ok(mut store) = s.variable_values.lock() {
        store.insert(trimmed, Evaluated::Number(0.0));
    }
    auto_save(&mut s);
    emit_state_updated(&app, &s);
    Ok(())
}

/// Renames a declared variable and every reference to it.
pub(crate) fn rename_variable(
    state: &SharedState,
    app: &AppHandle,
    old_name: String,
    new_name: String,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    // Rename on a copy first, so a rejected or no-op rename doesn't add a
    // useless undo entry.
    let mut graph = s
        .current_macro
        .as_ref()
        .map(|mac| mac.graph.clone())
        .ok_or("No macro selected")?;
    let trimmed = graph.rename_variable(&old_name, &new_name)?;
    if trimmed == old_name {
        return Ok(());
    }
    push_undo(&mut s);
    if let Some(mac) = &mut s.current_macro {
        mac.graph = graph;
    }
    if let Ok(mut store) = s.variable_values.lock() {
        if let Some(v) = store.remove(&old_name) {
            store.insert(trimmed, v);
        }
    }
    auto_save(&mut s);
    emit_state_updated(&app, &s);
    Ok(())
}

/// Sets a declared variable's stored value, the one the next run starts
/// from. Text that reads as a number is stored as one.
pub(crate) fn set_variable_value(
    state: &SharedState,
    app: &AppHandle,
    name: String,
    value: String,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    let mac = s.current_macro.as_mut().ok_or("No macro selected")?;
    let value = blockwork_core::macros::typed_value(&value);
    mac.set_variable_value(&name, value.clone())?;
    if let Ok(mut store) = s.variable_values.lock() {
        store.insert(name, value);
    }
    auto_save(&mut s);
    emit_state_updated(&app, &s);
    Ok(())
}

/// Deletes a declared variable. No `push_undo` - same precedent as
/// `create_variable`.
pub(crate) fn delete_variable(
    state: &SharedState,
    app: &AppHandle,
    name: String,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    let mac = s.current_macro.as_mut().ok_or("No macro selected")?;
    mac.remove_variable(&name);
    if let Ok(mut store) = s.variable_values.lock() {
        store.remove(&name);
    }
    auto_save(&mut s);
    emit_state_updated(&app, &s);
    Ok(())
}

pub(crate) fn create_signal(
    state: &SharedState,
    app: &AppHandle,
    name: String,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    let name = s
        .current_macro
        .as_ref()
        .ok_or("No macro selected")?
        .validate_signal_name(&name, None)?;
    push_undo(&mut s);
    s.current_macro.as_mut().unwrap().signals.push(name);
    auto_save(&mut s);
    emit_state_updated(app, &s);
    Ok(())
}

pub(crate) fn rename_signal(
    state: &SharedState,
    app: &AppHandle,
    old_name: String,
    new_name: String,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    let mac = s.current_macro.as_ref().ok_or("No macro selected")?;
    let name = mac.validate_signal_name(&new_name, Some(&old_name))?;
    if !mac.signals.contains(&old_name) {
        return Err("Signal not found".into());
    }
    if name == old_name {
        return Ok(());
    }
    push_undo(&mut s);
    s.current_macro
        .as_mut()
        .unwrap()
        .rename_signal(&old_name, &name)?;
    auto_save(&mut s);
    emit_state_updated(app, &s);
    Ok(())
}

pub(crate) fn delete_signal(
    state: &SharedState,
    app: &AppHandle,
    name: String,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    let mac = s.current_macro.as_ref().ok_or("No macro selected")?;
    if !mac.signals.contains(&name) {
        return Err("Signal not found".into());
    }
    let mut used = false;
    mac.graph.walk_instructions(&mut |ins| {
        if let InstructionKind::WhenSignal(target)
        | InstructionKind::SendSignal { name: target, .. } = &ins.kind
            && !target.expression
            && let Value::Text { value } = &target.value
        {
            used |= value.trim().eq_ignore_ascii_case(&name);
        }
    });
    if used {
        return Err("This signal is still used by a named signal block. Change or remove those blocks first.".into());
    }
    push_undo(&mut s);
    s.current_macro
        .as_mut()
        .unwrap()
        .signals
        .retain(|s| s != &name);
    auto_save(&mut s);
    emit_state_updated(app, &s);
    Ok(())
}
pub(super) fn create_list_in(mac: &mut Macro, name: &str) -> Result<String, String> {
    let trimmed = mac.graph.create_list(name)?;
    if let Some(list) = mac.graph.lists.iter_mut().find(|list| list.name == trimmed) {
        list.editor_x = 36;
        list.editor_y = 36;
    }
    Ok(trimmed)
}

pub(crate) fn create_list(
    state: &SharedState,
    app: &AppHandle,
    name: String,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    // Validate before checkpointing so a rejected name never creates a dead
    // undo entry.
    {
        let mac = s.current_macro.as_ref().ok_or("No macro selected")?;
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err("List name can't be empty".to_string());
        }
        if mac.lists.iter().any(|list| list.name == trimmed) {
            return Err(format!("A list named \"{trimmed}\" already exists"));
        }
    }
    push_undo(&mut s);
    create_list_in(s.current_macro.as_mut().ok_or("No macro selected")?, &name)?;
    sync_list_values(&mut s);
    auto_save(&mut s);
    emit_state_updated(&app, &s);
    Ok(())
}

pub(crate) fn rename_list(
    state: &SharedState,
    app: &AppHandle,
    old_name: String,
    new_name: String,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    let trimmed = new_name.trim().to_string();
    {
        let mac = s.current_macro.as_ref().ok_or("No macro selected")?;
        if trimmed.is_empty() {
            return Err("List name can't be empty".to_string());
        }
        if trimmed != old_name && mac.lists.iter().any(|list| list.name == trimmed) {
            return Err(format!("A list named \"{trimmed}\" already exists"));
        }
        if !mac.lists.iter().any(|list| list.name == old_name) {
            return Err("List not found".to_string());
        }
    }
    if trimmed == old_name {
        return Ok(());
    }

    push_undo(&mut s);
    s.current_macro
        .as_mut()
        .ok_or("No macro selected")?
        .graph
        .rename_list(&old_name, &trimmed)?;
    sync_list_values(&mut s);
    auto_save(&mut s);
    emit_state_updated(&app, &s);
    Ok(())
}

pub(crate) fn delete_list(
    state: &SharedState,
    app: &AppHandle,
    name: String,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    let exists = s
        .current_macro
        .as_ref()
        .ok_or("No macro selected")?
        .lists
        .iter()
        .any(|list| list.name == name);
    if exists {
        push_undo(&mut s);
        s.current_macro
            .as_mut()
            .expect("checked above")
            .graph
            .remove_list(&name);
    }
    sync_list_values(&mut s);
    auto_save(&mut s);
    emit_state_updated(&app, &s);
    Ok(())
}

/// Replaces a list's visible-editor contents. `ListItemDto` intentionally has
/// no boolean/expression case, which enforces the literal-only list contract.
pub(crate) fn set_list_items(
    state: &SharedState,
    app: &AppHandle,
    name: String,
    items: Vec<ListItemDto>,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    let items: Vec<ListItem> = items.iter().map(crate::state::dto_to_list_item).collect();
    s.current_macro
        .as_mut()
        .ok_or("No macro selected")?
        .graph
        .set_list_items(&name, items)?;
    sync_list_values(&mut s);
    auto_save(&mut s);
    emit_state_updated(&app, &s);
    Ok(())
}

/// Saves whether a list's editable canvas monitor is open and where it sits.
/// This is a presentation preference rather than an undoable macro edit.
pub(crate) fn set_list_editor_state(
    state: &SharedState,
    app: &AppHandle,
    name: String,
    visible: bool,
    x: i32,
    y: i32,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.current_macro
        .as_mut()
        .ok_or("No macro selected")?
        .graph
        .set_list_editor_state(&name, visible, x, y)?;
    auto_save(&mut s);
    emit_state_updated(&app, &s);
    Ok(())
}
