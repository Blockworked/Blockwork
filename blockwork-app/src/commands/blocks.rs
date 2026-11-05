//! Custom blocks ("My Blocks").

use super::*;

/// Colors are stored as six-digit CSS hex values. Restricting the persisted
/// value keeps imported/user-created macros from injecting arbitrary CSS when
/// the frontend uses it as a custom property on a block row.
pub(super) fn normalize_block_color(color: &str) -> Result<String, String> {
    normalize_persisted_block_color(color).ok_or_else(|| "Choose a valid block color".to_string())
}

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
    BlockDef::validate_pieces(&pieces)?;
    blockwork_core::block_packs::check_identifier(&identifier)?;
    let color = normalize_block_color(&color)?;
    push_undo(&mut s);
    let mac = s.current_macro.as_mut().ok_or("No macro selected")?;
    let (x, y) = position.unwrap_or_else(|| mac.next_editor_position());
    let id = mac.create_block(pieces, shape, color, x, y);
    mac.set_block_details(&id, &identifier, &description, &icon)?;
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
    blockwork_core::block_packs::check_editable(
        s.current_macro.as_ref().ok_or("No macro selected")?, &block_id,
    )?;
    // `update_block` re-checks both; doing it first keeps a rejected edit
    // from leaving a useless undo step.
    BlockDef::validate_pieces(&pieces)?;
    blockwork_core::block_packs::check_identifier(&identifier)?;
    normalize_block_color(&color)?;
    push_undo(&mut s);
    let mac = s.current_macro.as_mut().ok_or("No macro selected")?;
    if let Some(def) = mac.block_defs.iter().find(|b| b.id == block_id) {
        let old_pieces = def.pieces.clone();
        let renames: Vec<(String, String)> = pieces
            .iter()
            .filter_map(|new_piece| {
                let BlockPiece::Branch { id, name: new_name } = new_piece else {
                    return None;
                };
                let old_name = old_pieces.iter().find_map(|p| match p {
                    BlockPiece::Branch { id: old_id, name } if old_id == id => Some(name),
                    _ => None,
                })?;
                (old_name != new_name).then(|| (old_name.clone(), new_name.clone()))
            })
            .collect();
        for (old_name, new_name) in &renames {
            mac.rename_block_branch_body(&block_id, old_name, new_name);
        }
    }
    s.current_macro
        .as_mut()
        .ok_or("No macro selected")?
        .update_block(&block_id, pieces, shape, &color)?;
    s.current_macro
        .as_mut()
        .ok_or("No macro selected")?
        .set_block_details(&block_id, &identifier, &description, &icon)?;
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
        s.current_macro.as_ref().ok_or("No macro selected")?, &block_id,
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
