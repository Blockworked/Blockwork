//! Maps a command name plus its JSON arguments onto the matching function in
//! `commands.rs`. Argument names are the camelCase spelling the frontend
//! already used with Tauri's `invoke`, so `ui/src/tauri.ts` is unchanged.

use crate::Backend;
use crate::commands;
use crate::state::{BlockPiece, HotkeyActionDto, InstructionDto, PathStep, ValueLocation};
use blockwork_core::input::value::Value as BlockValue;
use blockwork_core::macros::BlockShape;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::collections::HashMap;

fn arg<T: DeserializeOwned>(args: &Value, name: &str) -> Result<T, String> {
    let value = args.get(name).cloned().unwrap_or(Value::Null);
    serde_json::from_value(value).map_err(|e| format!("invalid argument '{name}': {e}"))
}

fn to_json<T: serde::Serialize>(value: T) -> Result<Value, String> {
    serde_json::to_value(value).map_err(|e| e.to_string())
}

impl Backend {
    /// Runs the command named `cmd` with `args` (a JSON object keyed by
    /// camelCase argument name) and returns its result as JSON.
    pub async fn dispatch(&self, cmd: &str, args: Value) -> Result<Value, String> {
        match cmd {
            "get_state" => to_json(commands::get_state(&self.state)?),
            "select_macro" => {
                let index: usize = arg(&args, "index")?;
                to_json(commands::select_macro(&self.state, &self.app, index)?)
            }
            "new_macro" => to_json(commands::new_macro(&self.state, &self.app)?),
            "remove_macro" => to_json(commands::remove_macro(&self.state, &self.app)?),
            "set_title" => {
                let title: String = arg(&args, "title")?;
                to_json(commands::set_title(&self.state, &self.app, title)?)
            }
            "set_macro_speed_multiplier" => {
                let multiplier: f64 = arg(&args, "multiplier")?;
                to_json(commands::set_macro_speed_multiplier(
                    &self.state,
                    &self.app,
                    multiplier,
                )?)
            }
            "set_macro_always_listen" => {
                let enabled: bool = arg(&args, "enabled")?;
                to_json(commands::set_macro_always_listen(
                    &self.state,
                    &self.app,
                    enabled,
                )?)
            }
            "save_macro" => to_json(commands::save_macro(&self.state, &self.app)?),
            "export_macro_file_name" => {
                let macro_id: String = arg(&args, "macroId")?;
                to_json(commands::export_macro_file_name(macro_id)?)
            }
            "export_macro" => {
                let macro_id: String = arg(&args, "macroId")?;
                let path: String = arg(&args, "path")?;
                to_json(commands::export_macro(macro_id, path)?)
            }
            "import_macro" => {
                let path: String = arg(&args, "path")?;
                to_json(commands::import_macro(&self.state, &self.app, path)?)
            }
            "confirm_import_macro" => {
                let keep_settings: HashMap<String, bool> = arg(&args, "keepSettings")?;
                to_json(
                    commands::confirm_import_macro(&self.state, &self.app, keep_settings).await?,
                )
            }
            "cancel_import_macro" => to_json(commands::cancel_import_macro(&self.state)?),
            "add_instruction" => {
                let strand_id: String = arg(&args, "strandId")?;
                let path: Vec<PathStep> = arg(&args, "path")?;
                let instruction: InstructionDto = arg(&args, "instruction")?;
                to_json(
                    commands::add_instruction(&self.state, &self.app, strand_id, path, instruction)
                        .await?,
                )
            }
            "edit_instruction" => {
                let strand_id: String = arg(&args, "strandId")?;
                let path: Vec<PathStep> = arg(&args, "path")?;
                let instruction: InstructionDto = arg(&args, "instruction")?;
                to_json(
                    commands::edit_instruction(&self.state, &self.app, strand_id, path, instruction)
                        .await?,
                )
            }
            "create_variable" => {
                let name: String = arg(&args, "name")?;
                to_json(commands::create_variable(&self.state, &self.app, name)?)
            }
            "rename_variable" => {
                let old_name: String = arg(&args, "oldName")?;
                let new_name: String = arg(&args, "newName")?;
                to_json(commands::rename_variable(
                    &self.state,
                    &self.app,
                    old_name,
                    new_name,
                )?)
            }
            "delete_variable" => {
                let name: String = arg(&args, "name")?;
                to_json(commands::delete_variable(&self.state, &self.app, name)?)
            }
            "create_list" => {
                let name: String = arg(&args, "name")?;
                to_json(commands::create_list(&self.state, &self.app, name)?)
            }
            "rename_list" => {
                let old_name: String = arg(&args, "oldName")?;
                let new_name: String = arg(&args, "newName")?;
                to_json(commands::rename_list(
                    &self.state,
                    &self.app,
                    old_name,
                    new_name,
                )?)
            }
            "delete_list" => {
                let name: String = arg(&args, "name")?;
                to_json(commands::delete_list(&self.state, &self.app, name)?)
            }
            "set_list_items" => {
                let name: String = arg(&args, "name")?;
                let items: Vec<crate::state::ListItemDto> = arg(&args, "items")?;
                to_json(commands::set_list_items(
                    &self.state,
                    &self.app,
                    name,
                    items,
                )?)
            }
            "set_list_editor_state" => {
                let name: String = arg(&args, "name")?;
                let visible: bool = arg(&args, "visible")?;
                let x: i32 = arg(&args, "x")?;
                let y: i32 = arg(&args, "y")?;
                to_json(commands::set_list_editor_state(
                    &self.state,
                    &self.app,
                    name,
                    visible,
                    x,
                    y,
                )?)
            }
            "create_block" => {
                let pieces: Vec<BlockPiece> = arg(&args, "pieces")?;
                let shape: BlockShape = arg(&args, "shape")?;
                let color: String = arg(&args, "color")?;
                to_json(commands::create_block(
                    &self.state,
                    &self.app,
                    pieces,
                    shape,
                    color,
                )?)
            }
            "edit_block" => {
                let block_id: String = arg(&args, "blockId")?;
                let pieces: Vec<BlockPiece> = arg(&args, "pieces")?;
                let shape: BlockShape = arg(&args, "shape")?;
                let color: String = arg(&args, "color")?;
                to_json(commands::edit_block(
                    &self.state,
                    &self.app,
                    block_id,
                    pieces,
                    shape,
                    color,
                )?)
            }
            "delete_block" => {
                let block_id: String = arg(&args, "blockId")?;
                to_json(commands::delete_block(&self.state, &self.app, block_id)?)
            }
            "edit_value_field" => {
                let location: ValueLocation = arg(&args, "location")?;
                let text: String = arg(&args, "text")?;
                to_json(commands::edit_value_field(
                    &self.state,
                    &self.app,
                    location,
                    text,
                )?)
            }
            "set_value_kind" => {
                let location: ValueLocation = arg(&args, "location")?;
                let kind: String = arg(&args, "kind")?;
                to_json(commands::set_value_kind(
                    &self.state,
                    &self.app,
                    location,
                    kind,
                )?)
            }
            "take_value" => {
                let location: ValueLocation = arg(&args, "location")?;
                to_json(commands::take_value(&self.state, &self.app, location)?)
            }
            "put_value" => {
                let location: ValueLocation = arg(&args, "location")?;
                let value: BlockValue = arg(&args, "value")?;
                to_json(commands::put_value(
                    &self.state,
                    &self.app,
                    location,
                    value,
                )?)
            }
            "preview_value" => {
                let value: BlockValue = arg(&args, "value")?;
                to_json(commands::preview_value(&self.state, value)?)
            }
            "create_floating_value" => {
                let x: i32 = arg(&args, "x")?;
                let y: i32 = arg(&args, "y")?;
                let value: BlockValue = arg(&args, "value")?;
                let origin_block_id: Option<String> = arg(&args, "originBlockId")?;
                to_json(commands::create_floating_value(
                    &self.state,
                    &self.app,
                    x,
                    y,
                    value,
                    origin_block_id,
                )?)
            }
            "move_floating_value" => {
                let floating_id: String = arg(&args, "floatingId")?;
                let x: i32 = arg(&args, "x")?;
                let y: i32 = arg(&args, "y")?;
                to_json(commands::move_floating_value(
                    &self.state,
                    &self.app,
                    floating_id,
                    x,
                    y,
                )?)
            }
            "remove_floating_value" => {
                let floating_id: String = arg(&args, "floatingId")?;
                to_json(commands::remove_floating_value(
                    &self.state,
                    &self.app,
                    floating_id,
                )?)
            }
            "create_comment" => {
                let x: i32 = arg(&args, "x")?;
                let y: i32 = arg(&args, "y")?;
                let text: String = arg(&args, "text")?;
                to_json(commands::create_comment(
                    &self.state,
                    &self.app,
                    x,
                    y,
                    text,
                )?)
            }
            "create_attached_comment" => {
                let instruction_id: String = arg(&args, "instructionId")?;
                let dx: i32 = arg(&args, "dx")?;
                let dy: i32 = arg(&args, "dy")?;
                let text: String = arg(&args, "text")?;
                to_json(commands::create_attached_comment(
                    &self.state,
                    &self.app,
                    instruction_id,
                    dx,
                    dy,
                    text,
                )?)
            }
            "move_comment" => {
                let comment_id: String = arg(&args, "commentId")?;
                let x: i32 = arg(&args, "x")?;
                let y: i32 = arg(&args, "y")?;
                to_json(commands::move_comment(
                    &self.state,
                    &self.app,
                    comment_id,
                    x,
                    y,
                )?)
            }
            "remove_comment" => {
                let comment_id: String = arg(&args, "commentId")?;
                to_json(commands::remove_comment(
                    &self.state,
                    &self.app,
                    comment_id,
                )?)
            }
            "edit_comment_text" => {
                let comment_id: String = arg(&args, "commentId")?;
                let text: String = arg(&args, "text")?;
                to_json(commands::edit_comment_text(
                    &self.state,
                    &self.app,
                    comment_id,
                    text,
                )?)
            }
            "set_comment_collapsed" => {
                let comment_id: String = arg(&args, "commentId")?;
                let collapsed: bool = arg(&args, "collapsed")?;
                to_json(commands::set_comment_collapsed(
                    &self.state,
                    &self.app,
                    comment_id,
                    collapsed,
                )?)
            }
            "remove_instruction" => {
                let strand_id: String = arg(&args, "strandId")?;
                let path: Vec<PathStep> = arg(&args, "path")?;
                to_json(commands::remove_instruction(
                    &self.state,
                    &self.app,
                    strand_id,
                    path,
                )?)
            }
            "reorder_instruction" => {
                let strand_id: String = arg(&args, "strandId")?;
                let path: Vec<PathStep> = arg(&args, "path")?;
                let direction: i32 = arg(&args, "direction")?;
                to_json(commands::reorder_instruction(
                    &self.state,
                    &self.app,
                    strand_id,
                    path,
                    direction,
                )?)
            }
            "clear_instructions" => to_json(commands::clear_instructions(&self.state, &self.app)?),
            "add_strand" => {
                let x: Option<i32> = arg(&args, "x")?;
                let y: Option<i32> = arg(&args, "y")?;
                let instruction: Option<InstructionDto> = arg(&args, "instruction")?;
                to_json(commands::add_strand(
                    &self.state,
                    &self.app,
                    x,
                    y,
                    instruction,
                )?)
            }
            "remove_strand" => {
                let strand_id: String = arg(&args, "strandId")?;
                to_json(commands::remove_strand(&self.state, &self.app, strand_id)?)
            }
            "move_strand" => {
                let strand_id: String = arg(&args, "strandId")?;
                let x: i32 = arg(&args, "x")?;
                let y: i32 = arg(&args, "y")?;
                to_json(commands::move_strand(
                    &self.state,
                    &self.app,
                    strand_id,
                    x,
                    y,
                )?)
            }
            "split_strand" => {
                let strand_id: String = arg(&args, "strandId")?;
                let path: Vec<PathStep> = arg(&args, "path")?;
                let x: i32 = arg(&args, "x")?;
                let y: i32 = arg(&args, "y")?;
                to_json(commands::split_strand(
                    &self.state,
                    &self.app,
                    strand_id,
                    path,
                    x,
                    y,
                )?)
            }
            "merge_strand" => {
                let dragged_id: String = arg(&args, "draggedId")?;
                let target_id: String = arg(&args, "targetId")?;
                let path: Vec<PathStep> = arg(&args, "path")?;
                to_json(commands::merge_strand(
                    &self.state,
                    &self.app,
                    dragged_id,
                    target_id,
                    path,
                )?)
            }
            "delete_instruction" => {
                let strand_id: String = arg(&args, "strandId")?;
                let path: Vec<PathStep> = arg(&args, "path")?;
                let x: i32 = arg(&args, "x")?;
                let y: i32 = arg(&args, "y")?;
                to_json(commands::delete_instruction(
                    &self.state,
                    &self.app,
                    strand_id,
                    path,
                    x,
                    y,
                )?)
            }
            "paste_instructions" => {
                let x: i32 = arg(&args, "x")?;
                let y: i32 = arg(&args, "y")?;
                let instructions: Vec<InstructionDto> = arg(&args, "instructions")?;
                to_json(commands::paste_instructions(
                    &self.state,
                    &self.app,
                    x,
                    y,
                    instructions,
                )?)
            }
            "set_recording_target" => {
                let strand_id: String = arg(&args, "strandId")?;
                to_json(commands::set_recording_target(
                    &self.state,
                    &self.app,
                    strand_id,
                )?)
            }
            "undo" => to_json(commands::undo(&self.state, &self.app)?),
            "redo" => to_json(commands::redo(&self.state, &self.app)?),
            "start_key_capture" => {
                let strand_id: String = arg(&args, "strandId")?;
                let path: Vec<PathStep> = arg(&args, "path")?;
                to_json(commands::start_key_capture(
                    &self.state,
                    &self.app,
                    strand_id,
                    path,
                )?)
            }
            "start_standalone_key_capture" => to_json(commands::start_standalone_key_capture(
                &self.state,
                &self.app,
            )?),
            "key_capture_event" => {
                let code: String = arg(&args, "code")?;
                let key: String = arg(&args, "key")?;
                to_json(commands::key_capture_event(
                    &self.state,
                    &self.app,
                    code,
                    key,
                )?)
            }
            "clear_standalone_key_capture" => to_json(commands::clear_standalone_key_capture(
                &self.state,
                &self.app,
            )?),
            "run_macro" => to_json(commands::run_macro(&self.state, &self.app)?),
            "toggle_loop_mode" => {
                let enabled: bool = arg(&args, "enabled")?;
                to_json(commands::toggle_loop_mode(&self.state, &self.app, enabled)?)
            }
            "set_global_speed_multiplier" => {
                let multiplier: f64 = arg(&args, "multiplier")?;
                to_json(commands::set_global_speed_multiplier(
                    &self.state,
                    &self.app,
                    multiplier,
                )?)
            }
            "start_recording" => to_json(commands::start_recording(&self.state, &self.app)?),
            "stop_recording" => to_json(commands::stop_recording(&self.state, &self.app)?),
            "request_absolute_mouse_support" => to_json(
                commands::request_absolute_mouse_support(&self.state, &self.app).await?,
            ),
            "toggle_record_mouse_relative" => {
                let relative: bool = arg(&args, "relative")?;
                to_json(
                    commands::toggle_record_mouse_relative(&self.state, &self.app, relative)
                        .await?,
                )
            }
            "toggle_record_mouse_movement" => {
                let enabled: bool = arg(&args, "enabled")?;
                to_json(commands::toggle_record_mouse_movement(
                    &self.state,
                    &self.app,
                    enabled,
                )?)
            }
            "open_settings" => to_json(commands::open_settings(&self.state, &self.app)?),
            "close_settings" => to_json(commands::close_settings(&self.state, &self.app)?),
            "start_combo_capture" => {
                let action: HotkeyActionDto = arg(&args, "action")?;
                to_json(commands::start_combo_capture(
                    &self.state,
                    &self.app,
                    action,
                )?)
            }
            "start_pending_combo_capture" => to_json(commands::start_pending_combo_capture(
                &self.state,
                &self.app,
            )?),
            "combo_capture_event" => {
                let code: String = arg(&args, "code")?;
                let modifiers: u8 = arg(&args, "modifiers")?;
                to_json(commands::combo_capture_event(
                    &self.state,
                    &self.app,
                    code,
                    modifiers,
                )?)
            }
            "cancel_combo_capture" => {
                to_json(commands::cancel_combo_capture(&self.state, &self.app)?)
            }
            "set_pending_macro_idx" => {
                let index: Option<usize> = arg(&args, "index")?;
                to_json(commands::set_pending_macro_idx(
                    &self.state,
                    &self.app,
                    index,
                )?)
            }
            "add_macro_hotkey" => to_json(commands::add_macro_hotkey(&self.state, &self.app)?),
            "remove_hotkey_binding" => {
                let index: usize = arg(&args, "index")?;
                to_json(commands::remove_hotkey_binding(
                    &self.state,
                    &self.app,
                    index,
                )?)
            }
            "clear_named_hotkey" => {
                let action: HotkeyActionDto = arg(&args, "action")?;
                to_json(commands::clear_named_hotkey(
                    &self.state,
                    &self.app,
                    action,
                )?)
            }
            "reset_hotkey_to_default" => {
                let action: HotkeyActionDto = arg(&args, "action")?;
                to_json(commands::reset_hotkey_to_default(
                    &self.state,
                    &self.app,
                    action,
                )?)
            }
            "set_ipc_port_text" => {
                let text: String = arg(&args, "text")?;
                to_json(commands::set_ipc_port_text(&self.state, &self.app, text)?)
            }
            "start_ipc_server" => {
                to_json(commands::start_ipc_server(&self.state, &self.app).await?)
            }
            "stop_ipc_server" => to_json(commands::stop_ipc_server(&self.state, &self.app)?),
            "set_ipc_auto_start" => {
                let enabled: bool = arg(&args, "enabled")?;
                to_json(commands::set_ipc_auto_start(
                    &self.state,
                    &self.app,
                    enabled,
                )?)
            }
            "set_close_to_tray" => {
                let enabled: bool = arg(&args, "enabled")?;
                to_json(commands::set_close_to_tray(
                    &self.state,
                    &self.app,
                    enabled,
                )?)
            }
            "check_for_updates" => to_json(commands::check_for_updates(&self.state, &self.app)?),
            "apply_update" => to_json(commands::apply_update(&self.state, &self.app)?),
            "list_installed_apps" => to_json(commands::list_installed_apps().await),
            other => Err(format!("unknown command: {other}")),
        }
    }
}
