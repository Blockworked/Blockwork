//! Edits inside strands: instructions, values, floating values, comments and undo.

use super::*;

// Declare a palette default only after its block was successfully inserted.
pub(super) fn ensure_inserted_variable(s: &mut AppState, name: Option<String>) {
    let Some(name) = name else {
        return;
    };
    let Some(mac) = s.current_macro.as_mut() else {
        return;
    };
    let Ok(name) = mac.create_variable(&name) else {
        return;
    };
    if let Ok(mut store) = s.variable_values.lock() {
        store.insert(name, Evaluated::Number(0.0));
    }
}

pub(crate) async fn add_instruction(
    state: &SharedState,
    app: &AppHandle,
    strand_id: String,
    path: Vec<PathStep>,
    instruction: InstructionDto,
    target_y: Option<i32>,
    variable_to_create: Option<String>,
) -> Result<(), String> {
    let ins = dto_to_instruction(&instruction).ok_or("Unknown instruction type")?;
    if matches!(
        &ins.kind,
        InstructionKind::Token(InputToken::MoveMouse(
            _,
            _,
            blockwork_core::input::types::Coordinate::Abs
        ))
    ) {
        request_absolute_mouse_support(state, app).await?;
    }
    let mut s = state.lock().map_err(|e| e.to_string())?;
    // Checked before checkpointing, so a rejected drop costs no undo step.
    if let Some(mac) = &s.current_macro {
        mac.check_insert(&strand_id, &path, &ins)?;
    }
    push_undo(&mut s);
    if let Some(mac) = &mut s.current_macro
        && mac.insert_instruction(&strand_id, &path, ins)?
    {
        if let Some(y) = target_y
            && path.len() == 1
            && path[0].index == 0
            && let Some(strand) = mac.strand_mut(&strand_id)
        {
            strand.y = y;
        }
        ensure_inserted_variable(&mut s, variable_to_create);
        s.invalid_field_buffers.clear();
        auto_save(&mut s);
    }
    emit_state_updated(&app, &s);
    Ok(())
}

pub(crate) async fn edit_instruction(
    state: &SharedState,
    app: &AppHandle,
    strand_id: String,
    path: Vec<PathStep>,
    instruction: InstructionDto,
) -> Result<(), String> {
    let ins = dto_to_instruction(&instruction).ok_or("Unknown instruction type")?;
    if matches!(
        &ins.kind,
        InstructionKind::Token(InputToken::MoveMouse(
            _,
            _,
            blockwork_core::input::types::Coordinate::Abs
        ))
    ) {
        request_absolute_mouse_support(state, app).await?;
    }
    let mut s = state.lock().map_err(|e| e.to_string())?;
    // Free text coalesces keystrokes into one undo step, like
    // `edit_value_field`; every other kind gets its own undo step.
    let session = ins.kind.edits_as_text().then(|| EditSession::Instruction {
        strand_id: strand_id.clone(),
        index: path.clone(),
    });
    push_undo_for(&mut s, session);
    if let Some(mac) = &mut s.current_macro
        && mac.replace_instruction(&strand_id, &path, ins)
    {
        auto_save(&mut s);
    }
    emit_state_updated(&app, &s);
    Ok(())
}

pub(crate) fn edit_value_field(
    state: &SharedState,
    app: &AppHandle,
    location: ValueLocation,
    text: String,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    // A run of keystrokes into one field coalesces into one undo step; a
    // location that no longer resolves is quietly ignored, as before.
    let _ = run_edit(&mut s, EditCommand::EditFieldText { location, text });
    emit_state_updated(&app, &s);
    Ok(())
}

pub(crate) fn set_value_kind(
    state: &SharedState,
    app: &AppHandle,
    location: ValueLocation,
    kind: String,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    push_undo(&mut s);
    let env: HashMap<String, Evaluated> = s
        .variable_values
        .lock()
        .map(|g| g.clone())
        .unwrap_or_default();
    let result = match &mut s.current_macro {
        Some(mac) => mac.set_value_kind(&location, &kind, &env),
        None => Ok(()),
    };
    prune_value_buffers(&mut s.invalid_field_buffers, &location);
    auto_save(&mut s);
    emit_state_updated(&app, &s);
    result
}

/// Removes the value at `location` and returns it, leaving a `Field`
/// location holding whatever it was shadowing (or its typed blank value); a
/// root `Floating` location is deleted entirely. Pairs with `put_value`/
/// `create_floating_value` on the frontend side of a drag.
pub(crate) fn take_value(
    state: &SharedState,
    app: &AppHandle,
    location: ValueLocation,
) -> Result<Value, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    push_undo(&mut s);
    let taken = s
        .current_macro
        .as_mut()
        .and_then(|mac| mac.take_value(&location));
    prune_value_buffers(&mut s.invalid_field_buffers, &location);
    auto_save(&mut s);
    emit_state_updated(&app, &s);
    taken.ok_or_else(|| "Nothing to take at that location".to_string())
}

/// Overwrites the node at `location` with `value` - the "put" half of
/// moving a block into a field/subfield slot. An incoming operator's
/// shadowed value is overwritten with the destination's prior content.
pub(crate) fn put_value(
    state: &SharedState,
    app: &AppHandle,
    location: ValueLocation,
    value: Value,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    push_undo(&mut s);
    if let Some(mac) = &mut s.current_macro {
        mac.put_value(&location, value);
    }
    prune_value_buffers(&mut s.invalid_field_buffers, &location);
    auto_save(&mut s);
    emit_state_updated(&app, &s);
    Ok(())
}

/// Moves a value block in one edit: into the slot `to`, or parked on open
/// canvas at `x`/`y` when `to` is none. One undo step and one state update.
pub(crate) fn move_value(
    state: &SharedState,
    app: &AppHandle,
    from: ValueLocation,
    to: Option<ValueLocation>,
    x: i32,
    y: i32,
    origin_block_id: Option<String>,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    let to = match to {
        Some(to) => ValueDestination::Slot(to),
        None => ValueDestination::Canvas {
            x,
            y,
            origin_block_id,
        },
    };
    let result = run_edit(&mut s, EditCommand::MoveValue { from, to });
    emit_state_updated(&app, &s);
    result.map(|_| ()).map_err(String::from)
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

/// Creates a new value block parked on open canvas - for a sidebar drop, or
/// the "create" half of dragging an existing block out onto canvas.
/// `origin_block_id` is set only when `value` is a `Param` reporter dragged
/// straight out of its declaring block's header (see blockstitchSetup.ts's
/// `createFloatingValue`) - lets a floating param render with its real
/// declared shape instead of a guess.
pub(crate) fn create_floating_value(
    state: &SharedState,
    app: &AppHandle,
    x: i32,
    y: i32,
    value: Value,
    origin_block_id: Option<String>,
) -> Result<String, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    push_undo(&mut s);
    let id = s
        .current_macro
        .as_mut()
        .ok_or("No macro selected")?
        .add_floating_value(x, y, value, origin_block_id);
    auto_save(&mut s);
    emit_state_updated(&app, &s);
    Ok(id)
}

/// Repositions a floating value block dropped on open canvas. No
/// `push_undo` - pure repositioning, same as `move_strand`.
pub(crate) fn move_floating_value(
    state: &SharedState,
    app: &AppHandle,
    floating_id: String,
    x: i32,
    y: i32,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if let Some(mac) = &mut s.current_macro
        && mac.move_floating_value(&floating_id, x, y)
    {
        auto_save(&mut s);
    }
    emit_state_updated(&app, &s);
    Ok(())
}

/// Deletes a floating value block outright (dropped on the sidebar trash) -
/// mirrors `remove_strand`.
pub(crate) fn remove_floating_value(
    state: &SharedState,
    app: &AppHandle,
    floating_id: String,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    push_undo(&mut s);
    if let Some(mac) = &mut s.current_macro {
        mac.remove_floating_value(&floating_id);
        auto_save(&mut s);
    }
    emit_state_updated(&app, &s);
    Ok(())
}

/// Creates a freestanding note parked on open canvas - the "Add Comment"
/// canvas-context-menu item. Mirrors `create_floating_value`.
pub(crate) fn create_comment(
    state: &SharedState,
    app: &AppHandle,
    x: i32,
    y: i32,
    text: String,
) -> Result<String, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    push_undo(&mut s);
    let id = s
        .current_macro
        .as_mut()
        .ok_or("No macro selected")?
        .add_comment(x, y, text, None);
    auto_save(&mut s);
    emit_state_updated(&app, &s);
    Ok(id)
}

/// Creates a note pinned to `instruction_id` - the "Add Comment" block/header
/// context-menu item. `dx`/`dy` are an offset from that instruction's
/// on-screen position, not an absolute canvas coordinate (see `Comment`'s
/// doc comment); the frontend picks a default that clears the block.
pub(crate) fn create_attached_comment(
    state: &SharedState,
    app: &AppHandle,
    instruction_id: String,
    dx: i32,
    dy: i32,
    text: String,
) -> Result<String, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    push_undo(&mut s);
    let id = s
        .current_macro
        .as_mut()
        .ok_or("No macro selected")?
        .add_comment(dx, dy, text, Some(instruction_id));
    auto_save(&mut s);
    emit_state_updated(&app, &s);
    Ok(id)
}

/// Repositions a note - `x`/`y` are the same value the frontend already
/// tracks for it (an absolute canvas position if freestanding, an offset
/// from its attached instruction if not), plus the pointer's drag delta; see
/// `Comment`'s doc comment. No `push_undo` - pure repositioning, same as
/// `move_floating_value`.
pub(crate) fn move_comment(
    state: &SharedState,
    app: &AppHandle,
    comment_id: String,
    x: i32,
    y: i32,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if let Some(mac) = &mut s.current_macro
        && mac.move_comment(&comment_id, x, y)
    {
        auto_save(&mut s);
    }
    emit_state_updated(&app, &s);
    Ok(())
}

/// Deletes a note outright (its own "×" button) - mirrors `remove_floating_value`.
pub(crate) fn remove_comment(
    state: &SharedState,
    app: &AppHandle,
    comment_id: String,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    push_undo(&mut s);
    if let Some(mac) = &mut s.current_macro {
        mac.remove_comment(&comment_id);
        auto_save(&mut s);
    }
    emit_state_updated(&app, &s);
    Ok(())
}

/// Edits a note's text - coalesces keystrokes into one undo group, same as
/// `edit_instruction`'s freeform-text instructions.
pub(crate) fn edit_comment_text(
    state: &SharedState,
    app: &AppHandle,
    comment_id: String,
    text: String,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    push_undo_for(
        &mut s,
        Some(EditSession::Comment {
            comment_id: comment_id.clone(),
        }),
    );
    if let Some(mac) = &mut s.current_macro
        && mac.set_comment_text(&comment_id, text)
    {
        auto_save(&mut s);
    }
    emit_state_updated(&app, &s);
    Ok(())
}

/// Toggles a note's collapsed state - view state, not content, so no
/// `push_undo` (same reasoning as `move_comment`).
pub(crate) fn set_comment_collapsed(
    state: &SharedState,
    app: &AppHandle,
    comment_id: String,
    collapsed: bool,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if let Some(mac) = &mut s.current_macro
        && mac.set_comment_collapsed(&comment_id, collapsed)
    {
        auto_save(&mut s);
    }
    emit_state_updated(&app, &s);
    Ok(())
}

pub(crate) fn remove_instruction(
    state: &SharedState,
    app: &AppHandle,
    strand_id: String,
    path: Vec<PathStep>,
    count: usize,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    // `count` removes a whole tail as one edit. A path addressing nothing
    // costs no undo step and isn't an error.
    let _ = run_edit(
        &mut s,
        EditCommand::RemoveInstructions {
            strand_id,
            path,
            count,
        },
    );
    emit_state_updated(&app, &s);
    Ok(())
}

/// Deletes the instruction at `path`, splitting anything below it (in the
/// same body list) off into a new top-level strand at `(x, y)` - atomic
/// with the removal, so it's one undo step. Returns the new strand's id, or
/// `None` if nothing was split off.
pub(crate) fn delete_instruction(
    state: &SharedState,
    app: &AppHandle,
    strand_id: String,
    path: Vec<PathStep>,
    x: i32,
    y: i32,
) -> Result<Option<String>, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    let mut graph = match &s.current_macro {
        Some(mac) => mac.graph.clone(),
        None => {
            emit_state_updated(&app, &s);
            return Ok(None);
        }
    };
    // Out of range isn't worth an error: the block just isn't there.
    let Ok(new_id) = graph.delete_instruction(&strand_id, &path, x, y) else {
        emit_state_updated(&app, &s);
        return Ok(None);
    };
    push_undo(&mut s);
    if let Some(mac) = &mut s.current_macro {
        mac.graph = graph;
    }
    s.invalid_field_buffers.clear();
    auto_save(&mut s);
    emit_state_updated(&app, &s);
    Ok(new_id)
}

pub(crate) fn reorder_instruction(
    state: &SharedState,
    app: &AppHandle,
    strand_id: String,
    path: Vec<PathStep>,
    direction: i32,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    // Reordering can legitimately do nothing (either end of the list, or a
    // swap past a header), so only checkpoint if it moved.
    let mut graph = match &s.current_macro {
        Some(mac) => mac.graph.clone(),
        None => {
            emit_state_updated(&app, &s);
            return Ok(());
        }
    };
    if graph.reorder_instruction(&strand_id, &path, direction) {
        push_undo(&mut s);
        if let Some(mac) = &mut s.current_macro {
            mac.graph = graph;
        }
        s.invalid_field_buffers.clear();
        auto_save(&mut s);
    }
    emit_state_updated(&app, &s);
    Ok(())
}

pub(crate) fn clear_instructions(state: &SharedState, app: &AppHandle) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    push_undo(&mut s);
    if let Some(mac) = &mut s.current_macro {
        mac.clear_editor();
        s.invalid_field_buffers.clear();
        auto_save(&mut s);
    }
    emit_state_updated(&app, &s);
    Ok(())
}

/// Swaps the canvas for the neighbouring history entry; `step` is
/// [`History::undo`] or [`History::redo`], which are otherwise identical.
pub(super) fn apply_history_step(
    state: &SharedState,
    app: &AppHandle,
    step: fn(
        &mut crate::state::History<crate::state::EditorSnapshot>,
        crate::state::EditorSnapshot,
    ) -> Option<crate::state::EditorSnapshot>,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if let Some(current) = s.current_macro.as_ref().map(Macro::undo_snapshot)
        && let Some(restored) = step(&mut s.history, current)
    {
        if let Some(mac) = &mut s.current_macro {
            mac.restore_undo_snapshot(restored);
        }
        sync_variable_values(&mut s);
        sync_list_values(&mut s);
        s.invalid_field_buffers.clear();
        auto_save(&mut s);
    }
    emit_state_updated(app, &s);
    Ok(())
}

pub(crate) fn undo(state: &SharedState, app: &AppHandle) -> Result<(), String> {
    apply_history_step(state, app, crate::state::History::undo)
}

pub(crate) fn redo(state: &SharedState, app: &AppHandle) -> Result<(), String> {
    apply_history_step(state, app, crate::state::History::redo)
}
