//! Native UI polls this mailbox. Waiting happens only on macro threads;
//! cancellation removes the prompt even when no activity is attached.

use blockwork_core::prompt::{Reply, Request};
use serde_json::{Value, json};
use std::sync::{Condvar, Mutex};
use std::time::Duration;

struct Pending {
    id: u64,
    request: Request,
    reply: Option<Reply>,
}

static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
static PENDING: Mutex<Option<Pending>> = Mutex::new(None);
static ANSWERED: Condvar = Condvar::new();

pub fn ask(request: &Request, stopped: &dyn Fn() -> bool) -> Result<Reply, String> {
    let mut pending = PENDING.lock().map_err(|e| e.to_string())?;
    let id = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    *pending = Some(Pending {
        id,
        request: request.clone(),
        reply: None,
    });
    loop {
        if stopped() {
            *pending = None;
            return Err("stopped".into());
        }
        if let Some(reply) = pending.as_mut().and_then(|p| p.reply.take()) {
            *pending = None;
            return Ok(reply);
        }
        pending = ANSWERED
            .wait_timeout(pending, Duration::from_millis(50))
            .map_err(|e| e.to_string())?
            .0;
    }
}

pub fn pending() -> Result<Value, String> {
    Ok(PENDING
        .lock()
        .map_err(|e| e.to_string())?
        .as_ref()
        .map(|p| json!({"id": p.id, "request": p.request}))
        .unwrap_or(Value::Null))
}

pub fn answer(id: u64, reply: Reply) -> Result<Value, String> {
    let mut pending = PENDING.lock().map_err(|e| e.to_string())?;
    let prompt = pending
        .as_mut()
        .filter(|p| p.id == id)
        .ok_or("prompt was cancelled or already answered")?;
    if prompt.reply.is_some() {
        return Err("prompt was already answered".into());
    }
    prompt.reply = Some(reply);
    ANSWERED.notify_all();
    Ok(Value::Null)
}
