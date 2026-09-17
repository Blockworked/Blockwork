//! Debug-only HTTP+WebSocket bridge (feature-gated, never in release builds)
//! so the running backend can be driven from a browser tab instead of the
//! Tauri webview. `POST /invoke/:cmd` runs `Backend::dispatch`; `GET /events`
//! streams the state snapshot on connect and after every change. Binds to
//! 127.0.0.1 only.

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use blockwork_app::{Backend, Event};
use serde_json::Value;
use tokio::sync::broadcast::error::RecvError;
use tower_http::cors::CorsLayer;
use tracing::{info, warn};

const PORT: u16 = 4127;

pub(crate) async fn run(backend: Backend) {
    let router = Router::new()
        .route("/invoke/{cmd}", post(invoke_handler))
        .route("/events", get(ws_handler))
        .layer(CorsLayer::permissive())
        .with_state(backend);

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

async fn ws_handler(ws: WebSocketUpgrade, State(backend): State<Backend>) -> Response {
    ws.on_upgrade(move |socket| handle_socket(socket, backend))
}

async fn handle_socket(mut socket: WebSocket, backend: Backend) {
    let mut events = backend.subscribe();
    // Snapshot on connect so a freshly-opened tab sees state immediately.
    if let Ok(initial) = backend.state_json() {
        if socket.send(Message::Text(initial.into())).await.is_err() {
            return;
        }
    }

    loop {
        tokio::select! {
            event = events.recv() => {
                let payload = match event {
                    Ok(Event::State(json)) => json.to_string(),
                    Ok(_) => continue,
                    Err(RecvError::Lagged(_)) => match backend.state_json() {
                        Ok(json) => json,
                        Err(_) => continue,
                    },
                    Err(RecvError::Closed) => return,
                };
                if socket.send(Message::Text(payload.into())).await.is_err() {
                    return;
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

async fn invoke_handler(
    Path(cmd): Path<String>,
    State(backend): State<Backend>,
    Json(args): Json<Value>,
) -> impl IntoResponse {
    match backend.dispatch(&cmd, args).await {
        Ok(data) => (
            StatusCode::OK,
            Json(serde_json::json!({"ok": true, "data": data})),
        ),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"ok": false, "error": e})),
        ),
    }
}
