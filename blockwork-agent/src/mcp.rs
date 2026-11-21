//! stdio MCP server (protocol 2024-11-05). Tools come from `tools.rs`;
//! resources and prompts are the documented four. `agent_wait_run` is
//! client-side polling (`run_status` + `run_log`), no server verb.

use crate::client::DaemonClient;
use crate::tools::tool_table;
use serde_json::{Value, json};
use std::io::Write;

pub async fn run() -> Result<(), String> {
    let mut client = DaemonClient::connect().await.map_err(|e| e.to_string())?;
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut input = stdin.lock();
    let mut out = stdout.lock();
    for line in std::io::BufRead::lines(&mut input) {
        let line = line.map_err(|e| e.to_string())?;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let req: Value = serde_json::from_str(line).map_err(|e| format!("bad json: {e}"))?;
        let id = req.get("id").cloned().unwrap_or(Value::Null);
        let method = req.get("method").and_then(|v| v.as_str()).unwrap_or("");
        let params = req.get("params").cloned().unwrap_or(Value::Null);
        let is_notification = req.get("id").is_none();
        let reply = match method {
            "initialize" => Some(json!({
                "jsonrpc": "2.0", "id": id,
                "result": {
                    "protocolVersion": "2024-11-05",
                    "capabilities": {"tools": {}, "resources": {}, "prompts": {}},
                    "serverInfo": {"name": "blockwork-agent", "version": env!("CARGO_PKG_VERSION")},
                    "instructions":
                    "Blockwork is the local macro editor plus a desktop harness; \
                    reach for it when a task needs the screen or synthetic input, \
                    or when a flow should become a saved macro. \
                    See: screenshot returns a temp PNG path with origin/scale \
                    and downscale (delivered pixels are downscaled by default; \
                    screen_x = origin_x + model_x * downscale / scale, \
                    full_res:true gives native); the path is the delivery -- read \
                    the file with your native image-reading (e.g. a Read tool on \
                    the path) to see it; ocr reads \
                    a region as text; find_image locates a template PNG and \
                    returns a screen-space center; pixel and cursor verify \
                    positions. Act: move_mouse/move_mouse_rel/click/scroll/key/type_text/button \
                    are gated by the daemon (FORBIDDEN until agent_enabled and \
                    agent_input_allowed are on; BUSY while a macro drives input \
                    unless force is set). Standard loop: screenshot -> \
                    find_image/ocr -> click -> screenshot to verify. For \
                    anything done twice, prefer building a macro \
                    (describe_blocks, create_macro, add_block/replace_canvas, \
                    run_macro, read run_log) over raw synthetic input; \
                    agent-created macros start restricted. window tools: \
                    windows/active_window/window_bounds, window_action focuses, \
                    window_move moves. On Wayland call move_mouse before cursor or \
                    pixel reads (cursor is tracked, not queried). \
                    Every act verb (move/click/scroll/key/type/button) accepts \
                    expect: an app/title substring the focused window must match, \
                    checked in the daemon right before the input fires - a \
                    mismatch returns FORBIDDEN instead of typing into the wrong \
                    app. Results include after:{active, cursor} receipts for \
                    verification. Pass expect whenever focus could plausibly \
                    be elsewhere (e.g. after a launch, a dialog, or in any app \
                    besides the one you just verified).\
                    Coordinate discipline: do not estimate icon/button centers \
                    from a downscaled screenshot by eye - that is the main cause \
                    of missed clicks. Instead crop a tight region around the \
                    target, Read the crop PNG to confirm it shows the intended \
                    widget, then pass its path to find_image (same-frame crop \
                    returns its own origin as a self-match - exclude it or \
                    use region/all and verify diff), then click the \
                    returned center and verify with cursor or a fresh screenshot. \
                    Before clicking, move to the target first so hover/focus \
                    settles, and verify with cursor or screenshot after the action \
                    - a click reports ok even when it misses. Use window_action \
                    Focus when focus may be elsewhere, and prefer standard \
                    shortcuts (Control+s/n, Alt+f menus) over mouse work \
                    when they are known. Prefer launch_app over Meta+type+Enter \
                    to open apps. key combos are held chords (Control+n holds \
                    Control), not sequential taps; key with hold_ms or action press/release holds a key down, button press holds until release, and move_mouse_rel rotates view in pointer-lock games. \
                    App identity: never assume a launch worked - after any \
                    launcher/search/Enter action, check active_window and only \
                    proceed when the expected app is focused; pinned taskbar \
                    icons are not self-describing, so disambiguate by checking \
                    windows app/title first. Focus discipline: after any \
                    no-op or wrong-window surprise, stop and read active_window + \
                    screenshot to re-establish state instead of retrying the \
                    same coordinates; an action with no visible effect means \
                    focus, app, or modal state assumed wrong - compare before/ \
                    after each step."
                }
            })),
            "notifications/initialized" => None,
            "ping" => Some(reply(&id, json!({}))),
            "tools/list" => Some(reply(
                &id,
                json!({
                    "tools": tool_table().iter().map(|t| json!({
                        "name": t.mcp,
                        "description": t.description,
                        "inputSchema": t.schema,
                    })).collect::<Vec<_>>()
                }),
            )),
            "tools/call" => {
                let name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
                let args = params.get("arguments").cloned().unwrap_or(json!({}));
                let wire = tool_table().iter().find(|t| t.mcp == name).map(|t| t.wire);
                let wire = match wire {
                    Some(w) => w,
                    None => {
                        return write(
                            &mut out,
                            &error_reply(&id, format!("unknown tool '{name}'")),
                        );
                    }
                };
                // Screenshot: return only the daemon JSON (path + metadata);
                // the agent reads the PNG file with its own image reading.
                let result = client.call(wire, args).await;
                match result {
                    Ok(v) => {
                        let content = vec![json!({"type": "text", "text": v.to_string()})];
                        Some(reply(&id, json!({"content": content, "isError": false})))
                    }
                    Err(e) => Some(reply(
                        &id,
                        json!({
                            "content": [{"type": "text", "text": e}],
                            "isError": true
                        }),
                    )),
                }
            }
            "resources/list" => Some(reply(
                &id,
                json!({
                    "resources": [
                        {"uri": "blockwork://macros", "name": "Macros", "mimeType": "application/json"},
                        {"uri": "blockwork://macro/{id}", "name": "Macro by id", "mimeType": "application/json"},
                        {"uri": "blockwork://state", "name": "Daemon state", "mimeType": "application/json"},
                        {"uri": "blockwork://runlog", "name": "Run log", "mimeType": "text/plain"},
                    ]
                }),
            )),
            "resources/read" => {
                let uri = params.get("uri").and_then(|v| v.as_str()).unwrap_or("");
                let (wire, args) = if uri == "blockwork://macros" {
                    ("agent_list_macros", json!({}))
                } else if uri == "blockwork://state" {
                    ("agent_get_global_settings", json!({}))
                } else if uri == "blockwork://runlog" {
                    ("agent_run_log", json!({"last": 100}))
                } else if let Some(id) = uri.strip_prefix("blockwork://macro/") {
                    ("agent_get_macro", json!({"id": id}))
                } else {
                    return write(
                        &mut out,
                        &error_reply(&id, format!("unknown resource '{uri}'")),
                    );
                };
                match client.call(wire, args).await {
                    Ok(v) => Some(reply(
                        &id,
                        json!({"contents": [{
                            "uri": uri, "mimeType": "application/json", "text": v.to_string()
                        }]}),
                    )),
                    Err(e) => Some(reply(
                        &id,
                        json!({"contents": [{
                            "uri": uri, "mimeType": "text/plain", "text": e
                        }]}),
                    )),
                }
            }
            "prompts/list" => Some(reply(
                &id,
                json!({
                    "prompts": [
                        {"name": "build_macro", "description": "Describe, build, run, read log, iterate", "arguments": [{"name": "description", "required": true}]},
                        {"name": "locate_and_act", "description": "Find, click, screenshot, verify", "arguments": [{"name": "target", "required": true}]},
                    ]
                }),
            )),
            "prompts/get" => {
                let name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
                let hint = match name {
                    "build_macro" => {
                        "Use describe_blocks, create_macro, add_block/replace_canvas, run_macro, then read run_log."
                    }
                    "locate_and_act" => {
                        "Use windows/active_window, screenshot, find_image, then click and screenshot to verify."
                    }
                    _ => "Unknown prompt",
                };
                Some(reply(
                    &id,
                    json!({"messages": [{
                        "role": "user", "content": {"type": "text", "text": hint}
                    }]}),
                ))
            }
            _ if is_notification => None,
            _ => Some(error_reply(&id, format!("method '{method}' not supported"))),
        };
        if let Some(v) = reply {
            write(&mut out, &v)?;
        }
        out.flush().map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn reply(id: &Value, result: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "result": result})
}

fn error_reply(id: &Value, message: String) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": -32601, "message": message}})
}

fn write(out: &mut impl Write, v: &Value) -> Result<(), String> {
    writeln!(out, "{v}").map_err(|e| e.to_string())
}
