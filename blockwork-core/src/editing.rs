//! The editor's canvas commands over the open macro: strands, blocks, values,
//! comments, variables, signals, lists, custom blocks and undo. The app and
//! the website's in-browser editor both run them, so a rule lives here once.
//!
//! A host hands its document over through [`Document`] and keeps what only it
//! has (disk, a live variable store) in that trait's hooks. [`dispatch`] takes
//! the command names and camelCase arguments the QML sends.

use std::collections::HashMap;

use blockstitch_core::editor::{
    Created, EditCommand, EditError, EditOutcome, EditSession, History, PathStep, ValueBuffers,
    ValueDestination, ValueLocation, apply_buffer_effects, drop_strand_buffers,
    prune_value_buffers, retain_live_buffers,
};
use blockstitch_core::graph::ListItem;
use serde::de::DeserializeOwned;
use serde_json::Value as Json;

use crate::block_packs;
use crate::frontend::{
    FragmentDto, InstructionDto, ListItemDto, dto_to_fragment, dto_to_instruction,
    dto_to_list_item, edit_reporter_branch, fragment_to_dto, targets_reporter_branch,
};
use crate::input::value::{Evaluated, Value};
use crate::macros::{
    BlockDraft, EDIT_CONTEXT, Instruction, InstructionKind, Macro, MacroGraph, UndoSnapshot,
    typed_value,
};

const NO_MACRO: &str = "No macro selected";

/// What a command edits, borrowed apart so the macro and its history can be
/// changed together.
pub struct Parts<'a> {
    pub open: Option<&'a mut Macro>,
    pub history: &'a mut History<UndoSnapshot>,
    pub buffers: &'a mut ValueBuffers,
}

/// A host's open document. The hooks keep a host's own copies in step; the
/// defaults suit a host that has none.
pub trait Document {
    fn parts(&mut self) -> Parts<'_>;
    /// The values `set_value_kind` resolves variables against.
    fn variable_values(&self) -> HashMap<String, Evaluated>;
    /// A variable now holds `value`, or is gone when it's `None`.
    fn variable_set(&mut self, _name: &str, _value: Option<Evaluated>) {}
    fn variable_renamed(&mut self, _old: &str, _new: &str) {}
    /// The macro's lists changed.
    fn lists_changed(&mut self) {}
    /// Undo or redo swapped the macro's variables and lists back.
    fn restored(&mut self) {}
    /// A command changed the open macro (the app saves here).
    fn changed(&mut self) {}
}

/// Every command [`dispatch`] answers.
pub const COMMANDS: &[&str] = &[
    "add_strand",
    "remove_strand",
    "move_strand",
    "split_strand",
    "merge_strand",
    "merge_tail",
    "clear_instructions",
    "set_recording_target",
    "add_instruction",
    "edit_instruction",
    "remove_instruction",
    "delete_instruction",
    "reorder_instruction",
    "copy_instructions",
    "paste_fragment",
    "paste_instructions",
    "duplicate_instruction",
    "edit_value_field",
    "set_value_kind",
    "take_value",
    "put_value",
    "move_value",
    "create_floating_value",
    "move_floating_value",
    "remove_floating_value",
    "create_comment",
    "create_attached_comment",
    "move_comment",
    "edit_comment_text",
    "set_comment_collapsed",
    "remove_comment",
    "create_variable",
    "rename_variable",
    "set_variable_value",
    "delete_variable",
    "create_signal",
    "rename_signal",
    "delete_signal",
    "create_list",
    "rename_list",
    "delete_list",
    "set_list_items",
    "set_list_editor_state",
    "create_block",
    "edit_block",
    "delete_block",
    "undo",
    "redo",
];

/// Commands that only read the document.
pub const READS: &[&str] = &["copy_instructions"];

fn arg<T: DeserializeOwned>(args: &Json, name: &str) -> Result<T, String> {
    let value = args.get(name).cloned().unwrap_or(Json::Null);
    serde_json::from_value(value).map_err(|e| format!("invalid argument '{name}': {e}"))
}

fn to_json<T: serde::Serialize>(value: T) -> Result<Json, String> {
    serde_json::to_value(value).map_err(|e| e.to_string())
}

fn instruction(dto: &InstructionDto) -> Result<Instruction, String> {
    dto_to_instruction(dto).ok_or_else(|| "Unknown instruction type".to_string())
}

/// A custom block's definition from `create_block`/`edit_block`'s args.
fn block_draft(args: &Json) -> Result<BlockDraft, String> {
    Ok(BlockDraft {
        pieces: arg(args, "pieces")?,
        shape: arg(args, "shape")?,
        color: arg(args, "color")?,
        icon: arg(args, "icon").unwrap_or_default(),
        identifier: arg(args, "identifier").unwrap_or_default(),
        description: arg(args, "description").unwrap_or_default(),
    })
}

/// Whether [`dispatch`] answers `cmd` with these `args`.
pub fn handles(cmd: &str, args: &Json) -> bool {
    targets_reporter_branch(args) || COMMANDS.contains(&cmd)
}

/// The instruction an `add_instruction`/`edit_instruction` call would write,
/// so a host can ask for a permission it needs first.
pub fn instruction_written(cmd: &str, args: &Json) -> Option<Instruction> {
    if !matches!(cmd, "add_instruction" | "edit_instruction") {
        return None;
    }
    let dto: InstructionDto = arg(args, "instruction").ok()?;
    dto_to_instruction(&dto)
}

/// Runs `cmd` on `doc`, or `None` when it isn't a canvas command.
pub fn dispatch<D: Document>(doc: &mut D, cmd: &str, args: &Json) -> Option<Result<Json, String>> {
    if targets_reporter_branch(args) {
        return Some(reporter_branch(doc, cmd, args));
    }
    let result = match cmd {
        "add_strand" => (|| {
            let instruction: Option<InstructionDto> = arg(args, "instruction")?;
            let instruction = instruction.as_ref().map(self::instruction).transpose()?;
            to_json(add_strand(
                doc,
                arg(args, "x")?,
                arg(args, "y")?,
                instruction,
                arg(args, "variableToCreate")?,
            )?)
        })(),
        "remove_strand" => arg(args, "strandId").map(|id: String| {
            remove_strand(doc, &id);
            Json::Null
        }),
        "move_strand" => (|| {
            move_strand(
                doc,
                &arg::<String>(args, "strandId")?,
                arg(args, "x")?,
                arg(args, "y")?,
            );
            Ok(Json::Null)
        })(),
        "split_strand" => (|| {
            let id: String = arg(args, "strandId")?;
            let path: Vec<PathStep> = arg(args, "path")?;
            to_json(split_strand(
                doc,
                &id,
                &path,
                arg(args, "x")?,
                arg(args, "y")?,
            )?)
        })(),
        "merge_strand" => (|| {
            let (dragged, target): (String, String) =
                (arg(args, "draggedId")?, arg(args, "targetId")?);
            let path: Vec<PathStep> = arg(args, "path")?;
            merge_strand(doc, &dragged, &target, &path, arg(args, "targetY")?)?;
            Ok(Json::Null)
        })(),
        "merge_tail" => (|| {
            let (id, path): (String, Vec<PathStep>) = (arg(args, "strandId")?, arg(args, "path")?);
            let (target, target_path): (String, Vec<PathStep>) =
                (arg(args, "targetId")?, arg(args, "targetPath")?);
            merge_tail(
                doc,
                &id,
                &path,
                &target,
                &target_path,
                arg(args, "targetY")?,
            )?;
            Ok(Json::Null)
        })(),
        "clear_instructions" => {
            clear_instructions(doc);
            Ok(Json::Null)
        }
        "set_recording_target" => arg(args, "strandId").map(|id: String| {
            set_recording_target(doc, id);
            Json::Null
        }),
        "add_instruction" => (|| {
            let (id, path): (String, Vec<PathStep>) = (arg(args, "strandId")?, arg(args, "path")?);
            let ins = instruction(&arg(args, "instruction")?)?;
            add_instruction(
                doc,
                &id,
                &path,
                ins,
                arg(args, "targetY")?,
                arg(args, "variableToCreate")?,
            )?;
            Ok(Json::Null)
        })(),
        "edit_instruction" => (|| {
            let (id, path): (String, Vec<PathStep>) = (arg(args, "strandId")?, arg(args, "path")?);
            edit_instruction(doc, id, path, instruction(&arg(args, "instruction")?)?);
            Ok(Json::Null)
        })(),
        "remove_instruction" => (|| {
            let count: Option<usize> = arg(args, "count")?;
            // A path addressing nothing costs no undo step and isn't an error.
            let _ = run_edit(
                doc,
                EditCommand::RemoveInstructions {
                    strand_id: arg(args, "strandId")?,
                    path: arg(args, "path")?,
                    count: count.unwrap_or(1),
                },
            );
            Ok(Json::Null)
        })(),
        "delete_instruction" => (|| {
            let (id, path): (String, Vec<PathStep>) = (arg(args, "strandId")?, arg(args, "path")?);
            to_json(delete_instruction(
                doc,
                &id,
                &path,
                arg(args, "x")?,
                arg(args, "y")?,
            ))
        })(),
        "reorder_instruction" => (|| {
            let (id, path): (String, Vec<PathStep>) = (arg(args, "strandId")?, arg(args, "path")?);
            reorder_instruction(doc, &id, &path, arg(args, "direction")?);
            Ok(Json::Null)
        })(),
        "copy_instructions" => (|| {
            let (id, path): (String, Vec<PathStep>) = (arg(args, "strandId")?, arg(args, "path")?);
            let mac = doc.parts().open.ok_or(NO_MACRO)?;
            to_json(fragment_to_dto(&mac.copy_fragment(
                &id,
                &path,
                arg(args, "count")?,
            )?))
        })(),
        "paste_fragment" | "paste_instructions" => (|| {
            let fragment: FragmentDto = if cmd == "paste_fragment" {
                arg(args, "fragment")?
            } else {
                FragmentDto {
                    instructions: arg(args, "instructions")?,
                    comments: Vec::new(),
                }
            };
            to_json(paste_fragment(
                doc,
                arg(args, "x")?,
                arg(args, "y")?,
                &fragment,
            )?)
        })(),
        "duplicate_instruction" => (|| {
            let edit = EditCommand::DuplicateInstruction {
                strand_id: arg(args, "strandId")?,
                path: arg(args, "path")?,
            };
            run_edit(doc, edit).map_err(String::from)?;
            Ok(Json::Null)
        })(),

        "edit_value_field" => (|| {
            let edit = EditCommand::EditFieldText {
                location: arg(args, "location")?,
                text: arg(args, "text")?,
            };
            // Keystrokes into one field coalesce into one undo step; a
            // location that no longer resolves is quietly ignored.
            let _ = run_edit(doc, edit);
            Ok(Json::Null)
        })(),
        "set_value_kind" => (|| {
            set_value_kind(doc, &arg(args, "location")?, &arg::<String>(args, "kind")?)?;
            Ok(Json::Null)
        })(),
        "take_value" => (|| to_json(take_value(doc, &arg(args, "location")?)?))(),
        "put_value" => (|| {
            put_value(doc, &arg(args, "location")?, arg(args, "value")?);
            Ok(Json::Null)
        })(),
        "move_value" => (|| {
            let to = match arg::<Option<ValueLocation>>(args, "to")? {
                Some(to) => ValueDestination::Slot(to),
                None => ValueDestination::Canvas {
                    x: arg::<Option<i32>>(args, "x")?.unwrap_or(0),
                    y: arg::<Option<i32>>(args, "y")?.unwrap_or(0),
                    origin_block_id: arg(args, "originBlockId")?,
                },
            };
            let edit = EditCommand::MoveValue {
                from: arg(args, "from")?,
                to,
            };
            run_edit(doc, edit).map_err(String::from)?;
            Ok(Json::Null)
        })(),
        "create_floating_value" => (|| {
            let (x, y): (i32, i32) = (arg(args, "x")?, arg(args, "y")?);
            let value: Value = arg(args, "value")?;
            to_json(create_floating_value(
                doc,
                x,
                y,
                value,
                arg(args, "originBlockId")?,
            )?)
        })(),
        "move_floating_value" => (|| {
            let id: String = arg(args, "floatingId")?;
            move_floating_value(doc, &id, arg(args, "x")?, arg(args, "y")?);
            Ok(Json::Null)
        })(),
        "remove_floating_value" => arg(args, "floatingId").map(|id: String| {
            remove_floating_value(doc, &id);
            Json::Null
        }),

        "create_comment" => (|| {
            let (x, y): (i32, i32) = (arg(args, "x")?, arg(args, "y")?);
            let text: Option<String> = arg(args, "text")?;
            to_json(create_comment(doc, x, y, text.unwrap_or_default(), None)?)
        })(),
        "create_attached_comment" => (|| {
            let (dx, dy): (i32, i32) = (arg(args, "dx")?, arg(args, "dy")?);
            let attached: String = arg(args, "instructionId")?;
            let text: Option<String> = arg(args, "text")?;
            to_json(create_comment(
                doc,
                dx,
                dy,
                text.unwrap_or_default(),
                Some(attached),
            )?)
        })(),
        "move_comment" => (|| {
            let id: String = arg(args, "commentId")?;
            move_comment(doc, &id, arg(args, "x")?, arg(args, "y")?);
            Ok(Json::Null)
        })(),
        "edit_comment_text" => (|| {
            edit_comment_text(doc, arg(args, "commentId")?, arg(args, "text")?);
            Ok(Json::Null)
        })(),
        "set_comment_collapsed" => (|| {
            let id: String = arg(args, "commentId")?;
            set_comment_collapsed(doc, &id, arg(args, "collapsed")?);
            Ok(Json::Null)
        })(),
        "remove_comment" => arg(args, "commentId").map(|id: String| {
            remove_comment(doc, &id);
            Json::Null
        }),

        "create_variable" => (|| {
            create_variable(doc, &arg::<String>(args, "name")?)?;
            Ok(Json::Null)
        })(),
        "rename_variable" => (|| {
            let (old, new): (String, String) = (arg(args, "oldName")?, arg(args, "newName")?);
            rename_variable(doc, &old, &new)?;
            Ok(Json::Null)
        })(),
        "set_variable_value" => (|| {
            let (name, value): (String, String) = (arg(args, "name")?, arg(args, "value")?);
            set_variable_value(doc, &name, &value)?;
            Ok(Json::Null)
        })(),
        "delete_variable" => (|| {
            delete_variable(doc, &arg::<String>(args, "name")?)?;
            Ok(Json::Null)
        })(),
        "create_signal" => (|| {
            create_signal(doc, &arg::<String>(args, "name")?)?;
            Ok(Json::Null)
        })(),
        "rename_signal" => (|| {
            let (old, new): (String, String) = (arg(args, "oldName")?, arg(args, "newName")?);
            rename_signal(doc, &old, &new)?;
            Ok(Json::Null)
        })(),
        "delete_signal" => (|| {
            delete_signal(doc, &arg::<String>(args, "name")?)?;
            Ok(Json::Null)
        })(),
        "create_list" => (|| {
            create_list(doc, &arg::<String>(args, "name")?)?;
            Ok(Json::Null)
        })(),
        "rename_list" => (|| {
            let (old, new): (String, String) = (arg(args, "oldName")?, arg(args, "newName")?);
            rename_list(doc, &old, &new)?;
            Ok(Json::Null)
        })(),
        "delete_list" => (|| {
            delete_list(doc, &arg::<String>(args, "name")?)?;
            Ok(Json::Null)
        })(),
        "set_list_items" => (|| {
            let (name, items): (String, Vec<ListItemDto>) =
                (arg(args, "name")?, arg(args, "items")?);
            let items: Vec<ListItem> = items.iter().map(dto_to_list_item).collect();
            graph_change(doc, |g| g.set_list_items(&name, items))?;
            doc.lists_changed();
            Ok(Json::Null)
        })(),
        "set_list_editor_state" => (|| {
            let (name, visible): (String, bool) = (arg(args, "name")?, arg(args, "visible")?);
            let (x, y): (i32, i32) = (arg(args, "x")?, arg(args, "y")?);
            // Where the card sits: a view preference, not an undo step.
            graph_change(doc, |g| g.set_list_editor_state(&name, visible, x, y))?;
            Ok(Json::Null)
        })(),

        "create_block" => (|| {
            let position = arg::<Option<i32>>(args, "x")?.zip(arg::<Option<i32>>(args, "y")?);
            to_json(create_block(doc, block_draft(args)?, position)?)
        })(),
        "edit_block" => (|| {
            edit_block(doc, &arg::<String>(args, "blockId")?, block_draft(args)?)?;
            Ok(Json::Null)
        })(),
        "delete_block" => (|| {
            delete_block(doc, &arg::<String>(args, "blockId")?)?;
            Ok(Json::Null)
        })(),

        "undo" => {
            history_step(doc, History::undo);
            Ok(Json::Null)
        }
        "redo" => {
            history_step(doc, History::redo);
            Ok(Json::Null)
        }
        _ => return None,
    };
    Some(result)
}

// ─── Undo ───────────────────────────────────────────────────────────────────

/// Checkpoints the macro before a structural edit, and ends the current
/// text-edit group so the next keystroke starts a fresh step.
pub fn push_undo<D: Document + ?Sized>(doc: &mut D) {
    push_undo_for(doc, None);
}

/// [`push_undo`] for an edit that coalesces with the keystrokes already in
/// progress at `session`: only the first one checkpoints.
fn push_undo_for<D: Document + ?Sized>(doc: &mut D, session: Option<EditSession>) {
    let parts = doc.parts();
    match parts.open.map(|mac| mac.undo_snapshot()) {
        Some(snapshot) => {
            parts.history.checkpoint(snapshot, session);
        }
        None => parts.history.end_session(),
    }
}

/// Runs one typed edit: a rejected or no-op edit costs no undo step;
/// buffers follow the outcome either way.
fn run_edit<D: Document>(
    doc: &mut D,
    command: EditCommand<InstructionKind>,
) -> Result<EditOutcome, EditError> {
    let parts = doc.parts();
    let mac = parts.open.ok_or(EditError::Missing(NO_MACRO))?;
    let prepared = mac.prepare_edit(command, &EDIT_CONTEXT)?;
    if prepared.changes() {
        parts
            .history
            .checkpoint(mac.undo_snapshot(), prepared.coalesce().cloned());
    }
    let outcome = mac.commit_edit(prepared);
    apply_buffer_effects(parts.buffers, &outcome.buffers);
    if outcome.changed {
        doc.changed();
    }
    Ok(outcome)
}

/// Commits an edit made on a copy of the graph, as one undo step.
fn commit_graph<D: Document>(doc: &mut D, graph: MacroGraph) {
    push_undo(doc);
    let parts = doc.parts();
    if let Some(mac) = parts.open {
        mac.graph = graph;
    }
    parts.buffers.clear();
    doc.changed();
}

/// A copy of the open macro's graph, to try an edit on first.
fn graph_copy<D: Document>(doc: &mut D) -> Result<MacroGraph, String> {
    Ok(doc.parts().open.ok_or(NO_MACRO)?.graph.clone())
}

/// A graph change that isn't an undo step.
fn graph_change<D: Document, T>(
    doc: &mut D,
    edit: impl FnOnce(&mut MacroGraph) -> Result<T, String>,
) -> Result<T, String> {
    let result = edit(&mut doc.parts().open.ok_or(NO_MACRO)?.graph)?;
    doc.changed();
    Ok(result)
}

/// Swaps the macro for the neighbouring history entry.
fn history_step<D: Document>(
    doc: &mut D,
    step: fn(&mut History<UndoSnapshot>, UndoSnapshot) -> Option<UndoSnapshot>,
) {
    let parts = doc.parts();
    let Some(mac) = parts.open else { return };
    if let Some(restored) = step(parts.history, mac.undo_snapshot()) {
        mac.restore_undo_snapshot(restored);
        parts.buffers.clear();
        doc.restored();
        doc.changed();
    }
}

/// A reporter body's blocks, edited on a copy so a rejected edit costs no
/// undo step.
fn reporter_branch<D: Document>(doc: &mut D, cmd: &str, args: &Json) -> Result<Json, String> {
    let mut graph = graph_copy(doc)?;
    let result = edit_reporter_branch(&mut graph, cmd, args)?;
    if !READS.contains(&cmd) {
        commit_graph(doc, graph);
    }
    Ok(result)
}

// ─── Strands ────────────────────────────────────────────────────────────────

/// A new strand, at `x`/`y` or beside the farthest-right one. A palette drop
/// passes its block (and any variable it names) to land in one step.
pub fn add_strand<D: Document>(
    doc: &mut D,
    x: Option<i32>,
    y: Option<i32>,
    instruction: Option<Instruction>,
    variable_to_create: Option<String>,
) -> Result<String, String> {
    push_undo(doc);
    let mac = doc.parts().open.ok_or(NO_MACRO)?;
    let (default_x, default_y) = mac.next_editor_position();
    let has_instruction = instruction.is_some();
    let id = mac.add_strand(
        x.unwrap_or(default_x),
        y.unwrap_or(default_y),
        instruction.into_iter().collect(),
    );
    if has_instruction {
        ensure_inserted_variable(doc, variable_to_create);
    }
    doc.changed();
    Ok(id)
}

pub fn remove_strand<D: Document>(doc: &mut D, strand_id: &str) {
    push_undo(doc);
    let parts = doc.parts();
    if let Some(mac) = parts.open {
        mac.remove_strand(strand_id);
        drop_strand_buffers(parts.buffers, strand_id);
        doc.changed();
    }
}

/// Moves a strand dropped on empty canvas. Not an undo step.
pub fn move_strand<D: Document>(doc: &mut D, strand_id: &str, x: i32, y: i32) {
    if let Some(mac) = doc.parts().open
        && mac.move_strand(strand_id, x, y)
    {
        doc.changed();
    }
}

/// Detaches the blocks at and after `path` into a new strand at `(x, y)`:
/// how the canvas picks up a block before dropping it elsewhere.
pub fn split_strand<D: Document>(
    doc: &mut D,
    strand_id: &str,
    path: &[PathStep],
    x: i32,
    y: i32,
) -> Result<String, String> {
    push_undo(doc);
    let parts = doc.parts();
    let id = parts
        .open
        .ok_or(NO_MACRO)?
        .split_strand(strand_id, path, x, y)?;
    parts.buffers.clear();
    doc.changed();
    Ok(id)
}

/// Keeps a strand's top where a block prepended to it was dropped.
fn hold_top(graph: &mut MacroGraph, strand_id: &str, path: &[PathStep], target_y: Option<i32>) {
    if let Some(y) = target_y
        && path.len() == 1
        && path[0].index == 0
        && let Some(strand) = graph.strand_mut(strand_id)
    {
        strand.y = y;
    }
}

/// Splices `dragged_id`'s blocks into `target_id` at `path` and drops the
/// emptied strand: how two stacks snap together.
pub fn merge_strand<D: Document>(
    doc: &mut D,
    dragged_id: &str,
    target_id: &str,
    path: &[PathStep],
    target_y: Option<i32>,
) -> Result<(), String> {
    let mut graph = graph_copy(doc)?;
    graph.merge_strand(dragged_id, target_id, path)?;
    hold_top(&mut graph, target_id, path, target_y);
    push_undo(doc);
    let parts = doc.parts();
    if let Some(mac) = parts.open {
        mac.graph = graph;
    }
    drop_strand_buffers(parts.buffers, dragged_id);
    doc.changed();
    Ok(())
}

/// Moves the tail at `path` into `target_id` at `target_path` in one step.
pub fn merge_tail<D: Document>(
    doc: &mut D,
    strand_id: &str,
    path: &[PathStep],
    target_id: &str,
    target_path: &[PathStep],
    target_y: Option<i32>,
) -> Result<(), String> {
    let mut graph = graph_copy(doc)?;
    graph.merge_tail(strand_id, path, target_id, target_path)?;
    hold_top(&mut graph, target_id, target_path, target_y);
    commit_graph(doc, graph);
    Ok(())
}

pub fn clear_instructions<D: Document>(doc: &mut D) {
    push_undo(doc);
    let parts = doc.parts();
    if let Some(mac) = parts.open {
        mac.clear_editor();
        parts.buffers.clear();
        doc.changed();
    }
}

/// The strand recorded input is appended to. Not an undo step; an unknown
/// strand is ignored.
pub fn set_recording_target<D: Document>(doc: &mut D, strand_id: String) {
    if let Some(mac) = doc.parts().open
        && mac.strand(&strand_id).is_some()
    {
        mac.recording_target = Some(strand_id);
        doc.changed();
    }
}

/// Drops a copied fragment at `(x, y)` as a new strand with fresh ids, in
/// one step. Returns that strand's id.
pub fn paste_fragment<D: Document>(
    doc: &mut D,
    x: i32,
    y: i32,
    fragment: &FragmentDto,
) -> Result<String, String> {
    let fragment = dto_to_fragment(fragment).ok_or("Unknown instruction type")?;
    run_edit(doc, EditCommand::PasteFragment { fragment, x, y })?
        .created
        .into_iter()
        .find_map(|created| match created {
            Created::Strand(id) => Some(id),
            _ => None,
        })
        .ok_or_else(|| "Nothing was pasted".to_string())
}

// ─── Blocks ─────────────────────────────────────────────────────────────────

/// Declares a palette block's variable once the block is in; a name that
/// can't be made (already there) is left alone.
fn ensure_inserted_variable<D: Document>(doc: &mut D, name: Option<String>) {
    let Some(name) = name else { return };
    let Some(mac) = doc.parts().open else { return };
    if let Ok(name) = mac.create_variable(&name) {
        doc.variable_set(&name, Some(Evaluated::Number(0.0)));
    }
}

pub fn add_instruction<D: Document>(
    doc: &mut D,
    strand_id: &str,
    path: &[PathStep],
    ins: Instruction,
    target_y: Option<i32>,
    variable_to_create: Option<String>,
) -> Result<(), String> {
    // Checked before checkpointing, so a rejected drop costs no undo step.
    if let Some(mac) = doc.parts().open {
        mac.check_insert(strand_id, path, &ins)?;
    }
    push_undo(doc);
    let Some(mac) = doc.parts().open else {
        return Ok(());
    };
    if mac.insert_instruction(strand_id, path, ins)? {
        hold_top(&mut mac.graph, strand_id, path, target_y);
        ensure_inserted_variable(doc, variable_to_create);
        doc.parts().buffers.clear();
        doc.changed();
    }
    Ok(())
}

/// Replaces a block. Free text coalesces keystrokes into one undo step, like
/// `edit_value_field`; every other kind is a step of its own.
pub fn edit_instruction<D: Document>(
    doc: &mut D,
    strand_id: String,
    path: Vec<PathStep>,
    ins: Instruction,
) {
    let session = ins.kind.edits_as_text().then(|| EditSession::Instruction {
        strand_id: strand_id.clone(),
        index: path.clone(),
    });
    push_undo_for(doc, session);
    if let Some(mac) = doc.parts().open
        && mac.replace_instruction(&strand_id, &path, ins)
    {
        doc.changed();
    }
}

/// Deletes the block at `path`, splitting what was below it into a new
/// strand at `(x, y)` in the same step. Returns that strand's id, if any.
pub fn delete_instruction<D: Document>(
    doc: &mut D,
    strand_id: &str,
    path: &[PathStep],
    x: i32,
    y: i32,
) -> Option<String> {
    let mut graph = graph_copy(doc).ok()?;
    // Out of range isn't worth an error: the block just isn't there.
    let new_id = graph.delete_instruction(strand_id, path, x, y).ok()?;
    commit_graph(doc, graph);
    new_id
}

/// Moves a block up or down. Either end of a list, or a swap past a header,
/// does nothing and costs no undo step.
pub fn reorder_instruction<D: Document>(
    doc: &mut D,
    strand_id: &str,
    path: &[PathStep],
    direction: i32,
) {
    let Ok(mut graph) = graph_copy(doc) else {
        return;
    };
    if graph.reorder_instruction(strand_id, path, direction) {
        commit_graph(doc, graph);
    }
}

// ─── Values ─────────────────────────────────────────────────────────────────

pub fn set_value_kind<D: Document>(
    doc: &mut D,
    location: &ValueLocation,
    kind: &str,
) -> Result<(), String> {
    push_undo(doc);
    let env = doc.variable_values();
    let parts = doc.parts();
    let result = match parts.open {
        Some(mac) => mac.set_value_kind(location, kind, &env),
        None => Ok(()),
    };
    prune_value_buffers(parts.buffers, location);
    doc.changed();
    result
}

/// Takes the value at `location` out and returns it: a slot falls back to
/// what it was shadowing, a floating value is deleted.
pub fn take_value<D: Document>(doc: &mut D, location: &ValueLocation) -> Result<Value, String> {
    push_undo(doc);
    let parts = doc.parts();
    let taken = parts.open.and_then(|mac| mac.take_value(location));
    prune_value_buffers(parts.buffers, location);
    doc.changed();
    taken.ok_or_else(|| "Nothing to take at that location".to_string())
}

/// Writes `value` over the node at `location`: the "put" half of a drag.
pub fn put_value<D: Document>(doc: &mut D, location: &ValueLocation, value: Value) {
    push_undo(doc);
    let parts = doc.parts();
    if let Some(mac) = parts.open {
        mac.put_value(location, value);
    }
    prune_value_buffers(parts.buffers, location);
    doc.changed();
}

/// A reporter parked on open canvas. `origin_block_id` names the custom
/// block a parameter was dragged out of, so it keeps its declared shape.
pub fn create_floating_value<D: Document>(
    doc: &mut D,
    x: i32,
    y: i32,
    value: Value,
    origin_block_id: Option<String>,
) -> Result<String, String> {
    push_undo(doc);
    let id = doc
        .parts()
        .open
        .ok_or(NO_MACRO)?
        .add_floating_value(x, y, value, origin_block_id);
    doc.changed();
    Ok(id)
}

/// Not an undo step, like `move_strand`.
pub fn move_floating_value<D: Document>(doc: &mut D, id: &str, x: i32, y: i32) {
    if let Some(mac) = doc.parts().open
        && mac.move_floating_value(id, x, y)
    {
        doc.changed();
    }
}

pub fn remove_floating_value<D: Document>(doc: &mut D, id: &str) {
    push_undo(doc);
    if let Some(mac) = doc.parts().open {
        mac.remove_floating_value(id);
        doc.changed();
    }
}

// ─── Comments ───────────────────────────────────────────────────────────────

/// A note on open canvas, or pinned to `attached_to` at an offset from it.
pub fn create_comment<D: Document>(
    doc: &mut D,
    x: i32,
    y: i32,
    text: String,
    attached_to: Option<String>,
) -> Result<String, String> {
    push_undo(doc);
    let id = doc
        .parts()
        .open
        .ok_or(NO_MACRO)?
        .add_comment(x, y, text, attached_to);
    doc.changed();
    Ok(id)
}

/// Not an undo step. `x`/`y` is an offset when the note is pinned.
pub fn move_comment<D: Document>(doc: &mut D, id: &str, x: i32, y: i32) {
    if let Some(mac) = doc.parts().open
        && mac.move_comment(id, x, y)
    {
        doc.changed();
    }
}

/// Keystrokes into one note coalesce into one undo step.
pub fn edit_comment_text<D: Document>(doc: &mut D, id: String, text: String) {
    push_undo_for(
        doc,
        Some(EditSession::Comment {
            comment_id: id.clone(),
        }),
    );
    if let Some(mac) = doc.parts().open
        && mac.set_comment_text(&id, text)
    {
        doc.changed();
    }
}

/// View state, not content, so not an undo step.
pub fn set_comment_collapsed<D: Document>(doc: &mut D, id: &str, collapsed: bool) {
    if let Some(mac) = doc.parts().open
        && mac.set_comment_collapsed(id, collapsed)
    {
        doc.changed();
    }
}

pub fn remove_comment<D: Document>(doc: &mut D, id: &str) {
    push_undo(doc);
    if let Some(mac) = doc.parts().open {
        mac.remove_comment(id);
        doc.changed();
    }
}

// ─── Variables, signals and lists ───────────────────────────────────────────

/// A new variable starting at 0. Naming isn't an undo step.
pub fn create_variable<D: Document>(doc: &mut D, name: &str) -> Result<(), String> {
    let name = doc.parts().open.ok_or(NO_MACRO)?.create_variable(name)?;
    doc.variable_set(&name, Some(Evaluated::Number(0.0)));
    doc.changed();
    Ok(())
}

/// Renames a variable and every reference to it. Tried on a copy first, so
/// a rejected or unchanged name adds no undo step.
pub fn rename_variable<D: Document>(doc: &mut D, old: &str, new: &str) -> Result<(), String> {
    let mut graph = graph_copy(doc)?;
    let trimmed = graph.rename_variable(old, new)?;
    if trimmed == old {
        return Ok(());
    }
    push_undo(doc);
    if let Some(mac) = doc.parts().open {
        mac.graph = graph;
    }
    doc.variable_renamed(old, &trimmed);
    doc.changed();
    Ok(())
}

/// The value the next run starts from. Text that reads as a number is
/// stored as one.
pub fn set_variable_value<D: Document>(doc: &mut D, name: &str, value: &str) -> Result<(), String> {
    let value = typed_value(value);
    doc.parts()
        .open
        .ok_or(NO_MACRO)?
        .set_variable_value(name, value.clone())?;
    doc.variable_set(name, Some(value));
    doc.changed();
    Ok(())
}

/// Not an undo step, like `create_variable`.
pub fn delete_variable<D: Document>(doc: &mut D, name: &str) -> Result<(), String> {
    doc.parts().open.ok_or(NO_MACRO)?.remove_variable(name);
    doc.variable_set(name, None);
    doc.changed();
    Ok(())
}

pub fn create_signal<D: Document>(doc: &mut D, name: &str) -> Result<(), String> {
    let name = doc
        .parts()
        .open
        .ok_or(NO_MACRO)?
        .validate_signal_name(name, None)?;
    push_undo(doc);
    doc.parts().open.ok_or(NO_MACRO)?.signals.push(name);
    doc.changed();
    Ok(())
}

pub fn rename_signal<D: Document>(doc: &mut D, old: &str, new: &str) -> Result<(), String> {
    let mac = doc.parts().open.ok_or(NO_MACRO)?;
    let Some(name) = mac.check_signal_rename(old, new)? else {
        return Ok(());
    };
    push_undo(doc);
    doc.parts()
        .open
        .ok_or(NO_MACRO)?
        .rename_signal(old, &name)?;
    doc.changed();
    Ok(())
}

pub fn delete_signal<D: Document>(doc: &mut D, name: &str) -> Result<(), String> {
    doc.parts()
        .open
        .ok_or(NO_MACRO)?
        .check_signal_deletable(name)?;
    push_undo(doc);
    doc.parts()
        .open
        .ok_or(NO_MACRO)?
        .signals
        .retain(|s| s != name);
    doc.changed();
    Ok(())
}

/// Checked before the checkpoint, so a rejected name adds no undo step.
pub fn create_list<D: Document>(doc: &mut D, name: &str) -> Result<(), String> {
    doc.parts()
        .open
        .ok_or(NO_MACRO)?
        .check_list_name(name, None)?;
    push_undo(doc);
    doc.parts().open.ok_or(NO_MACRO)?.add_list(name)?;
    doc.lists_changed();
    doc.changed();
    Ok(())
}

pub fn rename_list<D: Document>(doc: &mut D, old: &str, new: &str) -> Result<(), String> {
    let trimmed = doc
        .parts()
        .open
        .ok_or(NO_MACRO)?
        .check_list_name(new, Some(old))?;
    if trimmed == old {
        return Ok(());
    }
    push_undo(doc);
    doc.parts()
        .open
        .ok_or(NO_MACRO)?
        .graph
        .rename_list(old, &trimmed)?;
    doc.lists_changed();
    doc.changed();
    Ok(())
}

pub fn delete_list<D: Document>(doc: &mut D, name: &str) -> Result<(), String> {
    let mac = doc.parts().open.ok_or(NO_MACRO)?;
    if mac.lists.iter().any(|list| list.name == name) {
        push_undo(doc);
        if let Some(mac) = doc.parts().open {
            mac.graph.remove_list(name);
        }
    }
    doc.lists_changed();
    doc.changed();
    Ok(())
}

// ─── Custom blocks ──────────────────────────────────────────────────────────

/// A new custom block with its (empty) header strand at `position`, or
/// beside the existing strands.
pub fn create_block<D: Document>(
    doc: &mut D,
    draft: BlockDraft,
    position: Option<(i32, i32)>,
) -> Result<String, String> {
    draft.check()?;
    push_undo(doc);
    let id = doc
        .parts()
        .open
        .ok_or(NO_MACRO)?
        .add_block(draft, position)?;
    doc.changed();
    Ok(id)
}

/// Updates a block's definition, moving every call's arguments and branch
/// bodies along by piece id. Checked first, so a rejected edit costs no
/// undo step.
pub fn edit_block<D: Document>(
    doc: &mut D,
    block_id: &str,
    draft: BlockDraft,
) -> Result<(), String> {
    doc.parts()
        .open
        .ok_or(NO_MACRO)?
        .check_block_edit(block_id, &draft)?;
    push_undo(doc);
    doc.parts()
        .open
        .ok_or(NO_MACRO)?
        .edit_block(block_id, draft)?;
    doc.changed();
    Ok(())
}

/// Deletes a custom block, its definition strand and its calls.
pub fn delete_block<D: Document>(doc: &mut D, block_id: &str) -> Result<(), String> {
    block_packs::check_editable(doc.parts().open.ok_or(NO_MACRO)?, block_id)?;
    push_undo(doc);
    let parts = doc.parts();
    if let Some(mac) = parts.open {
        mac.remove_block(block_id);
        let surviving: Vec<String> = mac.strands.iter().map(|st| st.id.clone()).collect();
        retain_live_buffers(parts.buffers, &surviving);
        doc.changed();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// A host with no hooks, counting saves.
    struct Doc {
        mac: Option<Macro>,
        history: History<UndoSnapshot>,
        buffers: ValueBuffers,
        saves: usize,
    }

    impl Document for Doc {
        fn parts(&mut self) -> Parts<'_> {
            Parts {
                open: self.mac.as_mut(),
                history: &mut self.history,
                buffers: &mut self.buffers,
            }
        }
        fn variable_values(&self) -> HashMap<String, Evaluated> {
            HashMap::new()
        }
        fn changed(&mut self) {
            self.saves += 1;
        }
    }

    fn doc() -> Doc {
        Doc {
            mac: Some(Macro::new("Test".into(), String::new(), vec![])),
            history: History::new(crate::macros::UNDO_LIMIT),
            buffers: ValueBuffers::new(),
            saves: 0,
        }
    }

    fn run(doc: &mut Doc, cmd: &str, args: Json) -> Result<Json, String> {
        dispatch(doc, cmd, &args).expect("a canvas command")
    }

    #[test]
    fn an_edit_saves_and_undo_takes_it_back() {
        let mut d = doc();
        let before = d.mac.as_ref().unwrap().strands.len();
        run(&mut d, "add_strand", json!({})).unwrap();
        assert_eq!(d.mac.as_ref().unwrap().strands.len(), before + 1);
        assert_eq!(d.saves, 1);
        run(&mut d, "undo", json!({})).unwrap();
        assert_eq!(d.mac.as_ref().unwrap().strands.len(), before);
        assert!(d.history.can_redo());
    }

    #[test]
    fn a_rejected_edit_costs_no_undo_step_or_save() {
        let mut d = doc();
        let missing = json!({"draggedId": "nope", "targetId": "nope", "path": []});
        assert!(run(&mut d, "merge_strand", missing).is_err());
        assert!(!d.history.can_undo());
        assert_eq!(d.saves, 0);
        // Moving is a view change: saved, but not an undo step.
        let id = run(&mut d, "add_strand", json!({})).unwrap();
        run(
            &mut d,
            "move_strand",
            json!({"strandId": id, "x": 5, "y": 6}),
        )
        .unwrap();
        run(&mut d, "undo", json!({})).unwrap();
        assert!(!d.history.can_undo());
    }

    #[test]
    fn other_commands_are_left_to_the_host() {
        assert!(dispatch(&mut doc(), "run_macro", &json!({})).is_none());
    }

    /// Every command `dispatch` matches is listed, so hosts and the page's
    /// contract check see it.
    #[test]
    fn every_dispatched_command_is_listed() {
        let source = include_str!("editing.rs");
        let body = &source[source.find("    let result = match cmd {").unwrap()..];
        let mut seen = 0;
        for line in body.lines().skip(1) {
            let trimmed = line.trim_start();
            if line.len() - trimmed.len() < 8 && !trimmed.is_empty() {
                break;
            }
            if line.len() - trimmed.len() != 8 || !trimmed.starts_with('"') {
                continue;
            }
            let pattern = trimmed.split("=>").next().unwrap();
            for name in pattern.split('"').skip(1).step_by(2) {
                seen += 1;
                assert!(COMMANDS.contains(&name), "add `{name}` to COMMANDS");
            }
        }
        assert_eq!(seen, COMMANDS.len());
    }
}
