//! The opt-in webhook server: a request to `http://127.0.0.1:<port>/<path>`
//! carrying the token fires the live macros' `WhenWebhook` strands for that
//! path. Only listens on this computer.

use crate::scheduled_run;
use crate::state::{emit_state_updated, SharedState};
use crate::AppHandle;
use blockwork_core::config::{load_settings, update_settings};
use blockwork_core::input::value::Evaluated;
use blockwork_core::macros::InstructionKind;
use serde::Serialize;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

const DEFAULT_PORT: u16 = 8787;
const MAX_BODY: usize = 1 << 20;
const MAX_HEADER_LINES: usize = 100;

/// Bumped on every settings change so the server rebinds.
static GENERATION: AtomicU64 = AtomicU64::new(0);
/// Why the server isn't listening, if it should be.
static PROBLEM: Mutex<String> = Mutex::new(String::new());

#[derive(Serialize, Clone)]
pub(crate) struct WebhookDto {
    enabled: bool,
    port: u16,
    token: String,
    /// Set when the port couldn't be opened.
    error: String,
}

pub(crate) fn dto() -> WebhookDto {
    let settings = load_settings();
    WebhookDto {
        enabled: settings.webhook_enabled.unwrap_or(false),
        port: settings.webhook_port.unwrap_or(DEFAULT_PORT),
        token: settings.webhook_token.unwrap_or_default(),
        error: PROBLEM.lock().map(|p| p.clone()).unwrap_or_default(),
    }
}

fn fresh_token() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

pub(crate) fn configure(state: &SharedState, app: &AppHandle, enabled: bool, port: u16) -> Result<(), String> {
    if port == 0 {
        return Err("pick a port from 1 to 65535".into());
    }
    update_settings(|s| {
        s.webhook_enabled = Some(enabled);
        s.webhook_port = Some(port);
        if s.webhook_token.as_deref().is_none_or(str::is_empty) {
            s.webhook_token = Some(fresh_token());
        }
    });
    restart(state, app)
}

pub(crate) fn new_token(state: &SharedState, app: &AppHandle) -> Result<(), String> {
    update_settings(|s| s.webhook_token = Some(fresh_token()));
    restart(state, app)
}

/// The server publishes state once it has rebound.
fn restart(_state: &SharedState, _app: &AppHandle) -> Result<(), String> {
    GENERATION.fetch_add(1, Ordering::SeqCst);
    Ok(())
}

fn set_problem(text: String, state: &SharedState, app: &AppHandle) {
    if let Ok(mut problem) = PROBLEM.lock() {
        *problem = text;
    }
    if let Ok(s) = state.lock() {
        emit_state_updated(app, &s);
    }
}

pub(crate) fn start(state: SharedState, app: AppHandle) {
    let _ = std::thread::Builder::new().name("webhook".into()).spawn(move || serve(state, app));
}

fn serve(state: SharedState, app: AppHandle) {
    loop {
        let generation = GENERATION.load(Ordering::SeqCst);
        let settings = load_settings();
        let changed = || GENERATION.load(Ordering::SeqCst) != generation;
        if settings.webhook_enabled.unwrap_or(false) {
            let port = settings.webhook_port.unwrap_or(DEFAULT_PORT);
            match TcpListener::bind(("127.0.0.1", port)).and_then(|l| l.set_nonblocking(true).map(|_| l)) {
                Ok(listener) => {
                    set_problem(String::new(), &state, &app);
                    let token = settings.webhook_token.unwrap_or_default();
                    while !changed() {
                        match listener.accept() {
                            Ok((stream, _)) => {
                                let (state, app, token) = (Arc::clone(&state), app.clone(), token.clone());
                                std::thread::spawn(move || handle(stream, &token, &state, &app));
                            }
                            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => std::thread::sleep(Duration::from_millis(100)),
                            Err(e) => tracing::warn!("Webhook accept failed: {e}"),
                        }
                    }
                    continue;
                }
                Err(e) => set_problem(format!("Couldn't listen on port {port}: {e}"), &state, &app),
            }
        } else {
            set_problem(String::new(), &state, &app);
        }
        while !changed() {
            std::thread::sleep(Duration::from_millis(200));
        }
    }
}

struct Request {
    method: String,
    path: String,
    query: HashMap<String, String>,
    headers: HashMap<String, String>,
    body: String,
}

fn read_request(stream: &TcpStream) -> Result<Request, (u16, &'static str)> {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line).map_err(|_| (400, "bad request"))?;
    let mut parts = line.split_whitespace();
    let (Some(method), Some(target)) = (parts.next(), parts.next()) else { return Err((400, "bad request")) };
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    let query = query
        .split('&')
        .filter_map(|pair| pair.split_once('=').or(Some((pair, ""))))
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    let mut headers = HashMap::new();
    for _ in 0..MAX_HEADER_LINES {
        let mut header = String::new();
        reader.read_line(&mut header).map_err(|_| (400, "bad request"))?;
        let header = header.trim_end();
        if header.is_empty() {
            break;
        }
        if let Some((name, value)) = header.split_once(':') {
            headers.insert(name.trim().to_ascii_lowercase(), value.trim().to_string());
        }
    }
    let length: usize = headers.get("content-length").and_then(|l| l.parse().ok()).unwrap_or(0);
    if length > MAX_BODY {
        return Err((413, "the body is too large"));
    }
    let mut body = vec![0; length];
    reader.read_exact(&mut body).map_err(|_| (400, "the body was cut short"))?;
    Ok(Request {
        method: method.to_ascii_uppercase(),
        path: path.trim_matches('/').to_string(),
        query,
        headers,
        body: String::from_utf8_lossy(&body).into_owned(),
    })
}

fn respond(mut stream: &TcpStream, status: u16, body: serde_json::Value) {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        401 => "Unauthorized",
        404 => "Not Found",
        405 => "Method Not Allowed",
        413 => "Payload Too Large",
        _ => "Error",
    };
    let body = body.to_string();
    let _ = write!(
        stream,
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
}

fn handle(stream: TcpStream, token: &str, state: &SharedState, app: &AppHandle) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let request = match read_request(&stream) {
        Ok(request) => request,
        Err((status, message)) => return respond(&stream, status, serde_json::json!({ "error": message })),
    };
    if !matches!(request.method.as_str(), "GET" | "POST" | "PUT") {
        return respond(&stream, 405, serde_json::json!({ "error": "use GET, POST or PUT" }));
    }
    let given = request
        .headers
        .get("x-blockwork-token")
        .cloned()
        .or_else(|| request.headers.get("authorization").and_then(|a| a.strip_prefix("Bearer ")).map(str::to_string))
        .or_else(|| request.query.get("token").cloned())
        .unwrap_or_default();
    if token.is_empty() || given != token {
        return respond(&stream, 401, serde_json::json!({ "error": "missing or wrong token" }));
    }
    let fired = fire(&request.path, &request.body, state, app);
    if fired == 0 {
        return respond(&stream, 404, serde_json::json!({ "error": format!("no macro listens on /{}", request.path) }));
    }
    respond(&stream, 200, serde_json::json!({ "fired": fired }));
}

/// Fires every live `WhenWebhook` strand for `path`; returns how many.
fn fire(path: &str, body: &str, state: &SharedState, app: &AppHandle) -> usize {
    let mut fires = Vec::new();
    let (emulator, speed) = {
        let Ok(s) = state.lock() else { return 0 };
        let Some(emulator) = s.emulator.clone() else { return 0 };
        for mac in scheduled_run::live_macros(&s) {
            let env: HashMap<_, _> = mac.variables.iter().map(|v| (v.name.clone(), v.value.clone())).collect();
            for strand in &mac.strands {
                let Some(InstructionKind::WhenWebhook { path: listens, variable }) = strand.instructions.first().map(|i| &i.kind) else {
                    continue;
                };
                let listens = listens.resolve_vars(&env).eval_text().unwrap_or_default();
                if listens.trim().trim_matches('/') != path {
                    continue;
                }
                let mut variables = mac.variables.clone();
                if let Some(var) = variables.iter_mut().find(|v| v.name == *variable) {
                    var.value = Evaluated::Text(body.to_string());
                }
                fires.push((mac.id.clone(), strand.instructions[1..].to_vec(), variables, mac.lists.clone()));
            }
        }
        (emulator, s.global_speed_multiplier)
    };
    let count = fires.len();
    for (id, body, variables, lists) in fires {
        scheduled_run::fire(id, body, Arc::clone(&emulator), speed, &variables, &lists, Arc::clone(state), app.clone());
    }
    count
}
