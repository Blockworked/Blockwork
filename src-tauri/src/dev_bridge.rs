//! Debug-only HTTP+WebSocket bridge (feature-gated, never in release builds)
//! so the running backend can be driven from a browser tab instead of the
//! Tauri webview. `POST /invoke/:cmd` dispatches onto the real
//! `#[tauri::command]` functions; `GET /events` mirrors `state-updated`
//! emissions over a WebSocket. Binds to 127.0.0.1 only.

use crate::commands;
use crate::state::{
    self, BlockPieceDto, HotkeyActionDto, InstructionDto, PathStep, SharedState, ValueDto,
    ValueLocationDto,
};
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State as AxumState};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use blockwork_core::macros::BlockShape;
use serde::de::DeserializeOwned;
use serde_json::Value as Json_;
use std::sync::Arc;
use tauri::{Listener, Manager};
use tokio::sync::broadcast;
use tower_http::cors::CorsLayer;
use tracing::{info, warn};

const PORT: u16 = 4127;

#[derive(Clone)]
struct BridgeCtx {
    app: tauri::AppHandle,
    state_tx: Arc<broadcast::Sender<String>>,
}

pub(crate) async fn run(app: tauri::AppHandle) {
    let (state_tx, _) = broadcast::channel::<String>(64);
    let state_tx = Arc::new(state_tx);

    // Tauri's emit/listen event bus is decoupled from any webview, so a
    // Rust-side listener sees every `state-updated` emission just like a
    // real webview would.
    {
        let tx = Arc::clone(&state_tx);
        app.listen("state-updated", move |event| {
            let _ = tx.send(event.payload().to_string());
        });
    }

    let ctx = BridgeCtx { app, state_tx };

    let router = Router::new()
        .route("/invoke/{cmd}", post(invoke_handler))
        .route("/events", get(ws_handler))
        .layer(CorsLayer::permissive())
        .with_state(ctx);

    let listener = match tokio::net::TcpListener::bind(("127.0.0.1", PORT)).await {
        Ok(l) => l,
        Err(err) => {
            warn!("dev-bridge: failed to bind 127.0.0.1:{PORT}: {err}");
            return;
        }
    };
    info!("dev-bridge: listening on http://127.0.0.1:{PORT}");
    if let Err(err) = axum::serve(listener, router).await {
        warn!("dev-bridge: server error: {err}");
    }
}

async fn ws_handler(ws: WebSocketUpgrade, AxumState(ctx): AxumState<BridgeCtx>) -> Response {
    ws.on_upgrade(move |socket| handle_socket(socket, ctx))
}

async fn handle_socket(mut socket: WebSocket, ctx: BridgeCtx) {
    // Snapshot on connect so a freshly-opened tab sees state immediately.
    let shared = ctx.app.state::<SharedState>();
    let initial = match shared.lock() {
        Ok(s) => serde_json::to_string(&state::build_state_dto(&s)).ok(),
        Err(_) => None,
    };
    if let Some(initial) = initial {
        if socket.send(Message::Text(initial.into())).await.is_err() {
            return;
        }
    }

    let mut rx = ctx.state_tx.subscribe();
    loop {
        tokio::select! {
            msg = rx.recv() => {
                match msg {
                    Ok(payload) => {
                        if socket.send(Message::Text(payload.into())).await.is_err() {
                            return;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => return,
                }
            }
            incoming = socket.recv() => {
                // We don't expect the client to send anything; just watch for
                // the socket closing so this task doesn't leak.
                if incoming.is_none() {
                    return;
                }
            }
        }
    }
}

fn field<T: DeserializeOwned>(body: &Json_, name: &str) -> Result<T, String> {
    let v = body.get(name).cloned().unwrap_or(Json_::Null);
    serde_json::from_value(v).map_err(|e| format!("invalid field '{name}': {e}"))
}

fn ok_response<T: serde::Serialize>(result: Result<T, String>) -> (StatusCode, Json<Json_>) {
    match result {
        Ok(v) => (
            StatusCode::OK,
            Json(serde_json::json!({"ok": true, "data": v})),
        ),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"ok": false, "error": e})),
        ),
    }
}

async fn invoke_handler(
    Path(cmd): Path<String>,
    AxumState(ctx): AxumState<BridgeCtx>,
    Json(body): Json<Json_>,
) -> impl IntoResponse {
    let state = ctx.app.state::<SharedState>();
    let app = ctx.app.clone();

    macro_rules! call {
        ($result:expr) => {
            return ok_response($result)
        };
    }

    match cmd.as_str() {
        "get_state" => call!(commands::get_state(state)),
        "select_macro" => {
            let index: usize = match field(&body, "index") {
                Ok(v) => v,
                Err(e) => return ok_response::<()>(Err(e)),
            };
            call!(commands::select_macro(state, app, index))
        }
        "new_macro" => call!(commands::new_macro(state, app)),
        "remove_macro" => call!(commands::remove_macro(state, app)),
        "set_title" => {
            let title: String = match field(&body, "title") {
                Ok(v) => v,
                Err(e) => return ok_response::<()>(Err(e)),
            };
            call!(commands::set_title(state, app, title))
        }
        "set_macro_speed_multiplier" => {
            let multiplier: f64 = match field(&body, "multiplier") {
                Ok(v) => v,
                Err(e) => return ok_response::<()>(Err(e)),
            };
            call!(commands::set_macro_speed_multiplier(state, app, multiplier))
        }
        "save_macro" => call!(commands::save_macro(state, app)),
        "export_macro" => {
            let macro_id: String = match field(&body, "macroId") {
                Ok(v) => v,
                Err(e) => return ok_response::<()>(Err(e)),
            };
            call!(commands::export_macro(macro_id).await)
        }
        "import_macro" => call!(commands::import_macro(state, app).await),
        "add_instruction" => {
            let strand_id: String = match field(&body, "strandId") {
                Ok(v) => v,
                Err(e) => return ok_response::<()>(Err(e)),
            };
            let index: Vec<PathStep> = match field(&body, "path") {
                Ok(v) => v,
                Err(e) => return ok_response::<()>(Err(e)),
            };
            let instruction: InstructionDto = match field(&body, "instruction") {
                Ok(v) => v,
                Err(e) => return ok_response::<()>(Err(e)),
            };
            call!(commands::add_instruction(
                state,
                app,
                strand_id,
                index,
                instruction
            ))
        }
        "edit_instruction" => {
            let strand_id: String = match field(&body, "strandId") {
                Ok(v) => v,
                Err(e) => return ok_response::<()>(Err(e)),
            };
            let index: Vec<PathStep> = match field(&body, "path") {
                Ok(v) => v,
                Err(e) => return ok_response::<()>(Err(e)),
            };
            let instruction: InstructionDto = match field(&body, "instruction") {
                Ok(v) => v,
                Err(e) => return ok_response::<()>(Err(e)),
            };
            call!(commands::edit_instruction(
                state,
                app,
                strand_id,
                index,
                instruction
            ))
        }
        "create_variable" => {
            let name: String = match field(&body, "name") {
                Ok(v) => v,
                Err(e) => return ok_response::<()>(Err(e)),
            };
            call!(commands::create_variable(state, app, name))
        }
        "rename_variable" => {
            let old_name: String = match field(&body, "oldName") {
                Ok(v) => v,
                Err(e) => return ok_response::<()>(Err(e)),
            };
            let new_name: String = match field(&body, "newName") {
                Ok(v) => v,
                Err(e) => return ok_response::<()>(Err(e)),
            };
            call!(commands::rename_variable(state, app, old_name, new_name))
        }
        "delete_variable" => {
            let name: String = match field(&body, "name") {
                Ok(v) => v,
                Err(e) => return ok_response::<()>(Err(e)),
            };
            call!(commands::delete_variable(state, app, name))
        }
        "create_block" => {
            let pieces: Vec<BlockPieceDto> = match field(&body, "pieces") {
                Ok(v) => v,
                Err(e) => return ok_response::<()>(Err(e)),
            };
            let shape: BlockShape = match field(&body, "shape") {
                Ok(v) => v,
                Err(e) => return ok_response::<()>(Err(e)),
            };
            let color: String = match field(&body, "color") {
                Ok(v) => v,
                Err(e) => return ok_response::<()>(Err(e)),
            };
            call!(commands::create_block(state, app, pieces, shape, color))
        }
        "edit_block" => {
            let block_id: String = match field(&body, "blockId") {
                Ok(v) => v,
                Err(e) => return ok_response::<()>(Err(e)),
            };
            let pieces: Vec<BlockPieceDto> = match field(&body, "pieces") {
                Ok(v) => v,
                Err(e) => return ok_response::<()>(Err(e)),
            };
            let shape: BlockShape = match field(&body, "shape") {
                Ok(v) => v,
                Err(e) => return ok_response::<()>(Err(e)),
            };
            let color: String = match field(&body, "color") {
                Ok(v) => v,
                Err(e) => return ok_response::<()>(Err(e)),
            };
            call!(commands::edit_block(
                state, app, block_id, pieces, shape, color
            ))
        }
        "delete_block" => {
            let block_id: String = match field(&body, "blockId") {
                Ok(v) => v,
                Err(e) => return ok_response::<()>(Err(e)),
            };
            call!(commands::delete_block(state, app, block_id))
        }
        "edit_value_field" => {
            let location: ValueLocationDto = match field(&body, "location") {
                Ok(v) => v,
                Err(e) => return ok_response::<()>(Err(e)),
            };
            let text: String = match field(&body, "text") {
                Ok(v) => v,
                Err(e) => return ok_response::<()>(Err(e)),
            };
            call!(commands::edit_value_field(state, app, location, text))
        }
        "set_value_kind" => {
            let location: ValueLocationDto = match field(&body, "location") {
                Ok(v) => v,
                Err(e) => return ok_response::<()>(Err(e)),
            };
            let kind: String = match field(&body, "kind") {
                Ok(v) => v,
                Err(e) => return ok_response::<()>(Err(e)),
            };
            call!(commands::set_value_kind(state, app, location, kind))
        }
        "take_value" => {
            let location: ValueLocationDto = match field(&body, "location") {
                Ok(v) => v,
                Err(e) => return ok_response::<()>(Err(e)),
            };
            call!(commands::take_value(state, app, location))
        }
        "put_value" => {
            let location: ValueLocationDto = match field(&body, "location") {
                Ok(v) => v,
                Err(e) => return ok_response::<()>(Err(e)),
            };
            let value: ValueDto = match field(&body, "value") {
                Ok(v) => v,
                Err(e) => return ok_response::<()>(Err(e)),
            };
            call!(commands::put_value(state, app, location, value))
        }
        "preview_value" => {
            let value: ValueDto = match field(&body, "value") {
                Ok(v) => v,
                Err(e) => return ok_response::<()>(Err(e)),
            };
            call!(commands::preview_value(state, value))
        }
        "create_floating_value" => {
            let x: i32 = match field(&body, "x") {
                Ok(v) => v,
                Err(e) => return ok_response::<()>(Err(e)),
            };
            let y: i32 = match field(&body, "y") {
                Ok(v) => v,
                Err(e) => return ok_response::<()>(Err(e)),
            };
            let value: ValueDto = match field(&body, "value") {
                Ok(v) => v,
                Err(e) => return ok_response::<()>(Err(e)),
            };
            let origin_block_id: Option<String> = match field(&body, "originBlockId") {
                Ok(v) => v,
                Err(e) => return ok_response::<()>(Err(e)),
            };
            call!(commands::create_floating_value(
                state,
                app,
                x,
                y,
                value,
                origin_block_id
            ))
        }
        "move_floating_value" => {
            let floating_id: String = match field(&body, "floatingId") {
                Ok(v) => v,
                Err(e) => return ok_response::<()>(Err(e)),
            };
            let x: i32 = match field(&body, "x") {
                Ok(v) => v,
                Err(e) => return ok_response::<()>(Err(e)),
            };
            let y: i32 = match field(&body, "y") {
                Ok(v) => v,
                Err(e) => return ok_response::<()>(Err(e)),
            };
            call!(commands::move_floating_value(state, app, floating_id, x, y))
        }
        "remove_floating_value" => {
            let floating_id: String = match field(&body, "floatingId") {
                Ok(v) => v,
                Err(e) => return ok_response::<()>(Err(e)),
            };
            call!(commands::remove_floating_value(state, app, floating_id))
        }
        "remove_instruction" => {
            let strand_id: String = match field(&body, "strandId") {
                Ok(v) => v,
                Err(e) => return ok_response::<()>(Err(e)),
            };
            let index: Vec<PathStep> = match field(&body, "path") {
                Ok(v) => v,
                Err(e) => return ok_response::<()>(Err(e)),
            };
            call!(commands::remove_instruction(state, app, strand_id, index))
        }
        "reorder_instruction" => {
            let strand_id: String = match field(&body, "strandId") {
                Ok(v) => v,
                Err(e) => return ok_response::<()>(Err(e)),
            };
            let index: Vec<PathStep> = match field(&body, "path") {
                Ok(v) => v,
                Err(e) => return ok_response::<()>(Err(e)),
            };
            let direction: i32 = match field(&body, "direction") {
                Ok(v) => v,
                Err(e) => return ok_response::<()>(Err(e)),
            };
            call!(commands::reorder_instruction(
                state, app, strand_id, index, direction
            ))
        }
        "clear_instructions" => call!(commands::clear_instructions(state, app)),
        "add_strand" => {
            let x: Option<i32> = match field(&body, "x") {
                Ok(v) => v,
                Err(e) => return ok_response::<()>(Err(e)),
            };
            let y: Option<i32> = match field(&body, "y") {
                Ok(v) => v,
                Err(e) => return ok_response::<()>(Err(e)),
            };
            let instruction: Option<InstructionDto> = match field(&body, "instruction") {
                Ok(v) => v,
                Err(e) => return ok_response::<()>(Err(e)),
            };
            call!(commands::add_strand(state, app, x, y, instruction))
        }
        "remove_strand" => {
            let strand_id: String = match field(&body, "strandId") {
                Ok(v) => v,
                Err(e) => return ok_response::<()>(Err(e)),
            };
            call!(commands::remove_strand(state, app, strand_id))
        }
        "move_strand" => {
            let strand_id: String = match field(&body, "strandId") {
                Ok(v) => v,
                Err(e) => return ok_response::<()>(Err(e)),
            };
            let x: i32 = match field(&body, "x") {
                Ok(v) => v,
                Err(e) => return ok_response::<()>(Err(e)),
            };
            let y: i32 = match field(&body, "y") {
                Ok(v) => v,
                Err(e) => return ok_response::<()>(Err(e)),
            };
            call!(commands::move_strand(state, app, strand_id, x, y))
        }
        "split_strand" => {
            let strand_id: String = match field(&body, "strandId") {
                Ok(v) => v,
                Err(e) => return ok_response::<()>(Err(e)),
            };
            let index: Vec<PathStep> = match field(&body, "path") {
                Ok(v) => v,
                Err(e) => return ok_response::<()>(Err(e)),
            };
            let x: i32 = match field(&body, "x") {
                Ok(v) => v,
                Err(e) => return ok_response::<()>(Err(e)),
            };
            let y: i32 = match field(&body, "y") {
                Ok(v) => v,
                Err(e) => return ok_response::<()>(Err(e)),
            };
            call!(commands::split_strand(state, app, strand_id, index, x, y))
        }
        "merge_strand" => {
            let dragged_id: String = match field(&body, "draggedId") {
                Ok(v) => v,
                Err(e) => return ok_response::<()>(Err(e)),
            };
            let target_id: String = match field(&body, "targetId") {
                Ok(v) => v,
                Err(e) => return ok_response::<()>(Err(e)),
            };
            let index: Vec<PathStep> = match field(&body, "path") {
                Ok(v) => v,
                Err(e) => return ok_response::<()>(Err(e)),
            };
            call!(commands::merge_strand(
                state, app, dragged_id, target_id, index
            ))
        }
        "undo" => call!(commands::undo(state, app)),
        "redo" => call!(commands::redo(state, app)),
        "delete_instruction" => {
            let strand_id: String = match field(&body, "strandId") {
                Ok(v) => v,
                Err(e) => return ok_response::<()>(Err(e)),
            };
            let index: Vec<PathStep> = match field(&body, "path") {
                Ok(v) => v,
                Err(e) => return ok_response::<()>(Err(e)),
            };
            let x: i32 = match field(&body, "x") {
                Ok(v) => v,
                Err(e) => return ok_response::<()>(Err(e)),
            };
            let y: i32 = match field(&body, "y") {
                Ok(v) => v,
                Err(e) => return ok_response::<()>(Err(e)),
            };
            call!(commands::delete_instruction(
                state, app, strand_id, index, x, y
            ))
        }
        "paste_instructions" => {
            let x: i32 = match field(&body, "x") {
                Ok(v) => v,
                Err(e) => return ok_response::<()>(Err(e)),
            };
            let y: i32 = match field(&body, "y") {
                Ok(v) => v,
                Err(e) => return ok_response::<()>(Err(e)),
            };
            let instructions: Vec<InstructionDto> = match field(&body, "instructions") {
                Ok(v) => v,
                Err(e) => return ok_response::<()>(Err(e)),
            };
            call!(commands::paste_instructions(state, app, x, y, instructions))
        }
        "set_recording_target" => {
            let strand_id: String = match field(&body, "strandId") {
                Ok(v) => v,
                Err(e) => return ok_response::<()>(Err(e)),
            };
            call!(commands::set_recording_target(state, app, strand_id))
        }
        "start_key_capture" => {
            let strand_id: String = match field(&body, "strandId") {
                Ok(v) => v,
                Err(e) => return ok_response::<()>(Err(e)),
            };
            let index: Vec<PathStep> = match field(&body, "path") {
                Ok(v) => v,
                Err(e) => return ok_response::<()>(Err(e)),
            };
            call!(commands::start_key_capture(state, app, strand_id, index))
        }
        "key_capture_event" => {
            let code: String = match field(&body, "code") {
                Ok(v) => v,
                Err(e) => return ok_response::<()>(Err(e)),
            };
            let key: String = match field(&body, "key") {
                Ok(v) => v,
                Err(e) => return ok_response::<()>(Err(e)),
            };
            call!(commands::key_capture_event(state, app, code, key))
        }
        "run_macro" => call!(commands::run_macro(state, app)),
        "toggle_loop_mode" => {
            let enabled: bool = match field(&body, "enabled") {
                Ok(v) => v,
                Err(e) => return ok_response::<()>(Err(e)),
            };
            call!(commands::toggle_loop_mode(state, app, enabled))
        }
        "set_global_speed_multiplier" => {
            let multiplier: f64 = match field(&body, "multiplier") {
                Ok(v) => v,
                Err(e) => return ok_response::<()>(Err(e)),
            };
            call!(commands::set_global_speed_multiplier(
                state, app, multiplier
            ))
        }
        "start_recording" => call!(commands::start_recording(state, app)),
        "stop_recording" => call!(commands::stop_recording(state, app)),
        "toggle_record_mouse_relative" => {
            let relative: bool = match field(&body, "relative") {
                Ok(v) => v,
                Err(e) => return ok_response::<()>(Err(e)),
            };
            call!(commands::toggle_record_mouse_relative(state, app, relative))
        }
        "toggle_record_mouse_movement" => {
            let enabled: bool = match field(&body, "enabled") {
                Ok(v) => v,
                Err(e) => return ok_response::<()>(Err(e)),
            };
            call!(commands::toggle_record_mouse_movement(state, app, enabled))
        }
        "open_settings" => call!(commands::open_settings(state, app)),
        "close_settings" => call!(commands::close_settings(state, app)),
        "reset_zoom" => call!(commands::reset_zoom(app)),
        "start_combo_capture" => {
            let action: HotkeyActionDto = match field(&body, "action") {
                Ok(v) => v,
                Err(e) => return ok_response::<()>(Err(e)),
            };
            call!(commands::start_combo_capture(state, app, action))
        }
        "start_pending_combo_capture" => call!(commands::start_pending_combo_capture(state, app)),
        "combo_capture_event" => {
            let code: String = match field(&body, "code") {
                Ok(v) => v,
                Err(e) => return ok_response::<()>(Err(e)),
            };
            let modifiers: u8 = match field(&body, "modifiers") {
                Ok(v) => v,
                Err(e) => return ok_response::<()>(Err(e)),
            };
            call!(commands::combo_capture_event(state, app, code, modifiers))
        }
        "cancel_combo_capture" => call!(commands::cancel_combo_capture(state, app)),
        "set_pending_macro_idx" => {
            let index: Option<usize> = match field(&body, "index") {
                Ok(v) => v,
                Err(e) => return ok_response::<()>(Err(e)),
            };
            call!(commands::set_pending_macro_idx(state, app, index))
        }
        "add_macro_hotkey" => call!(commands::add_macro_hotkey(state, app)),
        "remove_hotkey_binding" => {
            let index: usize = match field(&body, "index") {
                Ok(v) => v,
                Err(e) => return ok_response::<()>(Err(e)),
            };
            call!(commands::remove_hotkey_binding(state, app, index))
        }
        "clear_named_hotkey" => {
            let action: HotkeyActionDto = match field(&body, "action") {
                Ok(v) => v,
                Err(e) => return ok_response::<()>(Err(e)),
            };
            call!(commands::clear_named_hotkey(state, app, action))
        }
        "reset_hotkey_to_default" => {
            let action: HotkeyActionDto = match field(&body, "action") {
                Ok(v) => v,
                Err(e) => return ok_response::<()>(Err(e)),
            };
            call!(commands::reset_hotkey_to_default(state, app, action))
        }
        "set_ipc_port_text" => {
            let text: String = match field(&body, "text") {
                Ok(v) => v,
                Err(e) => return ok_response::<()>(Err(e)),
            };
            call!(commands::set_ipc_port_text(state, app, text))
        }
        "start_ipc_server" => call!(commands::start_ipc_server(state, app).await),
        "stop_ipc_server" => call!(commands::stop_ipc_server(state, app)),
        "set_ipc_auto_start" => {
            let enabled: bool = match field(&body, "enabled") {
                Ok(v) => v,
                Err(e) => return ok_response::<()>(Err(e)),
            };
            call!(commands::set_ipc_auto_start(state, app, enabled))
        }
        "check_for_updates" => call!(commands::check_for_updates(state, app)),
        "apply_update" => call!(commands::apply_update(state, app)),
        "list_installed_apps" => call!(Ok(commands::list_installed_apps().await)),
        other => ok_response::<()>(Err(format!("unknown command: {other}"))),
    }
}
