//! Custom blocks ("My Blocks").

use super::*;
use blockwork_core::macros::BlockDraft;

/// Defines a new custom block, creating its (initially empty) header strand
/// at the requested position, or beside existing strands. Pushes undo.
pub(crate) fn create_block(
    state: &SharedState,
    app: &AppHandle,
    pieces: Vec<BlockPiece>,
    shape: BlockShape,
    color: String,
    icon: String,
    identifier: String,
    description: String,
    position: Option<(i32, i32)>,
) -> Result<String, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    let draft = BlockDraft {
        pieces,
        shape,
        color,
        icon,
        identifier,
        description,
    };
    draft.check()?;
    push_undo(&mut s);
    let id = s
        .current_macro
        .as_mut()
        .ok_or("No macro selected")?
        .add_block(draft, position)?;
    auto_save(&mut s);
    emit_state_updated(&app, &s);
    Ok(id)
}

/// Updates a block's prototype/return-type, reconciling every call site's
/// `args` to the new input list and renaming body params that kept their
/// identity but changed name. Pushes undo.
pub(crate) fn edit_block(
    state: &SharedState,
    app: &AppHandle,
    block_id: String,
    pieces: Vec<BlockPiece>,
    shape: BlockShape,
    color: String,
    icon: String,
    identifier: String,
    description: String,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    let draft = BlockDraft {
        pieces,
        shape,
        color,
        icon,
        identifier,
        description,
    };
    // Checked first so a rejected edit leaves no useless undo step.
    s.current_macro
        .as_ref()
        .ok_or("No macro selected")?
        .check_block_edit(&block_id, &draft)?;
    push_undo(&mut s);
    s.current_macro
        .as_mut()
        .ok_or("No macro selected")?
        .edit_block(&block_id, draft)?;
    auto_save(&mut s);
    emit_state_updated(&app, &s);
    Ok(())
}

/// Deletes a custom block entirely (see `Macro::remove_block`). Pushes
/// undo, same reasoning as `create_block`.
pub(crate) fn delete_block(
    state: &SharedState,
    app: &AppHandle,
    block_id: String,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    blockwork_core::block_packs::check_editable(
        s.current_macro.as_ref().ok_or("No macro selected")?,
        &block_id,
    )?;
    push_undo(&mut s);
    if let Some(mac) = &mut s.current_macro {
        mac.remove_block(&block_id);
        let surviving: Vec<String> = mac.strands.iter().map(|st| st.id.clone()).collect();
        retain_live_buffers(&mut s.invalid_field_buffers, &surviving);
        auto_save(&mut s);
    }
    emit_state_updated(&app, &s);
    Ok(())
}
