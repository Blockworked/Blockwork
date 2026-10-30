//! One browser tab's macros: what the desktop app's `commands.rs` does to the
//! open macro, over the same `Macro` and blockstitch calls, without disk,
//! daemon or runner. Each command keeps the app's undo, buffer and
//! validation rules; where the page differs it says so.

use std::collections::HashMap;

use blockstitch_core::editor::{
    Created, EditCommand, EditContext, EditOutcome, EditSession, History, NumberSpelling, PathStep,
    ValueBuffers, ValueDestination, ValueLocation, apply_buffer_effects, drop_strand_buffers,
    prune_value_buffers, retain_live_buffers,
};
use blockstitch_core::graph::{BlockKind, ListItem, resolve_list_reporters};
use blockwork_core::block_packs::{self, PackRecord};
use blockwork_core::frontend::{
    FragmentDto, InstructionDto, InvalidFieldDto, KeyCaptureDto, ListItemDto, dto_to_fragment,
    dto_to_instruction, dto_to_list_item, fragment_to_dto, macro_to_dto,
};
use blockwork_core::input::key_names::{format_key_combo, match_key};
use blockwork_core::input::types::{InputToken, MacroKey};
use blockwork_core::input::value::{Evaluated, Op, Value, has_side_effect_operator, is_pure_operator};
use blockwork_core::key_mapping::{web_code_to_macro_key, web_key_to_macro_key};
use blockwork_core::macros::conditions::{PowerCondition, parse_clock};
use blockwork_core::macros::{
    BlockDef, BlockPiece, BlockShape, Instruction, InstructionKind, Macro, MacroGraph,
    SPEED_MULTIPLIER_RANGE, migrate, normalize_block_color, placement,
};
use blockstitch_ffi::{Host, arg};
use serde::de::DeserializeOwned;
use serde_json::{Value as Json, json};

/// What undo restores: the app's scope, the canvas plus signals and packs.
type Snapshot = (MacroGraph, Vec<String>, Vec<PackRecord>);

/// Same depth as the app's `UNDO_STACK_LIMIT`.
const UNDO_LIMIT: usize = 50;

/// Blockwork keeps "09" typed into an operator argument as text.
const EDIT_CONTEXT: EditContext = EditContext { numbers: NumberSpelling::KeepTypedInOperands };

enum KeyCapture {
    Strand(String, Vec<PathStep>),
    Standalone,
}

/// The page's document: every macro in the tab, the open one, its history.
pub struct Demo {
    macros: Vec<Macro>,
    selected: Option<usize>,
    history: History<Snapshot>,
    buffers: ValueBuffers,
    key_capture: Option<KeyCapture>,
    standalone_key: Option<String>,
    palette_layout: String,
}

impl Default for Demo {
    fn default() -> Self {
        // The pure text and date reporters; host ones stay unregistered.
        blockwork_core::init();
        Demo {
            macros: Vec::new(),
            selected: None,
            history: History::new(UNDO_LIMIT),
            buffers: ValueBuffers::new(),
            key_capture: None,
            standalone_key: None,
            palette_layout: "rail".into(),
        }
    }
}

/// Every command `dispatch` answers, for the page's contract check.
pub const COMMANDS: &[&str] = &[
    "load", "state", "commands", "preview_value",
    "new_macro", "select_macro_by_id", "duplicate_macro", "delete_macro", "remove_macro", "set_macro_folder",
    "set_title", "set_macro_speed_multiplier", "set_macro_always_listen", "set_macro_catch_up",
    "set_macro_conditions", "set_macro_max_run_secs", "set_macro_max_loop_iterations", "set_palette_layout",
    "add_strand", "remove_strand", "move_strand", "split_strand", "merge_strand", "merge_tail",
    "clear_instructions", "set_recording_target",
    "add_instruction", "edit_instruction", "remove_instruction", "delete_instruction", "reorder_instruction",
    "copy_instructions", "paste_fragment", "paste_instructions", "duplicate_instruction",
    "edit_value_field", "set_value_kind", "take_value", "put_value", "move_value",
    "create_floating_value", "move_floating_value", "remove_floating_value",
    "create_comment", "create_attached_comment", "move_comment", "edit_comment_text", "set_comment_collapsed",
    "remove_comment",
    "create_variable", "rename_variable", "delete_variable",
    "create_signal", "rename_signal", "delete_signal",
    "create_list", "rename_list", "delete_list", "set_list_items", "set_list_editor_state",
    "create_block", "edit_block", "delete_block",
    "undo", "redo",
    "start_key_capture", "start_standalone_key_capture", "key_capture_event", "clear_standalone_key_capture",
];

fn to_json<T: serde::Serialize>(value: T) -> Result<Json, String> {
    serde_json::to_value(value).map_err(|e| e.to_string())
}

/// `args[key]`, or `T::default()` when it's missing or null.
fn opt<T: DeserializeOwned + Default>(args: &Json, key: &str) -> Result<T, String> {
    Ok(arg::<Option<T>>(args, key)?.unwrap_or_default())
}

fn instruction(dto: InstructionDto) -> Result<Instruction, String> {
    dto_to_instruction(&dto).ok_or_else(|| "Unknown instruction type".to_string())
}

/// Commands that only read; everything else saves like the app's `auto_save`.
const READS: &[&str] = &["state", "commands", "preview_value", "copy_instructions", "select_macro_by_id"];

impl Host for Demo {
    fn dispatch(&mut self, command: &str, args: &Json) -> Result<Json, String> {
        let result = self.run(command, args);
        // The app tidies signal names on every save; the page has no disk
        // but keeps the same document.
        if result.is_ok()
            && !READS.contains(&command)
            && let Some(mac) = self.open_mut()
        {
            mac.collect_named_signals();
        }
        result
    }
}

impl Demo {
    fn run(&mut self, command: &str, args: &Json) -> Result<Json, String> {
        let a = args;
        match command {
            "state" => Ok(self.state()),
            "commands" => to_json(COMMANDS),
            "load" => {
                let macros: Vec<Json> = arg(a, "macros")?;
                self.load(macros)?;
                Ok(Json::Null)
            }
            "preview_value" => self.preview(arg(a, "value")?).map(Json::String),

            // Macro library: in memory, one history for whichever is open.
            "new_macro" => {
                self.add_and_select(Macro::new("New Macro".into(), String::new(), vec![]));
                Ok(Json::Null)
            }
            "select_macro_by_id" => {
                let id: String = arg(a, "id")?;
                let index = self.macros.iter().position(|m| m.id == id).ok_or("Macro not found")?;
                self.select(Some(index));
                Ok(Json::Null)
            }
            "duplicate_macro" => {
                let id: String = arg(a, "id")?;
                let mut copy = self.find(&id)?.clone();
                copy.id = uuid::Uuid::new_v4().simple().to_string();
                copy.name = format!("{} copy", copy.name);
                self.add_and_select(copy);
                Ok(Json::Null)
            }
            "delete_macro" => {
                let id: String = arg(a, "id")?;
                let index = self.macros.iter().position(|m| m.id == id).ok_or("Macro not found")?;
                self.delete(index);
                Ok(Json::Null)
            }
            "remove_macro" => {
                if let Some(index) = self.selected {
                    self.delete(index);
                }
                Ok(Json::Null)
            }
            "set_macro_folder" => {
                let (id, folder): (String, String) = (arg(a, "id")?, arg(a, "folder")?);
                let index = self.macros.iter().position(|m| m.id == id).ok_or("Macro not found")?;
                self.macros[index].settings.folder = folder.trim().trim_matches('/').trim().to_string();
                Ok(Json::Null)
            }
            "set_title" => {
                let title: String = arg(a, "title")?;
                if let Some(mac) = self.open_mut() {
                    mac.name = title;
                }
                Ok(Json::Null)
            }
            "set_macro_speed_multiplier" => {
                let multiplier: f64 = arg(a, "multiplier")?;
                if let Some(mac) = self.open_mut() {
                    mac.speed_multiplier = multiplier.clamp(*SPEED_MULTIPLIER_RANGE.start(), *SPEED_MULTIPLIER_RANGE.end());
                }
                Ok(Json::Null)
            }
            "set_macro_always_listen" | "set_macro_catch_up" => {
                let enabled: bool = arg(a, "enabled")?;
                if let Some(mac) = self.open_mut() {
                    if command == "set_macro_always_listen" {
                        mac.settings.always_listen = enabled;
                    } else {
                        mac.settings.catch_up_missed = enabled;
                    }
                }
                Ok(Json::Null)
            }
            "set_macro_conditions" => {
                let window: String = arg(a, "onlyInWindow")?;
                let power: PowerCondition = arg(a, "power")?;
                let (from, until): (String, String) = (arg(a, "activeFrom")?, arg(a, "activeUntil")?);
                let days: u8 = arg(a, "activeDays")?;
                let active_hours = match (from.trim(), until.trim()) {
                    ("", "") => None,
                    ("", _) | (_, "") => return Err("set both a start and an end time, or neither".into()),
                    (from, until) => Some((parse_clock(from)?, parse_clock(until)?)),
                };
                if let Some(mac) = self.open_mut() {
                    mac.settings.only_in_window = window.trim().to_string();
                    mac.settings.power = power;
                    mac.settings.active_hours = active_hours;
                    mac.settings.active_days = days & 0x7f;
                }
                Ok(Json::Null)
            }
            "set_macro_max_run_secs" => {
                let seconds: u32 = arg(a, "seconds")?;
                if let Some(mac) = self.open_mut() {
                    mac.settings.max_run_secs = seconds;
                }
                Ok(Json::Null)
            }
            "set_macro_max_loop_iterations" => {
                let iterations: u32 = arg(a, "iterations")?;
                if let Some(mac) = self.open_mut() {
                    mac.settings.max_loop_iterations = iterations;
                }
                Ok(Json::Null)
            }
            "set_palette_layout" => {
                let layout: String = arg(a, "layout")?;
                self.palette_layout = if layout == "drawer" { layout } else { "rail".into() };
                Ok(Json::Null)
            }

            // Strands
            "add_strand" => {
                let (x, y): (Option<i32>, Option<i32>) = (arg(a, "x")?, arg(a, "y")?);
                let ins = match arg::<Option<InstructionDto>>(a, "instruction")? {
                    Some(dto) => vec![instruction(dto)?],
                    None => vec![],
                };
                self.open()?;
                self.push_undo();
                let mac = self.open_mut().ok_or("No macro selected")?;
                let (dx, dy) = mac.next_editor_position();
                to_json(mac.add_strand(x.unwrap_or(dx), y.unwrap_or(dy), ins))
            }
            "remove_strand" => {
                let strand_id: String = arg(a, "strandId")?;
                self.push_undo();
                if let Some(mac) = self.open_mut() {
                    mac.remove_strand(&strand_id);
                    drop_strand_buffers(&mut self.buffers, &strand_id);
                }
                Ok(Json::Null)
            }
            "move_strand" => {
                let (strand_id, x, y): (String, i32, i32) = (arg(a, "strandId")?, arg(a, "x")?, arg(a, "y")?);
                if let Some(mac) = self.open_mut() {
                    mac.move_strand(&strand_id, x, y);
                }
                Ok(Json::Null)
            }
            "split_strand" => {
                let (strand_id, path): (String, Vec<PathStep>) = (arg(a, "strandId")?, arg(a, "path")?);
                let (x, y): (i32, i32) = (arg(a, "x")?, arg(a, "y")?);
                self.open()?;
                self.push_undo();
                let id = self.open_mut().ok_or("No macro selected")?.split_strand(&strand_id, &path, x, y)?;
                self.buffers.clear();
                to_json(id)
            }
            "merge_strand" => {
                let (dragged, target): (String, String) = (arg(a, "draggedId")?, arg(a, "targetId")?);
                let path: Vec<PathStep> = arg(a, "path")?;
                // Validated on a copy, so a rejected drop costs no undo step.
                let mut graph = self.open()?.graph.clone();
                graph.merge_strand(&dragged, &target, &path)?;
                self.push_undo();
                self.open_mut().ok_or("No macro selected")?.graph = graph;
                drop_strand_buffers(&mut self.buffers, &dragged);
                Ok(Json::Null)
            }
            "merge_tail" => {
                let (strand_id, path): (String, Vec<PathStep>) = (arg(a, "strandId")?, arg(a, "path")?);
                let (target_id, target_path): (String, Vec<PathStep>) = (arg(a, "targetId")?, arg(a, "targetPath")?);
                let mut graph = self.open()?.graph.clone();
                graph.merge_tail(&strand_id, &path, &target_id, &target_path)?;
                self.push_undo();
                self.open_mut().ok_or("No macro selected")?.graph = graph;
                self.buffers.clear();
                Ok(Json::Null)
            }
            "clear_instructions" => {
                self.push_undo();
                if let Some(mac) = self.open_mut() {
                    mac.clear_editor();
                    self.buffers.clear();
                }
                Ok(Json::Null)
            }
            "set_recording_target" => {
                let strand_id: String = arg(a, "strandId")?;
                if let Some(mac) = self.open_mut()
                    && mac.strand(&strand_id).is_some()
                {
                    mac.recording_target = Some(strand_id);
                }
                Ok(Json::Null)
            }

            // Instructions
            "add_instruction" => {
                let (strand_id, path): (String, Vec<PathStep>) = (arg(a, "strandId")?, arg(a, "path")?);
                let ins = instruction(arg(a, "instruction")?)?;
                let mac = self.open()?;
                mac.check_header_placement(&strand_id, &path, &ins)?;
                if let Some(strand) = mac.strand(&strand_id) {
                    placement::check_return(mac, strand, &ins)?;
                    placement::check_loop_control(strand, &path, &ins)?;
                }
                self.push_undo();
                if let Some(mac) = self.open_mut()
                    && mac.insert_instruction(&strand_id, &path, ins)?
                {
                    self.buffers.clear();
                }
                Ok(Json::Null)
            }
            "edit_instruction" => {
                let (strand_id, path): (String, Vec<PathStep>) = (arg(a, "strandId")?, arg(a, "path")?);
                let ins = instruction(arg(a, "instruction")?)?;
                // Freeform text coalesces keystrokes into one step, as in the app.
                let session = matches!(
                    &ins.kind,
                    InstructionKind::Command(_) | InstructionKind::CommandWait(_) | InstructionKind::Comment(_)
                )
                .then(|| EditSession::Instruction { strand_id: strand_id.clone(), index: path.clone() });
                self.push_undo_for(session);
                if let Some(mac) = self.open_mut() {
                    mac.replace_instruction(&strand_id, &path, ins);
                }
                Ok(Json::Null)
            }
            "remove_instruction" => {
                let (strand_id, path): (String, Vec<PathStep>) = (arg(a, "strandId")?, arg(a, "path")?);
                let count: Option<usize> = arg(a, "count")?;
                // A path addressing nothing is no step and no error.
                let _ = self.run_edit(EditCommand::RemoveInstructions { strand_id, path, count: count.unwrap_or(1) });
                Ok(Json::Null)
            }
            "delete_instruction" => {
                let (strand_id, path): (String, Vec<PathStep>) = (arg(a, "strandId")?, arg(a, "path")?);
                let (x, y): (i32, i32) = (arg(a, "x")?, arg(a, "y")?);
                let mut graph = self.open()?.graph.clone();
                let Ok(new_id) = graph.delete_instruction(&strand_id, &path, x, y) else {
                    return Ok(Json::Null);
                };
                self.push_undo();
                self.open_mut().ok_or("No macro selected")?.graph = graph;
                self.buffers.clear();
                to_json(new_id)
            }
            "reorder_instruction" => {
                let (strand_id, path): (String, Vec<PathStep>) = (arg(a, "strandId")?, arg(a, "path")?);
                let direction: i32 = arg(a, "direction")?;
                let mut graph = self.open()?.graph.clone();
                if graph.reorder_instruction(&strand_id, &path, direction) {
                    self.push_undo();
                    self.open_mut().ok_or("No macro selected")?.graph = graph;
                    self.buffers.clear();
                }
                Ok(Json::Null)
            }
            "copy_instructions" => {
                let (strand_id, path): (String, Vec<PathStep>) = (arg(a, "strandId")?, arg(a, "path")?);
                let count: Option<usize> = arg(a, "count")?;
                to_json(fragment_to_dto(&self.open()?.copy_fragment(&strand_id, &path, count)?))
            }
            "paste_fragment" | "paste_instructions" => {
                let (x, y): (i32, i32) = (arg(a, "x")?, arg(a, "y")?);
                let fragment: FragmentDto = if command == "paste_fragment" {
                    arg(a, "fragment")?
                } else {
                    FragmentDto { instructions: arg(a, "instructions")?, comments: Vec::new() }
                };
                let fragment = dto_to_fragment(&fragment).ok_or("Unknown instruction type")?;
                let outcome = self.run_edit(EditCommand::PasteFragment { fragment, x, y })?;
                outcome
                    .created
                    .into_iter()
                    .find_map(|c| match c {
                        Created::Strand(id) => Some(Json::String(id)),
                        _ => None,
                    })
                    .ok_or_else(|| "Nothing was pasted".to_string())
            }
            "duplicate_instruction" => {
                let (strand_id, path): (String, Vec<PathStep>) = (arg(a, "strandId")?, arg(a, "path")?);
                self.run_edit(EditCommand::DuplicateInstruction { strand_id, path })?;
                Ok(Json::Null)
            }

            // Values
            "edit_value_field" => {
                let (location, text): (ValueLocation, String) = (arg(a, "location")?, arg(a, "text")?);
                // A location that no longer resolves is quietly ignored, as in the app.
                let _ = self.run_edit(EditCommand::EditFieldText { location, text });
                Ok(Json::Null)
            }
            "move_value" => {
                let from: ValueLocation = arg(a, "from")?;
                let to = match arg::<Option<ValueLocation>>(a, "to")? {
                    Some(to) => ValueDestination::Slot(to),
                    None => ValueDestination::Canvas {
                        x: opt(a, "x")?,
                        y: opt(a, "y")?,
                        origin_block_id: arg(a, "originBlockId")?,
                    },
                };
                self.run_edit(EditCommand::MoveValue { from, to })?;
                Ok(Json::Null)
            }
            "set_value_kind" => {
                let (location, kind): (ValueLocation, String) = (arg(a, "location")?, arg(a, "kind")?);
                self.push_undo();
                let env = self.variable_values();
                let result = match self.open_mut() {
                    Some(mac) => mac.set_value_kind(&location, &kind, &env),
                    None => Ok(()),
                };
                prune_value_buffers(&mut self.buffers, &location);
                result.map(|_| Json::Null)
            }
            "take_value" => {
                let location: ValueLocation = arg(a, "location")?;
                self.push_undo();
                let taken = self.open_mut().and_then(|mac| mac.take_value(&location));
                prune_value_buffers(&mut self.buffers, &location);
                to_json(taken.ok_or("Nothing to take at that location")?)
            }
            "put_value" => {
                let (location, value): (ValueLocation, Value) = (arg(a, "location")?, arg(a, "value")?);
                self.push_undo();
                if let Some(mac) = self.open_mut() {
                    mac.put_value(&location, value);
                }
                prune_value_buffers(&mut self.buffers, &location);
                Ok(Json::Null)
            }
            "create_floating_value" => {
                let (x, y, value): (i32, i32, Value) = (arg(a, "x")?, arg(a, "y")?, arg(a, "value")?);
                let origin: Option<String> = arg(a, "originBlockId")?;
                self.open()?;
                self.push_undo();
                to_json(self.open_mut().ok_or("No macro selected")?.add_floating_value(x, y, value, origin))
            }
            "move_floating_value" => {
                let (id, x, y): (String, i32, i32) = (arg(a, "floatingId")?, arg(a, "x")?, arg(a, "y")?);
                if let Some(mac) = self.open_mut() {
                    mac.move_floating_value(&id, x, y);
                }
                Ok(Json::Null)
            }
            "remove_floating_value" => {
                let id: String = arg(a, "floatingId")?;
                self.push_undo();
                if let Some(mac) = self.open_mut() {
                    mac.remove_floating_value(&id);
                }
                Ok(Json::Null)
            }

            // Comments
            "create_comment" | "create_attached_comment" => {
                let text: String = opt(a, "text")?;
                let (x, y, attached): (i32, i32, Option<String>) = if command == "create_comment" {
                    (arg(a, "x")?, arg(a, "y")?, None)
                } else {
                    (arg(a, "dx")?, arg(a, "dy")?, Some(arg(a, "instructionId")?))
                };
                self.open()?;
                self.push_undo();
                to_json(self.open_mut().ok_or("No macro selected")?.add_comment(x, y, text, attached))
            }
            "move_comment" => {
                let (id, x, y): (String, i32, i32) = (arg(a, "commentId")?, arg(a, "x")?, arg(a, "y")?);
                if let Some(mac) = self.open_mut() {
                    mac.move_comment(&id, x, y);
                }
                Ok(Json::Null)
            }
            "edit_comment_text" => {
                let (id, text): (String, String) = (arg(a, "commentId")?, arg(a, "text")?);
                self.push_undo_for(Some(EditSession::Comment { comment_id: id.clone() }));
                if let Some(mac) = self.open_mut() {
                    mac.set_comment_text(&id, text);
                }
                Ok(Json::Null)
            }
            "set_comment_collapsed" => {
                let (id, collapsed): (String, bool) = (arg(a, "commentId")?, arg(a, "collapsed")?);
                if let Some(mac) = self.open_mut() {
                    mac.set_comment_collapsed(&id, collapsed);
                }
                Ok(Json::Null)
            }
            "remove_comment" => {
                let id: String = arg(a, "commentId")?;
                self.push_undo();
                if let Some(mac) = self.open_mut() {
                    mac.remove_comment(&id);
                }
                Ok(Json::Null)
            }

            // Variables, signals and lists
            "create_variable" => {
                let name: String = arg(a, "name")?;
                self.open_mut().ok_or("No macro selected")?.create_variable(&name)?;
                Ok(Json::Null)
            }
            "rename_variable" => {
                let (old, new): (String, String) = (arg(a, "oldName")?, arg(a, "newName")?);
                let mut graph = self.open()?.graph.clone();
                if graph.rename_variable(&old, &new)? != old {
                    self.push_undo();
                    self.open_mut().ok_or("No macro selected")?.graph = graph;
                }
                Ok(Json::Null)
            }
            "delete_variable" => {
                let name: String = arg(a, "name")?;
                self.open_mut().ok_or("No macro selected")?.remove_variable(&name);
                Ok(Json::Null)
            }
            "create_signal" => {
                let name: String = arg(a, "name")?;
                let name = self.open()?.validate_signal_name(&name, None)?;
                self.push_undo();
                self.open_mut().ok_or("No macro selected")?.signals.push(name);
                Ok(Json::Null)
            }
            "rename_signal" => {
                let (old, new): (String, String) = (arg(a, "oldName")?, arg(a, "newName")?);
                let mac = self.open()?;
                let name = mac.validate_signal_name(&new, Some(&old))?;
                if !mac.signals.contains(&old) {
                    return Err("Signal not found".into());
                }
                if name != old {
                    self.push_undo();
                    self.open_mut().ok_or("No macro selected")?.rename_signal(&old, &name)?;
                }
                Ok(Json::Null)
            }
            "delete_signal" => {
                let name: String = arg(a, "name")?;
                let mac = self.open()?;
                if !mac.signals.contains(&name) {
                    return Err("Signal not found".into());
                }
                let mut used = false;
                mac.graph.walk_instructions(&mut |ins| {
                    if let InstructionKind::WhenSignal(target) | InstructionKind::SendSignal { name: target, .. } = &ins.kind
                        && !target.expression
                        && let Value::Text { value } = &target.value
                    {
                        used |= value.trim().eq_ignore_ascii_case(&name);
                    }
                });
                if used {
                    return Err("This signal is still used by a named signal block. Change or remove those blocks first.".into());
                }
                self.push_undo();
                self.open_mut().ok_or("No macro selected")?.signals.retain(|s| s != &name);
                Ok(Json::Null)
            }
            "create_list" => {
                let name: String = arg(a, "name")?;
                let mac = self.open()?;
                let trimmed = name.trim();
                if trimmed.is_empty() {
                    return Err("List name can't be empty".into());
                }
                if mac.lists.iter().any(|list| list.name == trimmed) {
                    return Err(format!("A list named \"{trimmed}\" already exists"));
                }
                self.push_undo();
                let mac = self.open_mut().ok_or("No macro selected")?;
                let trimmed = mac.graph.create_list(&name)?;
                if let Some(list) = mac.graph.lists.iter_mut().find(|list| list.name == trimmed) {
                    list.editor_x = 36;
                    list.editor_y = 36;
                }
                Ok(Json::Null)
            }
            "rename_list" => {
                let (old, new): (String, String) = (arg(a, "oldName")?, arg(a, "newName")?);
                let trimmed = new.trim().to_string();
                let mac = self.open()?;
                if trimmed.is_empty() {
                    return Err("List name can't be empty".into());
                }
                if trimmed != old && mac.lists.iter().any(|list| list.name == trimmed) {
                    return Err(format!("A list named \"{trimmed}\" already exists"));
                }
                if !mac.lists.iter().any(|list| list.name == old) {
                    return Err("List not found".into());
                }
                if trimmed != old {
                    self.push_undo();
                    self.open_mut().ok_or("No macro selected")?.graph.rename_list(&old, &trimmed)?;
                }
                Ok(Json::Null)
            }
            "delete_list" => {
                let name: String = arg(a, "name")?;
                if self.open()?.lists.iter().any(|list| list.name == name) {
                    self.push_undo();
                    self.open_mut().ok_or("No macro selected")?.graph.remove_list(&name);
                }
                Ok(Json::Null)
            }
            "set_list_items" => {
                let (name, items): (String, Vec<ListItemDto>) = (arg(a, "name")?, arg(a, "items")?);
                let items: Vec<ListItem> = items.iter().map(dto_to_list_item).collect();
                self.open_mut().ok_or("No macro selected")?.graph.set_list_items(&name, items)?;
                Ok(Json::Null)
            }
            "set_list_editor_state" => {
                let (name, visible): (String, bool) = (arg(a, "name")?, arg(a, "visible")?);
                let (x, y): (i32, i32) = (arg(a, "x")?, arg(a, "y")?);
                self.open_mut().ok_or("No macro selected")?.graph.set_list_editor_state(&name, visible, x, y)?;
                Ok(Json::Null)
            }

            // Custom blocks
            "create_block" => {
                let pieces: Vec<BlockPiece> = arg(a, "pieces")?;
                let shape: BlockShape = arg(a, "shape")?;
                let color = block_color(&arg::<String>(a, "color")?)?;
                let (icon, identifier, description): (String, String, String) =
                    (opt(a, "icon")?, opt(a, "identifier")?, opt(a, "description")?);
                let position = arg::<Option<i32>>(a, "x")?.zip(arg::<Option<i32>>(a, "y")?);
                BlockDef::validate_pieces(&pieces)?;
                block_packs::check_identifier(&identifier)?;
                self.open()?;
                self.push_undo();
                let mac = self.open_mut().ok_or("No macro selected")?;
                let (x, y) = position.unwrap_or_else(|| mac.next_editor_position());
                let id = mac.create_block(pieces, shape, color, x, y);
                mac.set_block_details(&id, &identifier, &description, &icon)?;
                to_json(id)
            }
            "edit_block" => {
                let block_id: String = arg(a, "blockId")?;
                let pieces: Vec<BlockPiece> = arg(a, "pieces")?;
                let shape: BlockShape = arg(a, "shape")?;
                let color: String = arg(a, "color")?;
                let (icon, identifier, description): (String, String, String) =
                    (opt(a, "icon")?, opt(a, "identifier")?, opt(a, "description")?);
                block_packs::check_editable(self.open()?, &block_id)?;
                BlockDef::validate_pieces(&pieces)?;
                block_packs::check_identifier(&identifier)?;
                block_color(&color)?;
                self.push_undo();
                let mac = self.open_mut().ok_or("No macro selected")?;
                rename_branch_bodies(mac, &block_id, &pieces);
                mac.update_block(&block_id, pieces, shape, &color)?;
                mac.set_block_details(&block_id, &identifier, &description, &icon)?;
                Ok(Json::Null)
            }
            "delete_block" => {
                let block_id: String = arg(a, "blockId")?;
                block_packs::check_editable(self.open()?, &block_id)?;
                self.push_undo();
                if let Some(mac) = self.open_mut() {
                    mac.remove_block(&block_id);
                    let surviving: Vec<String> = mac.strands.iter().map(|st| st.id.clone()).collect();
                    retain_live_buffers(&mut self.buffers, &surviving);
                }
                Ok(Json::Null)
            }

            "undo" | "redo" => {
                let step = if command == "undo" { History::undo } else { History::redo };
                let Some(index) = self.selected else { return Ok(Json::Null) };
                let mac = &mut self.macros[index];
                let current = (mac.graph.clone(), mac.signals.clone(), mac.settings.block_packs.clone());
                if let Some(restored) = step(&mut self.history, current) {
                    (mac.graph, mac.signals, mac.settings.block_packs) = restored;
                    mac.ensure_id();
                    self.buffers.clear();
                }
                Ok(Json::Null)
            }

            // Key capture: the page's key events, named as the app names them.
            "start_key_capture" => {
                self.key_capture = Some(KeyCapture::Strand(arg(a, "strandId")?, arg(a, "path")?));
                Ok(Json::Null)
            }
            "start_standalone_key_capture" => {
                self.key_capture = Some(KeyCapture::Standalone);
                Ok(Json::Null)
            }
            "clear_standalone_key_capture" => {
                self.standalone_key = None;
                Ok(Json::Null)
            }
            "key_capture_event" => {
                let keys: Vec<(String, String)> = arg(a, "keys")?;
                self.key_capture_event(&keys);
                Ok(Json::Null)
            }

            other => Err(format!("unsupported command `{other}`")),
        }
    }
}

/// The app's color rule: a six-digit hex color.
fn block_color(color: &str) -> Result<String, String> {
    normalize_block_color(color).ok_or_else(|| "Choose a valid block color".to_string())
}

/// Keeps `RunBranch` callbacks working when a branch piece is renamed.
fn rename_branch_bodies(mac: &mut Macro, block_id: &str, pieces: &[BlockPiece]) {
    let Some(def) = mac.block_defs.iter().find(|b| b.id == block_id) else { return };
    let old = def.pieces.clone();
    let renames: Vec<(String, String)> = pieces
        .iter()
        .filter_map(|piece| {
            let BlockPiece::Branch { id, name } = piece else { return None };
            let old_name = old.iter().find_map(|p| match p {
                BlockPiece::Branch { id: old_id, name } if old_id == id => Some(name),
                _ => None,
            })?;
            (old_name != name).then(|| (old_name.clone(), name.clone()))
        })
        .collect();
    for (old_name, new_name) in &renames {
        mac.rename_block_branch_body(block_id, old_name, new_name);
    }
}

/// Whether `value` uses a reporter nothing on the page can answer (a host
/// reporter such as the battery or the clipboard).
fn needs_app(value: &Value) -> bool {
    match value {
        Value::Op { op, args, .. } => {
            // Not the registry: a native build with `system` unified in has
            // the machine's reporters registered too.
            matches!(op, Op::Ext(name) if !is_pure_operator(name)) || args.iter().any(needs_app)
        }
        Value::Call { args, .. } => args.iter().any(needs_app),
        _ => false,
    }
}

impl Demo {
    fn open(&self) -> Result<&Macro, String> {
        self.selected.and_then(|i| self.macros.get(i)).ok_or_else(|| "No macro selected".to_string())
    }

    fn open_mut(&mut self) -> Option<&mut Macro> {
        self.selected.and_then(|i| self.macros.get_mut(i))
    }

    fn find(&self, id: &str) -> Result<&Macro, String> {
        self.macros.iter().find(|m| m.id == id).ok_or_else(|| "Macro not found".to_string())
    }

    fn select(&mut self, index: Option<usize>) {
        self.selected = index.filter(|&i| i < self.macros.len());
        self.history.clear();
        self.buffers.clear();
        self.key_capture = None;
    }

    fn add_and_select(&mut self, mac: Macro) {
        self.macros.push(mac);
        self.select(Some(self.macros.len() - 1));
    }

    /// Deleting the open macro leaves nothing open, as in the app.
    fn delete(&mut self, index: usize) {
        self.macros.remove(index);
        match self.selected {
            Some(open) if open == index => self.select(None),
            Some(open) if open > index => self.selected = Some(open - 1),
            _ => {}
        }
    }

    /// Replaces the tab's macros with saved ones (the page's examples) and
    /// opens the first. Saved files go through the app's own migrations.
    fn load(&mut self, saved: Vec<Json>) -> Result<(), String> {
        let mut macros = Vec::with_capacity(saved.len());
        for value in saved {
            let (mut mac, _) = migrate::parse(&value.to_string())?;
            mac.ensure_id();
            macros.push(mac);
        }
        self.macros = macros;
        self.select((!self.macros.is_empty()).then_some(0));
        Ok(())
    }

    fn snapshot(mac: &Macro) -> Snapshot {
        (mac.graph.clone(), mac.signals.clone(), mac.settings.block_packs.clone())
    }

    /// Checkpoints the open macro before a structural edit (`push_undo`).
    fn push_undo(&mut self) {
        self.push_undo_for(None);
    }

    fn push_undo_for(&mut self, session: Option<EditSession>) {
        let snapshot = self.selected.and_then(|i| self.macros.get(i)).map(Self::snapshot);
        match (snapshot, session) {
            (Some(snapshot), Some(session)) => {
                self.history.push_for_session(snapshot, session);
            }
            (Some(snapshot), None) => self.history.push(snapshot),
            (None, _) => self.history.end_session(),
        }
    }

    /// One typed edit: a rejected or no-op edit costs no undo step;
    /// buffers follow the outcome either way.
    fn run_edit(&mut self, command: EditCommand<InstructionKind>) -> Result<EditOutcome, String> {
        let index = self.selected.filter(|&i| i < self.macros.len()).ok_or("No macro selected")?;
        let outcome = blockstitch_ffi::run_edit(&mut self.macros[index], &mut self.history, Self::snapshot, command, &EDIT_CONTEXT)
            .map_err(String::from)?;
        apply_buffer_effects(&mut self.buffers, &outcome.buffers);
        Ok(outcome)
    }

    /// Variables at their declared values, which is what the app's preview
    /// reads while nothing runs.
    fn variable_values(&self) -> HashMap<String, Evaluated> {
        self.open().map(|mac| mac.variables.iter().map(|v| (v.name.clone(), v.value.clone())).collect()).unwrap_or_default()
    }

    /// The app's click-to-preview over the shared evaluator. Host reporters
    /// are not simulated: they say they need the app.
    fn preview(&self, value: Value) -> Result<String, String> {
        if has_side_effect_operator(&value) {
            return Err("Preview is off for blocks that run commands or go online".into());
        }
        if needs_app(&value) {
            return Err("This reporter needs the Blockwork app to show a value".into());
        }
        let lists: HashMap<String, Vec<ListItem>> =
            self.open().map(|mac| mac.lists.iter().map(|l| (l.name.clone(), l.items.clone())).collect()).unwrap_or_default();
        resolve_list_reporters(&value.resolve_vars(&self.variable_values()), &lists)?.eval_text()
    }

    fn key_capture_event(&mut self, keys: &[(String, String)]) {
        let mut combo: Vec<MacroKey> = Vec::new();
        for (code, key) in keys {
            if let Some(mk) = web_code_to_macro_key(code).or_else(|| web_key_to_macro_key(key))
                && !combo.iter().any(|k| match_key(k) == match_key(&mk))
            {
                combo.push(mk);
            }
        }
        if combo.is_empty() {
            return;
        }
        let text = format_key_combo(&combo);
        match self.key_capture.take() {
            Some(KeyCapture::Strand(strand_id, path)) => {
                if let Some(mac) = self.open_mut()
                    && let Some(Instruction {
                        kind: InstructionKind::Token(InputToken::Key(value, _)) | InstructionKind::WhenKey(value, _),
                        ..
                    }) = mac.instruction_at_mut(&strand_id, &path)
                {
                    *value = Value::Text { value: text };
                }
            }
            Some(KeyCapture::Standalone) => self.standalone_key = Some(text),
            None => {}
        }
    }

    /// The app's `StateDto`, with the fields the editor reads; what only the
    /// app has (runs, recording, globals, permissions) stays empty.
    pub fn state(&self) -> Json {
        let open = self.selected.and_then(|i| self.macros.get(i));
        let buffers: Vec<InvalidFieldDto> = self
            .buffers
            .iter()
            .map(|(location, text)| InvalidFieldDto { location: location.clone(), text: text.clone() })
            .collect();
        let key_capture = self.key_capture.as_ref().map(|target| match target {
            KeyCapture::Strand(strand_id, index) => KeyCaptureDto {
                kind: "Strand".into(),
                strand_id: Some(strand_id.clone()),
                index: Some(index.clone()),
            },
            KeyCapture::Standalone => KeyCaptureDto { kind: "Standalone".into(), strand_id: None, index: None },
        });
        let library: Vec<Json> = self
            .macros
            .iter()
            .map(|mac| {
                let has_triggers = mac.strands.iter().any(|st| {
                    st.instructions.first().is_some_and(|i| {
                        i.kind.is_header()
                            && !matches!(i.kind, InstructionKind::WhenRan | InstructionKind::BlockHeader(_) | InstructionKind::WhenSignal(_))
                    })
                });
                json!({ "id": mac.id, "name": mac.name, "folder": mac.settings.folder,
                        "always_listen": mac.settings.always_listen, "has_triggers": has_triggers, "restricted": false })
            })
            .collect();
        json!({
            "macro_names": self.macros.iter().map(|m| &m.name).collect::<Vec<_>>(),
            "macro_selected": self.selected,
            "current_macro": open.map(macro_to_dto),
            "macros_data": [],
            "global_speed_multiplier": 1.0,
            "is_looping": false,
            "page": "Main",
            "onboarding_done": true,
            "palette_layout": self.palette_layout,
            "key_capture": key_capture,
            "standalone_key": self.standalone_key,
            "combo_capture": null,
            "can_undo": self.history.can_undo(),
            "can_redo": self.history.can_redo(),
            "recording_phase": { "phase": "Idle", "countdown": null },
            "recording_countdown_secs": 3,
            "record_mouse_relative": true,
            "record_mouse_movement": false,
            "absolute_mouse_position_available": true,
            "invalid_field_buffers": buffers,
            "triggers_paused": false,
            "run_log": [],
            "templates": [],
            "secret_names": [],
            "globals": [],
            "hotkey_bindings": [],
            "permissions": null,
            "breakpoints": [],
            "paused_macros": [],
            "paused_strands": [],
            "library": library,
            "language": "",
        })
    }
}
