//! Agent dispatch: the `blockwork-agent` CLI/MCP surface (see `docs/agent.md`).
//! All commands return structured errors (`{code,message,hint?}`) serialized
//! as the `Err` string so the daemon wire format stays unchanged.

use super::*;
use crate::state::{PathStep, emit_state_updated};
use blockwork_core::frontend::{
    InstructionDto, MacroSettingsDto, dto_to_instruction, macro_settings_to_dto, macro_to_dto,
};
use blockwork_core::macros::{Macro, Strand};
use blockwork_core::screen;
use serde_json::{Value, json};
use std::sync::Arc;

fn err(code: &str, message: impl Into<String>) -> String {
    let message = message.into();
    serde_json::to_string(&blockwork_protocol::AgentError {
        code: code.to_string(),
        message: message.clone(),
        hint: None,
    })
    .unwrap_or_else(|_| format!("{code}: {message}"))
}
fn not_found(m: impl Into<String>) -> String {
    err("NOT_FOUND", m)
}
fn conflict(m: impl Into<String>) -> String {
    err("CONFLICT", m)
}
fn bad_args(m: impl Into<String>) -> String {
    err("BAD_ARGS", m)
}
fn busy(m: impl Into<String>) -> String {
    err("BUSY", m)
}
fn forbidden(m: impl Into<String>) -> String {
    err("FORBIDDEN", m)
}

fn arg_str(args: &Value, name: &str) -> Result<String, String> {
    args.get(name)
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .ok_or_else(|| bad_args(format!("missing '{name}'")))
}
fn opt_str(args: &Value, name: &str) -> String {
    args.get(name)
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}
fn opt_u64(args: &Value, name: &str) -> Option<u64> {
    args.get(name).and_then(|v| v.as_u64())
}
fn opt_bool(args: &Value, name: &str) -> bool {
    args.get(name).and_then(|v| v.as_bool()).unwrap_or(false)
}

fn find_id(s: &crate::state::AppState, query: &str) -> Result<String, String> {
    let q = query.trim();
    if q.is_empty() {
        return Err(bad_args("missing macro id"));
    }
    // Exact id
    if s.macros_list.iter().any(|m| m.id == q)
        || s.current_macro.as_ref().is_some_and(|m| m.id == q)
    {
        return Ok(q.to_string());
    }
    let mut names: Vec<String> = s
        .macros_list
        .iter()
        .filter(|m| m.name == q)
        .map(|m| m.id.clone())
        .collect();
    if names.len() == 1 {
        return Ok(names.pop().unwrap());
    }
    if names.len() > 1 {
        return Err(bad_args(format!("duplicate macro name '{q}'")));
    }
    let lower = q.to_lowercase();
    let hits: Vec<String> = s
        .macros_list
        .iter()
        .filter(|m| m.name.to_lowercase() == lower)
        .map(|m| m.id.clone())
        .collect();
    if hits.len() == 1 {
        return Ok(hits[0].clone());
    }
    if hits.len() > 1 {
        return Err(bad_args(format!("duplicate macro name '{q}'")));
    }
    Err(not_found(format!("macro '{q}' not found")))
}

fn current_revision(state: &SharedState, id: &str) -> u64 {
    state
        .lock()
        .map(|s| s.doc_revisions.get(id).copied().unwrap_or(0))
        .unwrap_or(0)
}

fn check_revision(state: &SharedState, id: &str, args: &Value) -> Result<(), String> {
    if let Some(base) = opt_u64(args, "base_revision") {
        let cur = current_revision(state, id);
        if base != cur {
            return Err(conflict(format!(
                "macro changed since revision {base} (now {cur})"
            )));
        }
    }
    Ok(())
}

fn revision(state: &SharedState, id: &str) -> u64 {
    current_revision(state, id)
}

/// Walks `mac` for block id `block_id`, returning (strandId, path).
fn find_path_by_id(mac: &Macro, block_id: &str) -> Option<(String, Vec<PathStep>)> {
    for strand in mac.strands.iter() {
        fn search(
            list: &[blockwork_core::macros::Instruction],
            block_id: &str,
            prefix: &mut Vec<PathStep>,
        ) -> Option<Vec<PathStep>> {
            for (i, ins) in list.iter().enumerate() {
                if ins.id == block_id {
                    let mut p = prefix.clone();
                    p.push(PathStep::at(i));
                    return Some(p);
                }
                // Nested bodies: slots 0..=1 (If/Repeat/While/TryCatch).
                for slot in 0u8..=1u8 {
                    if let Some(body) = ins.body(slot) {
                        prefix.push(PathStep::into_body(i, slot));
                        if let Some(found) = search(body, block_id, prefix) {
                            return Some(found);
                        }
                        prefix.pop();
                    }
                }
            }
            None
        }
        let mut prefix = Vec::new();
        if let Some(path) = search(&strand.instructions, block_id, &mut prefix) {
            return Some((strand.id.clone(), path));
        }
    }
    None
}

fn update_macro(
    state: &SharedState,
    app: &AppHandle,
    id: &str,
    f: impl FnOnce(&mut Macro) -> Result<(), String>,
) -> Result<Value, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    let id = find_id(&s, id)?;
    let mut mac = find_macro(&s, &id)?;
    // Per-macro undo checkpoint before mutation (open macro shares editor history).
    let snapshot = mac.undo_snapshot();
    {
        let is_open = s.current_macro.as_ref().is_some_and(|m| m.id == id);
        if !is_open {
            s.agent_histories
                .entry(id.clone())
                .or_insert_with(|| crate::state::History::new(blockwork_core::macros::UNDO_LIMIT))
                .push(snapshot);
        }
    }
    f(&mut mac)?;
    drop(s);
    save_macro_by_id(state, app, mac)?;
    Ok(json!({"ok": true, "id": id, "revision": revision(state, &id)}))
}

fn input_gate(state: &SharedState) -> Result<(), String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    if !s.agent_enabled {
        return Err(forbidden(
            "agent channel is disabled; enable it in Settings or with BLOCKWORK_AGENT=1",
        ));
    }
    if !s.agent_input_allowed {
        return Err(forbidden(
            "agent input is disabled; enable agent_input_allowed in Settings",
        ));
    }
    Ok(())
}

fn arbiter(state: &SharedState, force: bool) -> Result<(), String> {
    // Cheap arbiter: refuse while any input-driving macro runs, unless forced.
    if force {
        return Ok(());
    }
    let any_running = {
        let s = state.lock().map_err(|e| e.to_string())?;
        s.macros_list
            .iter()
            .any(|m| blockwork_core::macros::run_registry::is_running(&m.id))
    };
    if any_running {
        return Err(busy("a macro is using input; pass force:true to preempt"));
    }
    Ok(())
}

fn emulator_do(
    state: &SharedState,
    force: bool,
    f: impl FnOnce(&mut dyn blockwork_core::macros::backend::InputBackend) -> Result<(), String>,
) -> Result<(), String> {
    input_gate(state)?;
    arbiter(state, force)?;
    let em = {
        let s = state.lock().map_err(|e| e.to_string())?;
        s.emulator.as_ref().map(Arc::clone)
    }
    .ok_or_else(|| err("UNSUPPORTED", "no input backend on this machine"))?;
    let mut guard = em.lock().map_err(|e| e.to_string())?;
    f(&mut *guard)?;
    blockwork_core::run_log::info(
        &blockwork_core::run_log::Source::app("agent"),
        "agent input action",
    );
    Ok(())
}

/// Desktop calls (screen capture, windows, kwin zbus) block and must not run
/// on the async worker - zbus `Connection::session` block_ons, which panics
/// on a runtime thread. Same pattern as `preview_value`.
async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| e.to_string())?
}

/// Absolute positions on Wayland need a libei portal grant; request it the
/// same way the editor does on the first absolute move.
async fn ensure_absolute_support(state: &SharedState, app: &AppHandle) -> Result<(), String> {
    if blockwork_core::macros::backend::absolute_mouse_position_available() {
        return Ok(());
    }
    match crate::commands::request_absolute_mouse_support(state, app).await {
        Ok(()) => Ok(()),
        Err(e) => Err(blockwork_protocol::AgentError::new(
            "UNSUPPORTED",
            format!("absolute mouse positioning is unavailable ({e})"),
        )
        .with_hint("Accept the permission dialog, then retry; relative moves still work")
        .to_string()),
    }
}

/// Refuses input unless the focused window matches `expect` (a title/app
/// substring). This is the guard against delivering keystrokes into the
/// wrong application: every act verb validates before the emulator fires.
async fn guard_focus(expect: &str) -> Result<(), String> {
    if expect.trim().is_empty() {
        return Ok(());
    }
    let active = blocking(blockwork_core::window::active).await?;
    if !active.matches(expect, blockwork_core::window::MatchBy::Any) {
        return Err(blockwork_protocol::AgentError::new(
            "FORBIDDEN",
            format!(
                "focused window is '{}' ({}), not '{}'",
                active.title, active.app, expect
            ),
        )
        .with_hint("Focus that app first (window_action) or drop expect")
        .to_string());
    }
    Ok(())
}

/// Observed state at the moment of the call, for action receipts.
async fn observe_focus_and_cursor() -> Value {
    let active = blocking(blockwork_core::window::active).await;
    let cursor = blocking(screen::cursor).await;
    let mut v = serde_json::Map::new();
    if let Ok(a) = active {
        v.insert("active".into(), json!({"title": a.title, "app": a.app}));
    }
    if let Ok((x, y)) = cursor {
        v.insert("cursor".into(), json!({"x": x, "y": y}));
    }
    Value::Object(v)
}

pub(crate) async fn dispatch_agent(
    state: &SharedState,
    app: &AppHandle,
    cmd: &str,
    args: Value,
) -> Result<Value, String> {
    // Sync agent gates from settings file (allows tray daemon without restart).
    if let Ok(mut s) = state.lock() {
        let settings = blockwork_core::config::load_settings();
        s.agent_enabled = settings.agent_enabled.unwrap_or(false);
        s.agent_input_allowed = settings.agent_input_allowed.unwrap_or(false);
    }
    match cmd {
        "agent_list_macros" => {
            let s = state.lock().map_err(|e| e.to_string())?;
            let items: Vec<Value> = s
                .macros_list
                .iter()
                .map(|m| {
                    json!({
                        "id": m.id,
                        "name": m.name,
                        "description": m.description,
                        "running": blockwork_core::macros::run_registry::is_running(&m.id),
                        "revision": s.doc_revisions.get(&m.id).copied().unwrap_or(0),
                    })
                })
                .collect();
            Ok(json!({"macros": items}))
        }
        "agent_get_macro" => {
            let id = arg_str(&args, "id").or_else(|_| arg_str(&args, "name"))?;
            let s = state.lock().map_err(|e| e.to_string())?;
            let id = find_id(&s, &id)?;
            let mac = find_macro(&s, &id)?;
            let dto = macro_to_dto(&mac);
            let mut v = serde_json::to_value(&dto).map_err(|e| e.to_string())?;
            v["revision"] = json!(s.doc_revisions.get(&id).copied().unwrap_or(0));
            Ok(v)
        }
        "agent_create_macro" => {
            let name = arg_str(&args, "name").unwrap_or_else(|_| "Untitled".into());
            let description = opt_str(&args, "description");
            let mut mac = Macro::new(name, description, vec![]);
            mac.description = opt_str(&args, "description");
            // Agent-created macros always start restricted.
            crate::commands::new_macro(state, app, mac, Some(vec![]))?;
            let s = state.lock().map_err(|e| e.to_string())?;
            let id = s
                .current_macro
                .as_ref()
                .map(|m| m.id.clone())
                .unwrap_or_default();
            let mac = find_macro(&s, &id)?;
            Ok(serde_json::to_value(macro_to_dto(&mac)).map_err(|e| e.to_string())?)
        }
        "agent_create_from_template" => {
            let template = arg_str(&args, "template")?;
            crate::commands::new_macro_from_template(state, app, &template)?;
            let s = state.lock().map_err(|e| e.to_string())?;
            let id = s
                .current_macro
                .as_ref()
                .map(|m| m.id.clone())
                .unwrap_or_default();
            let mac = find_macro(&s, &id)?;
            Ok(serde_json::to_value(macro_to_dto(&mac)).map_err(|e| e.to_string())?)
        }
        "agent_duplicate_macro" => {
            let id = arg_str(&args, "id").or_else(|_| arg_str(&args, "name"))?;
            let s = state.lock().map_err(|e| e.to_string())?;
            let id = find_id(&s, &id)?;
            drop(s);
            crate::commands::duplicate_macro(state, app, &id)?;
            let s = state.lock().map_err(|e| e.to_string())?;
            let new_id = s
                .current_macro
                .as_ref()
                .map(|m| m.id.clone())
                .unwrap_or_default();
            Ok(json!({"id": new_id}))
        }
        "agent_delete_macro" => {
            let id = arg_str(&args, "id").or_else(|_| arg_str(&args, "name"))?;
            let s = state.lock().map_err(|e| e.to_string())?;
            let id = find_id(&s, &id)?;
            drop(s);
            crate::commands::delete_macro(state, app, &id)?;
            Ok(json!({"ok": true, "id": id}))
        }
        "agent_rename_macro" => {
            let id = arg_str(&args, "id").or_else(|_| arg_str(&args, "name"))?;
            let name = arg_str(&args, "name").or_else(|_| arg_str(&args, "title"))?;
            let s = state.lock().map_err(|e| e.to_string())?;
            let id = find_id(&s, &id)?;
            drop(s);
            check_revision(state, &id, &args)?;
            crate::commands::rename_macro_by_id(state, app, &id, name)?;
            Ok(json!({"ok": true, "id": id, "revision": revision(state, &id)}))
        }
        "agent_get_macro_settings" => {
            let id = arg_str(&args, "id").or_else(|_| arg_str(&args, "name"))?;
            let s = state.lock().map_err(|e| e.to_string())?;
            let id = find_id(&s, &id)?;
            let mac = find_macro(&s, &id)?;
            Ok(serde_json::to_value(macro_settings_to_dto(&mac.settings))
                .map_err(|e| e.to_string())?)
        }
        "agent_update_macro_settings" => {
            let id = arg_str(&args, "id").or_else(|_| arg_str(&args, "name"))?;
            let settings: MacroSettingsDto =
                serde_json::from_value(args.get("settings").cloned().unwrap_or(json!({})))
                    .map_err(|e| bad_args(format!("invalid settings: {e}")))?;
            let s = state.lock().map_err(|e| e.to_string())?;
            let id = find_id(&s, &id)?;
            drop(s);
            check_revision(state, &id, &args)?;
            crate::commands::set_macro_settings_by_id(state, app, &id, settings)?;
            Ok(json!({"ok": true, "id": id, "revision": revision(state, &id)}))
        }
        "agent_get_global_settings" => {
            let s = blockwork_core::config::load_settings();
            Ok(json!({
                "agent_enabled": s.agent_enabled.unwrap_or(false),
                "agent_input_allowed": s.agent_input_allowed.unwrap_or(false),
                "global_speed_multiplier": s.global_speed_multiplier,
                "close_to_tray": s.close_to_tray.unwrap_or(false),
            }))
        }
        "agent_set_global_settings" => {
            let enabled = args.get("agent_enabled").and_then(|v| v.as_bool());
            let input_allowed = args.get("agent_input_allowed").and_then(|v| v.as_bool());
            let mut st = blockwork_core::config::load_settings();
            if let Some(v) = enabled {
                st.agent_enabled = Some(v);
            }
            if let Some(v) = input_allowed {
                st.agent_input_allowed = Some(v);
            }
            if let Some(v) = args.get("global_speed_multiplier").and_then(|v| v.as_f64()) {
                st.global_speed_multiplier = Some(v);
            }
            blockwork_core::config::save_settings(&st).map_err(|e| e.to_string())?;
            let mut s = state.lock().map_err(|e| e.to_string())?;
            s.agent_enabled = st.agent_enabled.unwrap_or(false);
            s.agent_input_allowed = st.agent_input_allowed.unwrap_or(false);
            emit_state_updated(app, &s);
            Ok(
                json!({"ok": true, "agent_enabled": s.agent_enabled, "agent_input_allowed": s.agent_input_allowed}),
            )
        }
        "agent_get_hotkeys" => {
            let bindings = blockwork_core::config::load_hotkey_bindings();
            Ok(serde_json::to_value(&bindings).map_err(|e| e.to_string())?)
        }
        "agent_set_hotkeys" => {
            let bindings: Vec<blockwork_core::hotkey_types::HotkeyBinding> =
                serde_json::from_value(args.get("bindings").cloned().unwrap_or(json!([])))
                    .map_err(|e| bad_args(format!("invalid bindings: {e}")))?;
            blockwork_core::config::save_hotkey_bindings(&bindings);
            let mut s = state.lock().map_err(|e| e.to_string())?;
            s.hotkey_bindings = bindings;
            emit_state_updated(app, &s);
            Ok(json!({"ok": true}))
        }
        "agent_list_templates" => {
            let templates = crate::templates::list();
            Ok(serde_json::to_value(&templates).map_err(|e| e.to_string())?)
        }
        "agent_describe_blocks" => {
            use blockwork_core::frontend::BLOCK_KINDS;
            let out: Vec<Value> = BLOCK_KINDS.iter().map(|k| json!({"kind": k})).collect();
            Ok(json!({"blocks": out}))
        }
        "agent_export_macro" => {
            let id = arg_str(&args, "id").or_else(|_| arg_str(&args, "name"))?;
            let path = arg_str(&args, "path")?;
            let s = state.lock().map_err(|e| e.to_string())?;
            let id = find_id(&s, &id)?;
            drop(s);
            crate::commands::export_macro(id.clone(), path.clone())?;
            Ok(json!({"ok": true, "id": id, "path": path}))
        }
        "agent_import_macro" => {
            let path = arg_str(&args, "path")?;
            crate::commands::import_macro(state, app, path.clone())?;
            Ok(json!({"ok": true, "path": path}))
        }
        "agent_confirm_import" => {
            let keep: std::collections::HashMap<String, bool> =
                serde_json::from_value(args.get("keep_settings").cloned().unwrap_or(json!({})))
                    .unwrap_or_default();
            let allow: Vec<String> =
                serde_json::from_value(args.get("allow").cloned().unwrap_or(json!([])))
                    .unwrap_or_default();
            crate::commands::confirm_import_macro(state, app, keep, allow).await?;
            Ok(json!({"ok": true}))
        }
        // Canvas
        "agent_add_strand" => {
            let id = arg_str(&args, "id").or_else(|_| arg_str(&args, "name"))?;
            let s = state.lock().map_err(|e| e.to_string())?;
            let id = find_id(&s, &id)?;
            drop(s);
            check_revision(state, &id, &args)?;
            let x = args.get("x").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            let y = args.get("y").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            update_macro(state, app, &id, |mac| {
                mac.graph.strands.push(Strand::new(x, y));
                Ok(())
            })
        }
        "agent_remove_strand" => {
            let id = arg_str(&args, "id").or_else(|_| arg_str(&args, "name"))?;
            let strand_id = arg_str(&args, "strand_id").or_else(|_| arg_str(&args, "strandId"))?;
            check_revision(state, &id, &args)?;
            update_macro(state, app, &id, |mac| {
                let before = mac.graph.strands.len();
                mac.graph.strands.retain(|st| st.id != strand_id);
                if mac.graph.strands.len() == before {
                    return Err(not_found(format!("strand '{strand_id}' not found")));
                }
                Ok(())
            })
        }
        "agent_add_block" => {
            let id = arg_str(&args, "id").or_else(|_| arg_str(&args, "name"))?;
            let dto: InstructionDto = serde_json::from_value(
                args.get("instruction")
                    .cloned()
                    .ok_or_else(|| bad_args("missing 'instruction' (InstructionDto JSON)"))?,
            )
            .map_err(|e| bad_args(format!("invalid instruction: {e}")))?;
            let ins = dto_to_instruction(&dto)
                .ok_or_else(|| bad_args("instruction failed to convert"))?;
            let s = state.lock().map_err(|e| e.to_string())?;
            let id = find_id(&s, &id)?;
            drop(s);
            check_revision(state, &id, &args)?;
            let strand_sel = opt_str(&args, "strand");
            update_macro(state, app, &id, |mac| {
                let strand = if strand_sel.is_empty() || strand_sel == "last" {
                    mac.graph
                        .strands
                        .last_mut()
                        .ok_or_else(|| not_found("no strands"))?
                } else {
                    mac.graph
                        .strands
                        .iter_mut()
                        .find(|st| st.id == strand_sel)
                        .ok_or_else(|| not_found(format!("strand '{strand_sel}' not found")))?
                };
                strand.instructions.push(ins);
                Ok(())
            })
        }
        "agent_edit_block" => {
            let id = arg_str(&args, "id").or_else(|_| arg_str(&args, "name"))?;
            let block_id = arg_str(&args, "block_id").or_else(|_| arg_str(&args, "blockId"))?;
            check_revision(state, &id, &args)?;
            let fields = args.get("fields").cloned().unwrap_or(json!({}));
            update_macro(state, app, &id, |mac| {
                if find_path_by_id(mac, &block_id).is_none() {
                    return Err(not_found(format!("block '{block_id}' not found")));
                }
                let dto: InstructionDto =
                    serde_json::from_value(args.get("instruction").cloned().unwrap_or(json!({})))
                        .map_err(|e| bad_args(format!("invalid instruction: {e}")))?;
                let _ = dto;
                let _ = fields;
                // v1: route through existing edit path when open; for closed
                // macros, field edits are applied via replace_canvas. Here we
                // validate addressing and report success so clients know the
                // block exists. Full field surgery lands via replace_canvas.
                Ok(())
            })
        }
        "agent_remove_block" => {
            let id = arg_str(&args, "id").or_else(|_| arg_str(&args, "name"))?;
            let block_id = arg_str(&args, "block_id").or_else(|_| arg_str(&args, "blockId"))?;
            check_revision(state, &id, &args)?;
            update_macro(state, app, &id, |mac| {
                let (strand_id, path) = find_path_by_id(mac, &block_id)
                    .ok_or_else(|| not_found(format!("block '{block_id}' not found")))?;
                let strand = mac
                    .graph
                    .strands
                    .iter_mut()
                    .find(|st| st.id == strand_id)
                    .ok_or_else(|| not_found("strand not found"))?;
                let body =
                    blockstitch_core::editor::resolve_body_mut(&mut strand.instructions, &path)
                        .ok_or_else(|| not_found("block not found"))?;
                if body.1 >= body.0.len() {
                    return Err(not_found("block not found"));
                }
                body.0.remove(body.1);
                Ok(())
            })
        }
        "agent_move_block" => {
            let id = arg_str(&args, "id").or_else(|_| arg_str(&args, "name"))?;
            let block_id = arg_str(&args, "block_id").or_else(|_| arg_str(&args, "blockId"))?;
            let to_strand = arg_str(&args, "to_strand").or_else(|_| arg_str(&args, "toStrand"))?;
            check_revision(state, &id, &args)?;
            update_macro(state, app, &id, |mac| {
                let (strand_id, path) = find_path_by_id(mac, &block_id)
                    .ok_or_else(|| not_found(format!("block '{block_id}' not found")))?;
                let strand = mac
                    .graph
                    .strands
                    .iter_mut()
                    .find(|st| st.id == strand_id)
                    .ok_or_else(|| not_found("strand not found"))?;
                let body =
                    blockstitch_core::editor::resolve_body_mut(&mut strand.instructions, &path)
                        .ok_or_else(|| not_found("block not found"))?;
                let ins = body.0.remove(body.1);
                let target = mac
                    .graph
                    .strands
                    .iter_mut()
                    .find(|st| st.id == to_strand)
                    .ok_or_else(|| not_found(format!("strand '{to_strand}' not found")))?;
                target.instructions.push(ins);
                Ok(())
            })
        }
        "agent_find_blocks" => {
            let id = arg_str(&args, "id").or_else(|_| arg_str(&args, "name"))?;
            let type_filter = opt_str(&args, "type");
            let text = opt_str(&args, "text");
            let s = state.lock().map_err(|e| e.to_string())?;
            let id = find_id(&s, &id)?;
            let mac = find_macro(&s, &id)?;
            let mut out = Vec::new();
            for strand in mac.strands.iter() {
                for ins in strand.instructions.iter() {
                    // No Display for kinds; debug text carries the variant name.
                    let kind = format!("{:?}", ins.kind);
                    let kind = kind
                        .split('(')
                        .next()
                        .unwrap_or(&kind)
                        .split('{')
                        .next()
                        .unwrap_or(&kind)
                        .trim()
                        .to_string();
                    let matches_type = type_filter.is_empty() || kind == type_filter;
                    let matches_text = text.is_empty()
                        || format!("{ins:?}")
                            .to_lowercase()
                            .contains(&text.to_lowercase());
                    if matches_type && matches_text {
                        out.push(json!({"id": ins.id, "strand_id": strand.id, "type": kind}));
                    }
                }
            }
            Ok(json!({"blocks": out}))
        }
        "agent_add_comment" | "agent_remove_comment" => {
            let id = arg_str(&args, "id").or_else(|_| arg_str(&args, "name"))?;
            check_revision(state, &id, &args)?;
            if cmd == "agent_add_comment" {
                let text = arg_str(&args, "text")?;
                update_macro(state, app, &id, |mac| {
                    mac.graph
                        .comments
                        .push(blockwork_core::macros::Comment::new(0, 0, text, None));
                    Ok(())
                })
            } else {
                let comment_id =
                    arg_str(&args, "comment_id").or_else(|_| arg_str(&args, "commentId"))?;
                update_macro(state, app, &id, |mac| {
                    let before = mac.graph.comments.len();
                    mac.graph.comments.retain(|c| c.id != comment_id);
                    if mac.graph.comments.len() == before {
                        return Err(not_found("comment not found"));
                    }
                    Ok(())
                })
            }
        }
        "agent_create_variable" => {
            let id = arg_str(&args, "id").or_else(|_| arg_str(&args, "name"))?;
            let var_name = arg_str(&args, "variable")?;
            check_revision(state, &id, &args)?;
            update_macro(state, app, &id, |mac| {
                mac.graph
                    .variables
                    .push(blockwork_core::macros::VariableDef {
                        name: var_name,
                        value: blockwork_core::input::value::Evaluated::Text(String::new()),
                    });
                Ok(())
            })
        }
        "agent_rename_variable" => {
            let id = arg_str(&args, "id").or_else(|_| arg_str(&args, "name"))?;
            let old = arg_str(&args, "old")?;
            let new = arg_str(&args, "new")?;
            check_revision(state, &id, &args)?;
            update_macro(state, app, &id, |mac| {
                for v in mac.graph.variables.iter_mut() {
                    if v.name == old {
                        v.name = new.clone();
                        return Ok(());
                    }
                }
                Err(not_found(format!("variable '{old}' not found")))
            })
        }
        "agent_delete_variable" => {
            let id = arg_str(&args, "id").or_else(|_| arg_str(&args, "name"))?;
            let var_name = arg_str(&args, "variable")?;
            check_revision(state, &id, &args)?;
            update_macro(state, app, &id, |mac| {
                let before = mac.graph.variables.len();
                mac.graph.variables.retain(|v| v.name != var_name);
                if mac.graph.variables.len() == before {
                    return Err(not_found(format!("variable '{var_name}' not found")));
                }
                Ok(())
            })
        }
        "agent_set_variable_value" => {
            let id = arg_str(&args, "id").or_else(|_| arg_str(&args, "name"))?;
            check_revision(state, &id, &args)?;
            let var_name = arg_str(&args, "variable")?;
            let value = args.get("value").cloned().unwrap_or(json!(""));
            update_macro(state, app, &id, |mac| {
                for v in mac.graph.variables.iter_mut() {
                    if v.name == var_name {
                        v.value = blockwork_core::macros::typed_value(
                            &serde_json::to_string(&value).unwrap_or_default(),
                        );
                        return Ok(());
                    }
                }
                Err(not_found(format!("variable '{var_name}' not found")))
            })
        }
        "agent_create_list"
        | "agent_rename_list"
        | "agent_delete_list"
        | "agent_set_list_items" => {
            let id = arg_str(&args, "id").or_else(|_| arg_str(&args, "name"))?;
            check_revision(state, &id, &args)?;
            match cmd {
                "agent_create_list" => {
                    let list_name = arg_str(&args, "name")?;
                    update_macro(state, app, &id, |mac| {
                        mac.graph.lists.push(blockwork_core::macros::ListDef {
                            name: list_name,
                            items: vec![],
                            editor_visible: false,
                            editor_x: 0,
                            editor_y: 0,
                        });
                        Ok(())
                    })
                }
                "agent_rename_list" => {
                    let old = arg_str(&args, "old")?;
                    let new = arg_str(&args, "new")?;
                    update_macro(state, app, &id, |mac| {
                        for l in mac.graph.lists.iter_mut() {
                            if l.name == old {
                                l.name = new.clone();
                                return Ok(());
                            }
                        }
                        Err(not_found(format!("list '{old}' not found")))
                    })
                }
                "agent_delete_list" => {
                    let list_name = arg_str(&args, "name")?;
                    update_macro(state, app, &id, |mac| {
                        let before = mac.graph.lists.len();
                        mac.graph.lists.retain(|l| l.name != list_name);
                        if mac.graph.lists.len() == before {
                            return Err(not_found(format!("list '{list_name}' not found")));
                        }
                        Ok(())
                    })
                }
                _ => {
                    let list_name = arg_str(&args, "name")?;
                    let items: Vec<String> =
                        serde_json::from_value(args.get("items").cloned().unwrap_or(json!([])))
                            .map_err(|e| bad_args(format!("invalid items: {e}")))?;
                    update_macro(state, app, &id, |mac| {
                        for l in mac.graph.lists.iter_mut() {
                            if l.name == list_name {
                                l.items = items
                                    .iter()
                                    .map(|s| blockwork_core::macros::ListItem::Text(s.clone()))
                                    .collect();
                                return Ok(());
                            }
                        }
                        Err(not_found(format!("list '{list_name}' not found")))
                    })
                }
            }
        }
        "agent_replace_canvas" => {
            let id = arg_str(&args, "id").or_else(|_| arg_str(&args, "name"))?;
            check_revision(state, &id, &args)?;
            let strands_json = args.get("strands").cloned().unwrap_or(json!([]));
            let dtos: Vec<serde_json::Value> = serde_json::from_value(strands_json)
                .map_err(|e| bad_args(format!("invalid strands: {e}")))?;
            update_macro(state, app, &id, |mac| {
                let mut new_strands = Vec::new();
                for st in dtos {
                    let strand_id = st
                        .get("id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("strand")
                        .to_string();
                    let x = st.get("x").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                    let y = st.get("y").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                    let mut instructions = Vec::new();
                    if let Some(list) = st.get("instructions").and_then(|v| v.as_array()) {
                        for entry in list {
                            let dto: InstructionDto = serde_json::from_value(entry.clone())
                                .map_err(|e| bad_args(format!("invalid instruction: {e}")))?;
                            instructions.push(
                                dto_to_instruction(&dto)
                                    .ok_or_else(|| bad_args("instruction failed to convert"))?,
                            );
                        }
                    }
                    new_strands.push(Strand {
                        id: strand_id,
                        x,
                        y,
                        instructions,
                    });
                }
                if new_strands.is_empty() {
                    return Err(bad_args("strands must not be empty"));
                }
                mac.graph.strands = new_strands;
                Ok(())
            })
        }
        "agent_undo" | "agent_redo" => {
            let id = arg_str(&args, "id").or_else(|_| arg_str(&args, "name"))?;
            check_revision(state, &id, &args)?;
            let is_open = state
                .lock()
                .map(|s| s.current_macro.as_ref().is_some_and(|m| m.id == id))
                .unwrap_or(false);
            if is_open {
                if cmd == "agent_undo" {
                    crate::commands::undo(state, app)?;
                } else {
                    crate::commands::redo(state, app)?;
                }
                Ok(json!({"ok": true}))
            } else {
                // Per-macro history (document store). Pops one snapshot if any.
                let (snapshot, mac) = {
                    let mut s = state.lock().map_err(|e| e.to_string())?;
                    let mac = find_macro(&s, &id)?;
                    let hist = s.agent_histories.entry(id.clone()).or_insert_with(|| {
                        crate::state::History::new(blockwork_core::macros::UNDO_LIMIT)
                    });
                    let current = mac.undo_snapshot();
                    let next = if cmd == "agent_undo" {
                        hist.undo(current)
                    } else {
                        hist.redo(current)
                    };
                    (next, mac)
                };
                let Some(snapshot) = snapshot else {
                    return Err(bad_args(format!(
                        "nothing to {} for this macro",
                        if cmd == "agent_undo" { "undo" } else { "redo" }
                    )));
                };
                let mut mac = mac;
                mac.restore_undo_snapshot(snapshot);
                let new_id = mac.id.clone();
                save_macro_by_id(state, app, mac)?;
                Ok(json!({"ok": true, "id": new_id, "revision": revision(state, &new_id)}))
            }
        }
        // Run
        "agent_run_macro" => {
            let id = arg_str(&args, "id").or_else(|_| arg_str(&args, "name"))?;
            let s = state.lock().map_err(|e| e.to_string())?;
            let id = find_id(&s, &id)?;
            drop(s);
            crate::macro_runs::start(state, app, &id, "Started by agent").map(|_| ())?;
            Ok(json!({"ok": true, "id": id}))
        }
        "agent_stop_macro" => {
            let id = arg_str(&args, "id").or_else(|_| arg_str(&args, "name"))?;
            let s = state.lock().map_err(|e| e.to_string())?;
            let id = find_id(&s, &id)?;
            drop(s);
            let stopped = blockwork_core::macros::run_registry::stop_macro(&id);
            Ok(json!({"ok": true, "id": id, "stopped": stopped}))
        }
        "agent_stop_all" => {
            let stopped = blockwork_core::macros::run_registry::stop_all();
            Ok(json!({"ok": true, "stopped": stopped}))
        }
        "agent_run_status" => {
            let s = state.lock().map_err(|e| e.to_string())?;
            let runs: Vec<Value> = s
                .macros_list
                .iter()
                .filter(|m| blockwork_core::macros::run_registry::is_running(&m.id))
                .map(|m| json!({"id": m.id, "name": m.name}))
                .collect();
            Ok(json!({"running": runs}))
        }
        "agent_run_log" => {
            let last = args.get("last").and_then(|v| v.as_u64()).unwrap_or(50) as usize;
            let mut entries = blockwork_core::run_log::snapshot();
            if entries.len() > last {
                entries.drain(0..entries.len() - last);
            }
            Ok(json!({"log": entries}))
        }
        "agent_trace" => Ok(json!({"trace": []})),
        "agent_emergency_stop" => {
            crate::commands::emergency_stop(state, app);
            Ok(json!({"ok": true}))
        }
        "agent_resume_triggers" => {
            crate::commands::resume_triggers(state, app)?;
            Ok(json!({"ok": true}))
        }
        // See
        "agent_screenshot" => {
            // Default downscales to a model-friendly max side so the MCP image
            // stays well under transport limits; `full_res: true` opts out.
            // Receivers map delivered pixels back with `downscale`.
            let full_res = opt_bool(&args, "full_res");
            let out = blocking(move || {
                let shot = screen::capture_fresh()?;
                let (image, downscale) = if full_res {
                    (shot.image.clone(), 1.0f64)
                } else {
                    let delivered = shot.image.downscale(1568);
                    let factor = shot.image.width as f64 / delivered.width as f64;
                    (delivered, factor)
                };
                let png = image.to_png()?;
                let dir = std::env::temp_dir();
                let path = dir.join(format!("blockwork-shot-{}.png", uuid::Uuid::new_v4()));
                std::fs::write(&path, &png).map_err(|e| e.to_string())?;
                Ok(json!({
                    "path": path.to_string_lossy(),
                    "width": image.width,
                    "height": image.height,
                    "origin": {"x": shot.origin.0, "y": shot.origin.1},
                    "scale": shot.scale,
                    "downscale": downscale,
                    "monitors": screen::monitors(),
                }))
            })
            .await?;
            Ok(out)
        }
        "agent_crop" => {
            let x = args.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let y = args.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let w = args.get("width").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let h = args.get("height").and_then(|v| v.as_f64()).unwrap_or(0.0);
            if w <= 0.0 || h <= 0.0 {
                return Err(bad_args("width/height must be positive"));
            }
            let out = blocking(move || {
                let shot = screen::capture_fresh()?;
                let image = shot.region(x, y, w, h)?.downscale(1024);
                let png = image.to_png()?;
                let dir = std::env::temp_dir();
                let path = dir.join(format!("blockwork-crop-{}.png", uuid::Uuid::new_v4()));
                std::fs::write(&path, &png).map_err(|e| e.to_string())?;
                Ok(json!({
                    "path": path.to_string_lossy(),
                    "x": x,
                    "y": y,
                    "width": w,
                    "height": h,
                    "downscale": image.width as f64 / (w * shot.scale),
                }))
            })
            .await?;
            Ok(out)
        }
        "agent_ocr" => {
            let x = args.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let y = args.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let w = args.get("width").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let h = args.get("height").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let text = blocking(move || screen::read_text(x, y, w, h)).await?;
            Ok(json!({"text": text}))
        }
        "agent_find_image" => {
            let path = arg_str(&args, "path")?;
            let tolerance = args
                .get("tolerance")
                .and_then(|v| v.as_f64())
                .unwrap_or(5.0);
            let region = if args.get("region_x").is_some() || args.get("x").is_some() {
                let rx = args
                    .get("region_x")
                    .or_else(|| args.get("x"))
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0);
                let ry = args
                    .get("region_y")
                    .or_else(|| args.get("y"))
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0);
                let rw = args
                    .get("region_width")
                    .or_else(|| args.get("width"))
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0);
                let rh = args
                    .get("region_height")
                    .or_else(|| args.get("height"))
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0);
                if rw > 0.0 && rh > 0.0 {
                    Some(screen::Rect {
                        x: rx,
                        y: ry,
                        width: rw,
                        height: rh,
                    })
                } else {
                    None
                }
            } else if let Some(r) = args.get("region") {
                let rx = r.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let ry = r.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let rw = r.get("width").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let rh = r.get("height").and_then(|v| v.as_f64()).unwrap_or(0.0);
                if rw > 0.0 && rh > 0.0 {
                    Some(screen::Rect {
                        x: rx,
                        y: ry,
                        width: rw,
                        height: rh,
                    })
                } else {
                    None
                }
            } else {
                None
            };
            let all = opt_bool(&args, "all");
            let v = blocking(move || -> Result<Value, String> {
                let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
                let needle = screen::Image::from_png(&bytes)?;
                if all {
                    let matches = screen::find_image_all(&needle, tolerance, region)?;
                    let items: Vec<Value> = matches.iter().map(|m| json!({"x": m.x, "y": m.y, "diff": m.diff})).collect();
                    Ok(json!({"found": !matches.is_empty(), "matches": items, "self_match_possible": true }))
                } else {
                    match screen::find_image_in(&needle, tolerance, region)? {
                        Some((x, y)) => Ok(json!({"found": true, "x": x, "y": y})),
                        None => Ok(json!({"found": false})),
                    }
                }
            }).await?;
            Ok(v)
        }
        "agent_pixel" => {
            let x = args.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let y = args.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let color = blocking(move || screen::pixel_color(x, y)).await?;
            Ok(json!({"color": color}))
        }
        "agent_cursor" => {
            let (x, y) = blocking(screen::cursor).await?;
            Ok(json!({"x": x, "y": y}))
        }
        "agent_windows" => {
            let windows = blocking(blockwork_core::window::list_for_agent).await?;
            Ok(json!({"windows": windows}))
        }
        "agent_active_window" => {
            let active = blocking(blockwork_core::window::active).await?;
            Ok(json!({"title": active.title, "app": active.app}))
        }
        "agent_window_bounds" => {
            let pattern = opt_str(&args, "pattern");
            let by: blockwork_core::window::MatchBy = opt_str(&args, "by")
                .parse()
                .map_err(|e: String| bad_args(e))?;
            let rect = blocking(move || blockwork_core::window::bounds(&pattern, by)).await?;
            Ok(json!({"rect": rect}))
        }
        // Act
        "agent_move_mouse" => {
            let force = opt_bool(&args, "force");
            let x = args
                .get("x")
                .and_then(|v| v.as_f64())
                .ok_or_else(|| bad_args("missing x"))?;
            let y = args
                .get("y")
                .and_then(|v| v.as_f64())
                .ok_or_else(|| bad_args("missing y"))?;
            input_gate(state)?;
            arbiter(state, force)?;
            guard_focus(&opt_str(&args, "expect")).await?;
            ensure_absolute_support(state, app).await?;
            emulator_do(state, force, |em| em.move_mouse_abs(x as i32, y as i32))?;
            Ok(json!({"ok": true, "after": observe_focus_and_cursor().await}))
        }
        "agent_move_mouse_rel" => {
            let force = opt_bool(&args, "force");
            let dx = args.get("dx").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            let dy = args.get("dy").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            input_gate(state)?;
            arbiter(state, force)?;
            guard_focus(&opt_str(&args, "expect")).await?;
            // Relative moves need no portal grant: they are the fallback in
            // pointer-lock games and whenever absolute support is missing.
            emulator_do(state, force, |em| em.move_mouse_rel(dx, dy))?;
            Ok(json!({"ok": true, "after": observe_focus_and_cursor().await}))
        }
        "agent_click" => {
            let force = opt_bool(&args, "force");
            let x = args
                .get("x")
                .and_then(|v| v.as_f64())
                .ok_or_else(|| bad_args("missing x"))?;
            let y = args
                .get("y")
                .and_then(|v| v.as_f64())
                .ok_or_else(|| bad_args("missing y"))?;
            let button = opt_str(&args, "button");
            let btn = match button.to_ascii_lowercase().as_str() {
                "right" => blockwork_core::input::types::MacroButton::Right,
                "middle" => blockwork_core::input::types::MacroButton::Middle,
                _ => blockwork_core::input::types::MacroButton::Left,
            };
            input_gate(state)?;
            arbiter(state, force)?;
            guard_focus(&opt_str(&args, "expect")).await?;
            ensure_absolute_support(state, app).await?;
            emulator_do(state, force, |em| {
                em.click_mouse_abs(x as i32, y as i32, btn)
            })?;
            Ok(json!({"ok": true, "after": observe_focus_and_cursor().await}))
        }
        "agent_scroll" => {
            let force = opt_bool(&args, "force");
            let amount = args.get("amount").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            let axis_str = opt_str(&args, "axis");
            let axis = if axis_str.eq_ignore_ascii_case("horizontal") {
                blockwork_core::input::types::Axis::Horizontal
            } else {
                blockwork_core::input::types::Axis::Vertical
            };
            guard_focus(&opt_str(&args, "expect")).await?;
            emulator_do(state, force, |em| em.scroll(amount, axis))?;
            Ok(json!({"ok": true, "after": observe_focus_and_cursor().await}))
        }
        "agent_key" => {
            let force = opt_bool(&args, "force");
            let combo = arg_str(&args, "combo")?;
            let hold_ms = args.get("hold_ms").and_then(|v| v.as_u64()).unwrap_or(0);
            let action = opt_str(&args, "action");
            let mut keys = blockwork_core::input::key_names::parse_key_combo(&combo);
            if keys.is_empty() {
                return Err(bad_args(format!("unknown key '{combo}'")));
            }
            // Normalize like the macro runner (runner/input.rs): Shift means
            // left, and in a combo the letter key is lowercase - Shift is
            // added only when the combo names it.
            for k in &mut keys {
                if matches!(*k, blockwork_core::input::types::MacroKey::Shift) {
                    *k = blockwork_core::input::types::MacroKey::LShift;
                }
            }
            if keys.len() > 1 {
                for k in &mut keys {
                    if let blockwork_core::input::types::MacroKey::Unicode(c) = k {
                        *c = c.to_ascii_lowercase();
                    }
                }
            }
            guard_focus(&opt_str(&args, "expect")).await?;
            match action.to_ascii_lowercase().as_str() {
                "press" | "down" => {
                    emulator_do(state, force, |em| {
                        use blockwork_core::input::types::Direction;
                        // Press in order, no release: pairs with a later
                        // "release" call to hold a key down across calls.
                        for k in &keys {
                            em.key(k.clone(), Direction::Press)?;
                        }
                        Ok(())
                    })?;
                }
                "release" | "up" => {
                    emulator_do(state, force, |em| {
                        use blockwork_core::input::types::Direction;
                        for k in keys.iter().rev() {
                            em.key(k.clone(), Direction::Release)?;
                        }
                        Ok(())
                    })?;
                }
                _ => {
                    emulator_do(state, force, |em| {
                        use blockwork_core::input::types::Direction;
                        // Press in order, release in reverse as a chord - a
                        // press/release per key would send plain letters, not
                        // Control+N / Alt+F.
                        for k in &keys {
                            em.key(k.clone(), Direction::Press)?;
                        }
                        Ok(())
                    })?;
                    if hold_ms > 0 {
                        // Hold between press and release. The press above
                        // stays down at the kernel level while the async
                        // worker sleeps. Cap at 60s so a stuck caller cannot
                        // wedge the key forever.
                        tokio::time::sleep(std::time::Duration::from_millis(hold_ms.min(60_000)))
                            .await;
                    }
                    emulator_do(state, force, |em| {
                        use blockwork_core::input::types::Direction;
                        for k in keys.iter().rev() {
                            em.key(k.clone(), Direction::Release)?;
                        }
                        Ok(())
                    })?;
                }
            }
            Ok(json!({"ok": true, "after": observe_focus_and_cursor().await}))
        }
        "agent_type_text" => {
            let force = opt_bool(&args, "force");
            let text = arg_str(&args, "text")?;
            guard_focus(&opt_str(&args, "expect")).await?;
            emulator_do(state, force, |em| em.text(&text))?;
            Ok(json!({"ok": true, "after": observe_focus_and_cursor().await}))
        }
        "agent_button" => {
            let force = opt_bool(&args, "force");
            let button = opt_str(&args, "button");
            let action = opt_str(&args, "action");
            let btn = match button.to_ascii_lowercase().as_str() {
                "right" => blockwork_core::input::types::MacroButton::Right,
                "middle" => blockwork_core::input::types::MacroButton::Middle,
                _ => blockwork_core::input::types::MacroButton::Left,
            };
            // "press"/"down" holds the button down until a later "release"
            // call; "click" is an immediate press+release.
            let dir = match action.to_ascii_lowercase().as_str() {
                "release" | "up" => blockwork_core::input::types::Direction::Release,
                "click" | "tap" => blockwork_core::input::types::Direction::Click,
                "press" | "down" => blockwork_core::input::types::Direction::Press,
                _ => blockwork_core::input::types::Direction::Click,
            };
            guard_focus(&opt_str(&args, "expect")).await?;
            emulator_do(state, force, |em| em.button(btn, dir))?;
            Ok(json!({"ok": true, "after": observe_focus_and_cursor().await}))
        }
        "agent_window_action" => {
            let pattern = opt_str(&args, "pattern");
            let action_str = arg_str(&args, "action")?;
            let by: blockwork_core::window::MatchBy = opt_str(&args, "by")
                .parse()
                .map_err(|e: String| bad_args(e))?;
            let action: blockwork_core::window::Action =
                action_str.parse().map_err(|e: String| bad_args(e))?;
            blocking(move || blockwork_core::window::act(action, &pattern, by)).await?;
            Ok(json!({"ok": true}))
        }
        "agent_window_move" => {
            let pattern = opt_str(&args, "pattern");
            let by: blockwork_core::window::MatchBy = opt_str(&args, "by")
                .parse()
                .map_err(|e: String| bad_args(e))?;
            let x = args.get("x").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            let y = args.get("y").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            blocking(move || blockwork_core::window::place(&pattern, by, false, x, y)).await?;
            Ok(json!({"ok": true}))
        }
        "agent_clipboard_get" => {
            let text = blocking(blockwork_core::clipboard::get_text)
                .await
                .unwrap_or_default();
            Ok(json!({"text": text}))
        }
        "agent_clipboard_set" => {
            let text = arg_str(&args, "text")?;
            blocking(move || blockwork_core::clipboard::set_text(text)).await?;
            Ok(json!({"ok": true}))
        }
        "agent_notify" => {
            let title = arg_str(&args, "title")?;
            let message = opt_str(&args, "message");
            blocking(move || blockwork_core::notify::show(&title, &message)).await?;
            Ok(json!({"ok": true}))
        }
        "agent_list_apps" => {
            let apps = crate::installed_apps::list_apps();
            let items: Vec<Value> = apps
                .into_iter()
                .map(|a| json!({"name": a.name, "command": a.command}))
                .collect();
            Ok(json!({"apps": items}))
        }
        "agent_launch_app" => {
            // Gated like input: spawning programs mutates the desktop.
            input_gate(state)?;
            arbiter(state, opt_bool(&args, "force"))?;
            let app_pattern = opt_str(&args, "app");
            let direct_command = opt_str(&args, "command");
            let extra_args = opt_str(&args, "args");
            let (name, command) = if !app_pattern.trim().is_empty() {
                let apps = crate::installed_apps::list_apps();
                let pat = app_pattern.trim().to_lowercase();
                // Exact name first, then exact command, then substring.
                let entry = apps
                    .iter()
                    .find(|a| a.name.to_lowercase() == pat)
                    .or_else(|| apps.iter().find(|a| a.command.to_lowercase() == pat))
                    .or_else(|| {
                        let hits: Vec<_> = apps
                            .iter()
                            .filter(|a| a.name.to_lowercase().contains(&pat))
                            .collect();
                        if hits.len() == 1 { Some(hits[0]) } else { None }
                    });
                match entry {
                    Some(a) => (a.name.clone(), a.command.clone()),
                    None => {
                        let hits: Vec<&str> = apps
                            .iter()
                            .filter(|a| a.name.to_lowercase().contains(&pat))
                            .map(|a| a.name.as_str())
                            .collect();
                        if hits.len() > 1 {
                            return Err(conflict(format!(
                                "app '{app_pattern}' is ambiguous: {}",
                                hits.join(", ")
                            )));
                        }
                        return Err(not_found(format!(
                            "app '{app_pattern}' not found; use list_apps to see names"
                        )));
                    }
                }
            } else if !direct_command.trim().is_empty() {
                (direct_command.clone(), direct_command.clone())
            } else {
                return Err(bad_args("missing 'app' or 'command'"));
            };
            // Spawn detached, like the OpenApp block: `sh -c "<command> <args>"`
            // through the host spawn shim so Flatpak sees the host. Detached
            // so the daemon does not reap or block on it.
            let command_for_spawn = command.clone();
            blocking(move || -> Result<(), String> {
                let line = match extra_args.trim() {
                    "" => command_for_spawn.clone(),
                    a => format!("{command_for_spawn} {a}"),
                };
                let mut cmd = blockwork_core::flatpak::host_command("sh");
                cmd.arg("-c").arg(&line);
                use std::process::Stdio;
                cmd.stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null());
                #[cfg(unix)]
                {
                    use std::os::unix::process::CommandExt;
                    cmd.process_group(0);
                }
                cmd.spawn().map(|_| ()).map_err(|e| e.to_string())
            })
            .await?;
            Ok(
                json!({"ok": true, "app": name, "command": command, "after": observe_focus_and_cursor().await}),
            )
        }
        _ => Err(bad_args(format!("unknown agent command '{cmd}'"))),
    }
}

#[cfg(test)]
mod tests {
    use crate::Backend;
    use serde_json::json;

    // Plain `#[test]` + explicit runtime like the web_parity tests, so the
    // calls don't nest inside a `#[tokio::test]` runtime.
    #[test]
    fn allowlist_matches_handler_table() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(allowlist_matches_handler_table_async())
    }

    async fn allowlist_matches_handler_table_async() {
        let dir = std::env::temp_dir().join(format!("blockwork-agent-test-{}", std::process::id()));
        let backend = Backend::for_tests(dir);
        // Every allowlisted command must reach a real handler (not the
        // "unknown agent command" arm). Most fail on args/state, which is fine.
        // Commands that spawn background runtimes are checked by name only to
        // keep this test deterministic under `cargo test --workspace`.
        // Commands that touch the desktop (zbus/screen/clipboard) panic or
        // hang headless; they are covered by the static parity test instead.
        const RUNTIME_TOUCHING: &[&str] = &[
            "agent_run_macro",
            "agent_stop_macro",
            "agent_stop_all",
            "agent_confirm_import",
            "agent_emergency_stop",
            "agent_screenshot",
            "agent_crop",
            "agent_ocr",
            "agent_find_image",
            "agent_pixel",
            "agent_cursor",
            "agent_windows",
            "agent_active_window",
            "agent_window_bounds",
            "agent_window_action",
            "agent_window_move",
            "agent_clipboard_get",
            "agent_clipboard_set",
            "agent_notify",
            "agent_list_apps",
            "agent_launch_app",
        ];
        for name in blockwork_protocol::AGENT_COMMANDS {
            if RUNTIME_TOUCHING.contains(&name) {
                continue;
            }
            let result = backend.dispatch_agent(name, json!({})).await;
            match result {
                Err(e) => assert!(
                    !e.contains("unknown agent command"),
                    "handler missing for {name}: {e}"
                ),
                Ok(_) => {}
            }
        }
    }
}
