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
        .map(|(k, v)| (percent_decode(k, true), percent_decode(v, true)))
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
        path: percent_decode(path.trim_matches('/'), false),
        query,
        headers,
        body: String::from_utf8_lossy(&body).into_owned(),
    })
}

/// Decodes `%XX` escapes, and in a query `+` (a space), so a token or path
/// a client URL-encoded still matches.
fn percent_decode(text: &str, query: bool) -> String {
    let bytes = text.as_bytes();
    let hex = |b: u8| (b as char).to_digit(16);
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                if let (Some(h), Some(l)) = (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                    out.push((h * 16 + l) as u8);
                    i += 3;
                    continue;
                }
                out.push(b'%');
            }
            b'+' if query => out.push(b' '),
            b => out.push(b),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Compares in time that doesn't depend on where the first difference is,
/// so the token can't be guessed a character at a time.
fn same_token(given: &str, token: &str) -> bool {
    let (a, b) = (given.as_bytes(), token.as_bytes());
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |diff, (x, y)| diff | (x ^ y)) == 0
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

fn prepare(stream: &TcpStream) {
    // On Windows an accepted socket inherits the listener's non-blocking
    // mode, and a request that arrives in pieces would fail to read.
    let _ = stream.set_nonblocking(false);
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
}

fn handle(stream: TcpStream, token: &str, state: &SharedState, app: &AppHandle) {
    prepare(&stream);
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
    if token.is_empty() || !same_token(&given, token) {
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
                let mut input = None;
                if let Some(var) = variables.iter_mut().find(|v| v.name == *variable) {
                    input = Some((var.name.clone(), var.value.clone()));
                    var.value = Evaluated::Text(body.to_string());
                }
                fires.push((mac.id.clone(), strand.instructions[1..].to_vec(), variables, mac.lists.clone(), input));
            }
        }
        (emulator, s.global_speed_multiplier)
    };
    let count = fires.len();
    for (id, body, variables, lists, input) in fires {
        scheduled_run::fire_with_input(id, body, Arc::clone(&emulator), speed, &variables, &lists, Arc::clone(state), app.clone(), input);
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_slow_request_from_a_nonblocking_listener() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        listener.set_nonblocking(true).unwrap();
        let port = listener.local_addr().unwrap().port();
        let client = std::thread::spawn(move || {
            let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
            stream.write_all(b"POST /deploy HTTP/1.1\r\n").unwrap();
            std::thread::sleep(Duration::from_millis(100));
            stream.write_all(b"Content-Length: 2\r\n\r\nhi").unwrap();
            std::thread::sleep(Duration::from_millis(200));
        });
        let stream = loop {
            match listener.accept() {
                Ok((stream, _)) => break stream,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => std::thread::sleep(Duration::from_millis(10)),
                Err(e) => panic!("{e}"),
            }
        };
        prepare(&stream);
        let request = read_request(&stream).map_err(|(status, _)| status).unwrap();
        assert_eq!((request.method.as_str(), request.path.as_str(), request.body.as_str()), ("POST", "deploy", "hi"));
        client.join().unwrap();
    }

    #[test]
    fn decodes_queries_and_compares_tokens() {
        assert_eq!(percent_decode("a%2Bb+c%zz%4", true), "a+b c%zz%4");
        assert_eq!(percent_decode("deploy%20now+x", false), "deploy now+x");
        assert!(same_token("abc", "abc"));
        assert!(!same_token("abd", "abc") && !same_token("ab", "abc") && !same_token("", "abc"));
    }
}
