//! Strands on the canvas: add, move, split, merge, copy and paste.

use super::*;

/// Creates a new detached strand. `x`/`y` default to an auto-picked spot
/// next to the farthest-right strand; an explicit position and initial
/// `instruction` are passed for a palette-block drop, as one atomic call.
pub(crate) fn add_strand(
    state: &SharedState,
    app: &AppHandle,
    x: Option<i32>,
    y: Option<i32>,
    instruction: Option<InstructionDto>,
    variable_to_create: Option<String>,
) -> Result<String, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    let ins = match instruction {
        Some(dto) => vec![dto_to_instruction(&dto).ok_or("Unknown instruction type")?],
        None => vec![],
    };
    push_undo(&mut s);
    let mac = s.current_macro.as_mut().ok_or("No macro selected")?;
    let (default_x, default_y) = mac.next_editor_position();
    let has_instruction = !ins.is_empty();
    let new_id = mac.add_strand(x.unwrap_or(default_x), y.unwrap_or(default_y), ins);
    if has_instruction {
        ensure_inserted_variable(&mut s, variable_to_create);
    }
    auto_save(&mut s);
    emit_state_updated(app, &s);
    Ok(new_id)
}

pub(crate) fn remove_strand(
    state: &SharedState,
    app: &AppHandle,
    strand_id: String,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    push_undo(&mut s);
    if let Some(mac) = &mut s.current_macro {
        mac.remove_strand(&strand_id);
        drop_strand_buffers(&mut s.invalid_field_buffers, &strand_id);
        auto_save(&mut s);
    }
    emit_state_updated(app, &s);
    Ok(())
}

/// Repositions a strand on the canvas - used while dragging a stack that
/// ends up dropped on empty space rather than snapped onto another strand.
pub(crate) fn move_strand(
    state: &SharedState,
    app: &AppHandle,
    strand_id: String,
    x: i32,
    y: i32,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if let Some(mac) = &mut s.current_macro
        && mac.move_strand(&strand_id, x, y)
    {
        auto_save(&mut s);
    }
    emit_state_updated(app, &s);
    Ok(())
}

/// Detaches the instructions at and after `path` (within the same body list)
/// into a new top-level strand at `(x, y)`, returning its id. This is how
/// the frontend "picks up" a block: split first, then drop as a stray strand
/// or re-merge elsewhere.
pub(crate) fn split_strand(
    state: &SharedState,
    app: &AppHandle,
    strand_id: String,
    path: Vec<PathStep>,
    x: i32,
    y: i32,
) -> Result<String, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    push_undo(&mut s);
    let new_id = s
        .current_macro
        .as_mut()
        .ok_or("No macro selected")?
        .split_strand(&strand_id, &path, x, y)?;
    s.invalid_field_buffers.clear();
    auto_save(&mut s);
    emit_state_updated(app, &s);
    Ok(new_id)
}

/// Splices `dragged_id`'s instructions into `target_id` at `path` and
/// deletes the (now empty) dragged strand - how two stacks snap together.
/// A "When Ran" strand can only be a merge target, never the dragged side.
pub(crate) fn merge_strand(
    state: &SharedState,
    app: &AppHandle,
    dragged_id: String,
    target_id: String,
    path: Vec<PathStep>,
    target_y: Option<i32>,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    // Merge validates before it moves anything, so a rejected drop leaves
    // both strands and the undo stack untouched.
    let mut graph = s
        .current_macro
        .as_ref()
        .map(|mac| mac.graph.clone())
        .ok_or("No macro selected")?;
    graph.merge_strand(&dragged_id, &target_id, &path)?;
    if let Some(y) = target_y
        && path.len() == 1
        && path[0].index == 0
        && let Some(strand) = graph.strand_mut(&target_id)
    {
        strand.y = y;
    }
    push_undo(&mut s);
    if let Some(mac) = &mut s.current_macro {
        mac.graph = graph;
    }
    drop_strand_buffers(&mut s.invalid_field_buffers, &dragged_id);
    auto_save(&mut s);
    emit_state_updated(app, &s);
    Ok(())
}

/// Moves the tail at and after `path` in `strand_id` into `target_id` at
/// `target_path` atomically - the Qt canvas's attach-on-drop for a partial
/// stack drag, without a split-then-merge round trip (the Qt bridge drops
/// backend replies, so it can't chain the new split id into a merge).
pub(crate) fn merge_tail(
    state: &SharedState,
    app: &AppHandle,
    strand_id: String,
    path: Vec<PathStep>,
    target_id: String,
    target_path: Vec<PathStep>,
    target_y: Option<i32>,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    let mut graph = s
        .current_macro
        .as_ref()
        .map(|mac| mac.graph.clone())
        .ok_or("No macro selected")?;
    graph.merge_tail(&strand_id, &path, &target_id, &target_path)?;
    if let Some(y) = target_y
        && target_path.len() == 1
        && target_path[0].index == 0
        && let Some(strand) = graph.strand_mut(&target_id)
    {
        strand.y = y;
    }
    push_undo(&mut s);
    if let Some(mac) = &mut s.current_macro {
        mac.graph = graph;
    }
    s.invalid_field_buffers.clear();
    auto_save(&mut s);
    emit_state_updated(app, &s);
    Ok(())
}

/// Creates a new detached strand at `(x, y)` holding `instructions`
/// verbatim - how "Paste" drops previously-copied blocks onto the canvas.
/// `count` blocks from `path` (the rest of that list when absent), with
/// their bodies and the notes pinned to them, for the editor's copy buffer.
pub(crate) fn copy_instructions(
    state: &SharedState,
    strand_id: String,
    path: Vec<PathStep>,
    count: Option<usize>,
) -> Result<FragmentDto, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    let mac = s.current_macro.as_ref().ok_or("No macro selected")?;
    Ok(fragment_to_dto(
        &mac.copy_fragment(&strand_id, &path, count)?,
    ))
}

/// Drops a copied fragment at `(x, y)` as a new strand, with fresh ids and
/// its notes, as one step. Returns that strand's id.
pub(crate) fn paste_fragment(
    state: &SharedState,
    app: &AppHandle,
    x: i32,
    y: i32,
    fragment: FragmentDto,
) -> Result<String, String> {
    let fragment = dto_to_fragment(&fragment).ok_or("Unknown instruction type")?;
    let mut s = state.lock().map_err(|e| e.to_string())?;
    let outcome = run_edit(&mut s, EditCommand::PasteFragment { fragment, x, y });
    emit_state_updated(app, &s);
    outcome?
        .created
        .into_iter()
        .find_map(|created| match created {
            Created::Strand(id) => Some(id),
            _ => None,
        })
        .ok_or_else(|| "Nothing was pasted".to_string())
}

/// [`paste_fragment`] for bare instructions, without notes.
pub(crate) fn paste_instructions(
    state: &SharedState,
    app: &AppHandle,
    x: i32,
    y: i32,
    instructions: Vec<InstructionDto>,
) -> Result<String, String> {
    let fragment = FragmentDto {
        instructions,
        comments: Vec::new(),
    };
    paste_fragment(state, app, x, y, fragment)
}

/// Copies the block at `path` (bodies and notes included) in right after
/// it, as one step.
pub(crate) fn duplicate_instruction(
    state: &SharedState,
    app: &AppHandle,
    strand_id: String,
    path: Vec<PathStep>,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    let outcome = run_edit(
        &mut s,
        EditCommand::DuplicateInstruction { strand_id, path },
    );
    emit_state_updated(app, &s);
    outcome.map(|_| ()).map_err(String::from)
}

/// Sets the strand that freshly-recorded input is appended to. Not part of
/// the undo/redo stacks, so it survives undo/redo untouched; no-ops if
/// `strand_id` doesn't exist.
pub(crate) fn set_recording_target(
    state: &SharedState,
    app: &AppHandle,
    strand_id: String,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if let Some(mac) = &mut s.current_macro
        && mac.strand(&strand_id).is_some()
    {
        mac.recording_target = Some(strand_id);
        auto_save(&mut s);
    }
    emit_state_updated(app, &s);
    Ok(())
}
