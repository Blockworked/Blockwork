//! The single source of truth for every agent tool: wire name, MCP name,
//! CLI name, description, and JSON schema. The CLI, the MCP server and the
//! daemon allowlist are all generated (or parity-tested) from this table.

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct ToolDef {
    /// Daemon wire name, e.g. `agent_list_macros`.
    pub wire: &'static str,
    /// MCP tool name (wire without `agent_` prefix).
    pub mcp: &'static str,
    /// CLI subcommand (kebab-case).
    pub cli: &'static str,
    pub description: &'static str,
    /// JSON schema for the args object.
    pub schema: serde_json::Value,
}

macro_rules! tool {
    ($wire:expr, $mcp:expr, $cli:expr, $desc:expr, $schema:expr) => {
        ToolDef {
            wire: $wire,
            mcp: $mcp,
            cli: $cli,
            description: $desc,
            schema: $schema,
        }
    };
}

use serde_json::json;

fn obj(fields: &[(&str, &str)]) -> serde_json::Value {
    let mut props = serde_json::Map::new();
    let mut required = Vec::new();
    for (name, typ) in fields {
        props.insert(name.to_string(), json!({"type": typ}));
        required.push(name.to_string());
    }
    json!({"type": "object", "properties": props, "required": required})
}

use std::sync::LazyLock;

fn table() -> Vec<ToolDef> {
    vec![
        tool!(
            "agent_list_macros",
            "list_macros",
            "list-macros",
            "List all macros",
            obj(&[])
        ),
        tool!(
            "agent_get_macro",
            "get_macro",
            "get-macro",
            "Get one macro (MacroDto + revision)",
            obj(&[("id", "string")])
        ),
        tool!(
            "agent_create_macro",
            "create_macro",
            "create-macro",
            "Create a macro (starts restricted)",
            obj(&[("name", "string")])
        ),
        tool!(
            "agent_create_from_template",
            "create_from_template",
            "create-from-template",
            "Create a macro from a template",
            obj(&[("template", "string")])
        ),
        tool!(
            "agent_duplicate_macro",
            "duplicate_macro",
            "duplicate-macro",
            "Duplicate a macro",
            obj(&[("id", "string")])
        ),
        tool!(
            "agent_delete_macro",
            "delete_macro",
            "delete-macro",
            "Delete a macro",
            obj(&[("id", "string")])
        ),
        tool!(
            "agent_rename_macro",
            "rename_macro",
            "rename-macro",
            "Rename a macro by id",
            obj(&[("id", "string"), ("name", "string")])
        ),
        tool!(
            "agent_get_macro_settings",
            "get_macro_settings",
            "get-macro-settings",
            "Get a macro's settings",
            obj(&[("id", "string")])
        ),
        tool!(
            "agent_update_macro_settings",
            "update_macro_settings",
            "update-macro-settings",
            "Update a macro's settings by id",
            obj(&[("id", "string"), ("settings", "object")])
        ),
        tool!(
            "agent_get_global_settings",
            "get_global_settings",
            "get-global-settings",
            "Get global settings",
            obj(&[])
        ),
        tool!(
            "agent_set_global_settings",
            "set_global_settings",
            "set-global-settings",
            "Set global settings (incl. agent gates)",
            obj(&[])
        ),
        tool!(
            "agent_get_hotkeys",
            "get_hotkeys",
            "get-hotkeys",
            "Get hotkey bindings",
            obj(&[])
        ),
        tool!(
            "agent_set_hotkeys",
            "set_hotkeys",
            "set-hotkeys",
            "Set hotkey bindings",
            obj(&[])
        ),
        tool!(
            "agent_list_templates",
            "list_templates",
            "list-templates",
            "List starter templates",
            obj(&[])
        ),
        tool!(
            "agent_describe_blocks",
            "describe_blocks",
            "describe-blocks",
            "Describe every block type",
            obj(&[])
        ),
        tool!(
            "agent_export_macro",
            "export_macro",
            "export-macro",
            "Export a macro file",
            obj(&[("id", "string"), ("path", "string")])
        ),
        tool!(
            "agent_import_macro",
            "import_macro",
            "import-macro",
            "Stage a macro import",
            obj(&[("path", "string")])
        ),
        tool!(
            "agent_confirm_import",
            "confirm_import",
            "confirm-import",
            "Confirm a staged import",
            obj(&[])
        ),
        tool!(
            "agent_add_strand",
            "add_strand",
            "add-strand",
            "Add a strand to a macro",
            obj(&[("id", "string")])
        ),
        tool!(
            "agent_remove_strand",
            "remove_strand",
            "remove-strand",
            "Remove a strand",
            obj(&[("id", "string"), ("strand_id", "string")])
        ),
        tool!(
            "agent_add_block",
            "add_block",
            "add-block",
            "Add a block (InstructionDto)",
            obj(&[("id", "string"), ("instruction", "object")])
        ),
        tool!(
            "agent_edit_block",
            "edit_block",
            "edit-block",
            "Edit a block by id",
            obj(&[("id", "string"), ("block_id", "string")])
        ),
        tool!(
            "agent_remove_block",
            "remove_block",
            "remove-block",
            "Remove a block by id",
            obj(&[("id", "string"), ("block_id", "string")])
        ),
        tool!(
            "agent_move_block",
            "move_block",
            "move-block",
            "Move a block to another strand",
            obj(&[
                ("id", "string"),
                ("block_id", "string"),
                ("to_strand", "string")
            ])
        ),
        tool!(
            "agent_find_blocks",
            "find_blocks",
            "find-blocks",
            "Find blocks by type/text",
            obj(&[("id", "string")])
        ),
        tool!(
            "agent_add_comment",
            "add_comment",
            "add-comment",
            "Add a comment",
            obj(&[("id", "string"), ("text", "string")])
        ),
        tool!(
            "agent_remove_comment",
            "remove_comment",
            "remove-comment",
            "Remove a comment",
            obj(&[("id", "string"), ("comment_id", "string")])
        ),
        tool!(
            "agent_create_variable",
            "create_variable",
            "create-variable",
            "Create a variable",
            obj(&[("id", "string"), ("variable", "string")])
        ),
        tool!(
            "agent_rename_variable",
            "rename_variable",
            "rename-variable",
            "Rename a variable",
            obj(&[("id", "string"), ("old", "string"), ("new", "string")])
        ),
        tool!(
            "agent_delete_variable",
            "delete_variable",
            "delete-variable",
            "Delete a variable",
            obj(&[("id", "string"), ("variable", "string")])
        ),
        tool!(
            "agent_set_variable_value",
            "set_variable_value",
            "set-variable-value",
            "Set a variable value",
            obj(&[
                ("id", "string"),
                ("variable", "string"),
                ("value", "string")
            ])
        ),
        tool!(
            "agent_set_list_items",
            "set_list_items",
            "set-list-items",
            "Set list items",
            obj(&[("id", "string"), ("name", "string"), ("items", "array")])
        ),
        tool!(
            "agent_create_list",
            "create_list",
            "create-list",
            "Create a list",
            obj(&[("id", "string"), ("name", "string")])
        ),
        tool!(
            "agent_rename_list",
            "rename_list",
            "rename-list",
            "Rename a list",
            obj(&[("id", "string"), ("old", "string"), ("new", "string")])
        ),
        tool!(
            "agent_delete_list",
            "delete_list",
            "delete-list",
            "Delete a list",
            obj(&[("id", "string"), ("name", "string")])
        ),
        tool!(
            "agent_replace_canvas",
            "replace_canvas",
            "replace-canvas",
            "Replace a macro's strands (InstructionDto[])",
            obj(&[("id", "string"), ("strands", "array")])
        ),
        tool!(
            "agent_undo",
            "undo",
            "undo",
            "Undo last agent edit (per-macro history)",
            obj(&[("id", "string")])
        ),
        tool!(
            "agent_redo",
            "redo",
            "redo",
            "Redo last agent edit",
            obj(&[("id", "string")])
        ),
        tool!(
            "agent_run_macro",
            "run_macro",
            "run-macro",
            "Run a macro by id",
            obj(&[("id", "string")])
        ),
        tool!(
            "agent_stop_macro",
            "stop_macro",
            "stop-macro",
            "Stop runs of a macro",
            obj(&[("id", "string")])
        ),
        tool!(
            "agent_stop_all",
            "stop_all",
            "stop-all",
            "Stop all running macros",
            obj(&[])
        ),
        tool!(
            "agent_run_status",
            "run_status",
            "run-status",
            "Which macros are running",
            obj(&[])
        ),
        tool!(
            "agent_run_log",
            "run_log",
            "run-log",
            "Recent run log",
            obj(&[])
        ),
        tool!(
            "agent_trace",
            "trace",
            "trace",
            "Active blocks trace",
            obj(&[])
        ),
        tool!(
            "agent_emergency_stop",
            "emergency_stop",
            "emergency-stop",
            "Emergency stop all macros",
            obj(&[])
        ),
        tool!(
            "agent_resume_triggers",
            "resume_triggers",
            "resume-triggers",
            "Resume triggers after emergency stop",
            obj(&[])
        ),
        tool!(
            "agent_screenshot",
            "screenshot",
            "screenshot",
            "Screenshot to temp PNG (path only + metadata; read the file with native image reading to see it)",
            json!({
                "type": "object",
                "properties": {"full_res": {"type": "boolean"}},
                "required": []
            })
        ),
        tool!(
            "agent_crop",
            "crop",
            "crop",
            "Crop a screen region to a temp PNG (use as find_image template; Read the crop PNG to confirm it shows the intended widget before searching)",
            obj(&[
                ("x", "number"),
                ("y", "number"),
                ("width", "number"),
                ("height", "number")
            ])
        ),
        tool!(
            "agent_ocr",
            "ocr",
            "ocr",
            "OCR a screen region",
            obj(&[
                ("x", "number"),
                ("y", "number"),
                ("width", "number"),
                ("height", "number")
            ])
        ),
        tool!(
            "agent_find_image",
            "find_image",
            "find-image",
            "Find a template image on screen (cut with crop from a *prior* screenshot, then Read the crop to confirm it shows the intended widget; same-frame crop+find returns the crop origin as a self-match - exclude it or use region/all and verify)",
            json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string"},
                    "region_x": {"type": "number"}, "region_y": {"type": "number"},
                    "region_width": {"type": "number"}, "region_height": {"type": "number"},
                    "tolerance": {"type": "number"},
                    "all": {"type": "boolean"}
                },
                "required": ["path"]
            })
        ),
        tool!(
            "agent_pixel",
            "pixel",
            "pixel",
            "Pixel color at a point",
            obj(&[("x", "number"), ("y", "number")])
        ),
        tool!(
            "agent_move_mouse",
            "move_mouse",
            "move-mouse",
            "Move the mouse (gated + arbitrated; use cursor to verify position)",
            json!({
                "type": "object",
                "properties": {
                    "x": {"type": "number"}, "y": {"type": "number"},
                    "force": {"type": "boolean"},
                    "expect": {"type": "string"}
                },
                "required": ["x", "y"]
            })
        ),
        tool!(
            "agent_move_mouse_rel",
            "move_mouse_rel",
            "move-mouse-rel",
            "Move the mouse relatively (gated + arbitrated; for pointer-lock games / no portal grant)",
            json!({
                "type": "object",
                "properties": {
                    "dx": {"type": "integer"}, "dy": {"type": "integer"},
                    "force": {"type": "boolean"},
                    "expect": {"type": "string"}
                },
                "required": ["dx", "dy"]
            })
        ),
        tool!(
            "agent_click",
            "click",
            "click",
            "Click at a point (gated + arbitrated; move there first so hover settles, verify after)",
            json!({
                "type": "object",
                "properties": {
                    "x": {"type": "number"}, "y": {"type": "number"},
                    "button": {"type": "string"},
                    "force": {"type": "boolean"},
                    "expect": {"type": "string"}
                },
                "required": ["x", "y"]
            })
        ),
        tool!(
            "agent_scroll",
            "scroll",
            "scroll",
            "Scroll (gated + arbitrated)",
            json!({
                "type": "object",
                "properties": {
                    "amount": {"type": "integer"}, "axis": {"type": "string"},
                    "force": {"type": "boolean"},
                    "expect": {"type": "string"}
                },
                "required": ["amount"]
            })
        ),
        tool!(
            "agent_key",
            "key",
            "key",
            "Press a key combo as a held chord (e.g. Control+n holds Control then taps n; gated + arbitrated). hold_ms keeps it down; action press/release holds across calls",
            json!({
                "type": "object",
                "properties": {
                    "combo": {"type": "string"},
                    "hold_ms": {"type": "integer"},
                    "action": {"type": "string"},
                    "force": {"type": "boolean"},
                    "expect": {"type": "string"}
                },
                "required": ["combo"]
            })
        ),
        tool!(
            "agent_type_text",
            "type_text",
            "type-text",
            "Type text (gated + arbitrated)",
            json!({
                "type": "object",
                "properties": {
                    "text": {"type": "string"},
                    "force": {"type": "boolean"},
                    "expect": {"type": "string"}
                },
                "required": ["text"]
            })
        ),
        tool!(
            "agent_button",
            "button",
            "button",
            "Press/release a mouse button (press holds until release)",
            json!({
                "type": "object",
                "properties": {
                    "button": {"type": "string"}, "action": {"type": "string"},
                    "force": {"type": "boolean"},
                    "expect": {"type": "string"}
                },
                "required": ["button", "action"]
            })
        ),
        tool!(
            "agent_window_action",
            "window_action",
            "window-action",
            "Focus/minimize/close a window (use Focus before input when focus may be elsewhere)",
            obj(&[("pattern", "string"), ("action", "string")])
        ),
        tool!(
            "agent_window_move",
            "window_move",
            "window-move",
            "Move a window (verify active_window after window ops)",
            obj(&[("pattern", "string"), ("x", "integer"), ("y", "integer")])
        ),
        tool!(
            "agent_clipboard_get",
            "clipboard_get",
            "clipboard-get",
            "Read the clipboard",
            obj(&[])
        ),
        tool!(
            "agent_clipboard_set",
            "clipboard_set",
            "clipboard-set",
            "Write the clipboard",
            obj(&[("text", "string")])
        ),
        tool!(
            "agent_notify",
            "notify",
            "notify",
            "Desktop notification",
            obj(&[("title", "string"), ("message", "string")])
        ),
        tool!(
            "agent_list_apps",
            "list_apps",
            "list-apps",
            "List installed applications",
            obj(&[])
        ),
        tool!(
            "agent_launch_app",
            "launch_app",
            "launch-app",
            "Launch an installed app by name (or direct command) with optional args (e.g. a file path); detached and receipts include after-focus; verify via windows/active_window",
            json!({
                "type": "object",
                "properties": {
                    "app": {"type": "string"}, "command": {"type": "string"},
                    "args": {"type": "string"}, "force": {"type": "boolean"}
                },
                "required": []
            })
        ),
        tool!(
            "agent_windows",
            "windows",
            "windows",
            "List windows with rects",
            obj(&[])
        ),
        tool!(
            "agent_active_window",
            "active_window",
            "active-window",
            "Active window (verify app identity after launches; do not assume a click/Enter worked)",
            obj(&[])
        ),
        tool!(
            "agent_window_bounds",
            "window_bounds",
            "window-bounds",
            "Window bounds by pattern",
            obj(&[("pattern", "string")])
        ),
        tool!(
            "agent_cursor",
            "cursor",
            "cursor",
            "Cursor position",
            obj(&[])
        ),
    ]
}

static TABLE: LazyLock<Vec<ToolDef>> = LazyLock::new(table);

pub fn tool_table() -> &'static [ToolDef] {
    &TABLE
}

#[cfg(test)]
mod parity {
    use super::*;
    use blockwork_protocol::AGENT_COMMANDS;

    #[test]
    fn tool_table_matches_allowlist() {
        let mut wire: Vec<&str> = tool_table().iter().map(|t| t.wire).collect();
        let mut allow: Vec<&str> = AGENT_COMMANDS.to_vec();
        wire.sort_unstable();
        allow.sort_unstable();
        assert_eq!(wire, allow, "tools.rs must match AGENT_COMMANDS");
    }

    #[test]
    fn tool_names_unique() {
        let mut wires = Vec::new();
        for t in tool_table() {
            assert!(t.wire.starts_with("agent_"), "{}", t.wire);
            assert_eq!(t.mcp, t.wire.strip_prefix("agent_").unwrap(), "{}", t.wire);
            assert!(!wires.contains(&t.wire), "dup {}", t.wire);
            wires.push(t.wire);
        }
    }
}
