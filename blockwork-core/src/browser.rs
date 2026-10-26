//! The browser bridge behind the browser blocks. The Blockwork extension
//! (Blockworked/blockwork-browser) connects to `ws://127.0.0.1:<port>` and
//! carries out requests in the active tab: click an element, type into it,
//! read it, open a page, run a script.
//!
//! Both sides prove they know the pairing key before anything else is said:
//! each answers the other's nonce with HMAC-SHA256(key, "client:"/"server:" +
//! nonce). Only extension origins may connect, so a web page can't pose as
//! the extension, and a program squatting on the port can't drive the browser.

use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, mpsc};
use std::time::{Duration, Instant};
use tracing::warn;

pub const DEFAULT_PORT: u16 = 8788;
const PROTOCOL: u64 = 1;
const MAX_MESSAGE: usize = 16 << 20;
/// The extension pings every 20 s, so a minute of silence is a dead peer.
const IDLE_LIMIT: Duration = Duration::from_secs(60);
const HANDSHAKE_LIMIT: Duration = Duration::from_secs(10);
const ORIGINS: [&str; 3] = ["chrome-extension://", "moz-extension://", "safari-web-extension://"];

/// Bumped on every settings change so the server rebinds.
static GENERATION: AtomicU64 = AtomicU64::new(0);
/// Why the server isn't listening, if it should be.
static PROBLEM: Mutex<String> = Mutex::new(String::new());
static CLIENTS: Mutex<Vec<Arc<Client>>> = Mutex::new(Vec::new());
static NEXT_ID: AtomicU64 = AtomicU64::new(1);
type Listener = Box<dyn Fn() + Send + Sync>;
static ON_CHANGE: OnceLock<Listener> = OnceLock::new();

type Reply = Result<Value, String>;

struct Client {
    browser: String,
    stream: Mutex<TcpStream>,
    pending: Mutex<HashMap<u64, mpsc::Sender<Reply>>>,
    /// When this browser last had a focused window; the newest one gets requests.
    focused_at: Mutex<Option<Instant>>,
    connected_at: Instant,
}

#[derive(Serialize, Clone, Debug)]
pub struct Status {
    pub enabled: bool,
    pub port: u16,
    /// Set when the port couldn't be opened.
    pub error: String,
    /// The connected browsers, like "Firefox 131.0".
    pub browsers: Vec<String>,
}

pub fn status() -> Status {
    let settings = crate::config::load_settings();
    Status {
        enabled: settings.browser_enabled.unwrap_or(false),
        port: settings.browser_port.unwrap_or(DEFAULT_PORT),
        error: PROBLEM.lock().map(|p| p.clone()).unwrap_or_default(),
        browsers: clients().iter().map(|c| c.browser.clone()).collect(),
    }
}

pub fn fresh_key() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

/// The pairing key, made on first use.
pub fn key() -> String {
    let mut key = crate::config::load_settings().browser_key.unwrap_or_default();
    if key.is_empty() {
        key = fresh_key();
        let saved = key.clone();
        crate::config::update_settings(|s| s.browser_key = Some(saved));
    }
    key
}

/// Rebinds with the current settings, dropping connected browsers.
pub fn restart() {
    GENERATION.fetch_add(1, Ordering::SeqCst);
}

/// Starts the server thread; `on_change` runs whenever `status()` changes.
pub fn start(on_change: impl Fn() + Send + Sync + 'static) {
    let _ = ON_CHANGE.set(Box::new(on_change));
    let _ = std::thread::Builder::new().name("browser-bridge".into()).spawn(serve);
}

fn changed() {
    if let Some(f) = ON_CHANGE.get() {
        f();
    }
}

fn set_problem(text: String) {
    if let Ok(mut problem) = PROBLEM.lock()
        && *problem != text
    {
        *problem = text;
        drop(problem);
        changed();
    }
}

fn clients() -> Vec<Arc<Client>> {
    CLIENTS.lock().map(|c| c.clone()).unwrap_or_default()
}

fn drop_clients() {
    for client in clients() {
        if let Ok(stream) = client.stream.lock() {
            let _ = stream.shutdown(Shutdown::Both);
        }
    }
}

fn serve() {
    loop {
        let generation = GENERATION.load(Ordering::SeqCst);
        let settings = crate::config::load_settings();
        let is_current = || GENERATION.load(Ordering::SeqCst) == generation;
        if settings.browser_enabled.unwrap_or(false) {
            let port = settings.browser_port.unwrap_or(DEFAULT_PORT);
            match TcpListener::bind(("127.0.0.1", port)).and_then(|l| l.set_nonblocking(true).map(|_| l)) {
                Ok(listener) => {
                    set_problem(String::new());
                    accept_while(&listener, &key(), &is_current);
                    drop_clients();
                    continue;
                }
                Err(e) => set_problem(format!("Couldn't listen on port {port}: {e}")),
            }
        } else {
            set_problem(String::new());
        }
        while is_current() {
            std::thread::sleep(Duration::from_millis(200));
        }
    }
}

fn accept_while(listener: &TcpListener, key: &str, keep_going: &dyn Fn() -> bool) {
    while keep_going() {
        match listener.accept() {
            Ok((stream, _)) => {
                let key = key.to_string();
                let _ = std::thread::Builder::new().name("browser-client".into()).spawn(move || connection(stream, &key));
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => std::thread::sleep(Duration::from_millis(100)),
            Err(e) => warn!("Browser bridge accept failed: {e}"),
        }
    }
}

// ── Requests ────────────────────────────────────────────────────────────────

/// The browser requests go to: the one focused most recently, else the
/// newest connection.
fn target() -> Result<Arc<Client>, String> {
    let all = clients();
    all.iter()
        .max_by_key(|c| (c.focused_at.lock().ok().and_then(|f| *f), c.connected_at))
        .cloned()
        .ok_or_else(|| {
            if crate::config::load_settings().browser_enabled.unwrap_or(false) {
                "no browser is connected: install the Blockwork browser extension and paste the pairing key from Settings > Integrations into it".into()
            } else {
                "browser control is off: turn it on in Settings > Integrations, then install the Blockwork browser extension".into()
            }
        })
}

/// Whether any browser is connected.
pub fn connected() -> bool {
    !clients().is_empty()
}

/// Asks the browser to carry out `cmd` and waits for its answer.
pub fn request(cmd: &str, args: Value, timeout: Duration) -> Result<Value, String> {
    let client = target()?;
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let (tx, rx) = mpsc::channel();
    client.pending.lock().map_err(|e| e.to_string())?.insert(id, tx);
    let message = json!({ "type": "request", "id": id, "cmd": cmd, "args": args });
    let sent = client
        .stream
        .lock()
        .map_err(|e| e.to_string())
        .and_then(|mut s| write_frame(&mut *s, OP_TEXT, message.to_string().as_bytes()).map_err(|e| e.to_string()));
    let answer = match sent {
        Ok(()) => rx.recv_timeout(timeout).unwrap_or_else(|e| match e {
            mpsc::RecvTimeoutError::Timeout => Err(format!("the browser didn't answer within {} s", timeout.as_secs())),
            mpsc::RecvTimeoutError::Disconnected => Err("the browser disconnected".into()),
        }),
        Err(e) => Err(format!("couldn't reach the browser: {e}")),
    };
    if let Ok(mut pending) = client.pending.lock() {
        pending.remove(&id);
    }
    answer
}

/// A request's `result.<field>` as text.
pub fn text_field(result: &Value, field: &str) -> String {
    match &result[field] {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

// ── One connection ──────────────────────────────────────────────────────────

fn connection(stream: TcpStream, key: &str) {
    // An accepted socket can inherit the listener's non-blocking mode.
    let _ = stream.set_nonblocking(false);
    let _ = stream.set_read_timeout(Some(HANDSHAKE_LIMIT));
    let _ = stream.set_nodelay(true);
    let Ok(writer) = stream.try_clone() else { return };
    let mut reader = BufReader::new(stream);
    let browser = match handshake(&mut reader, &writer, key) {
        Ok(browser) => browser,
        Err(e) => {
            tracing::info!("Browser bridge refused a connection: {e}");
            return;
        }
    };
    let _ = writer.set_read_timeout(Some(IDLE_LIMIT));
    let client = Arc::new(Client {
        browser,
        stream: Mutex::new(writer),
        pending: Mutex::new(HashMap::new()),
        focused_at: Mutex::new(None),
        connected_at: Instant::now(),
    });
    if let Ok(mut all) = CLIENTS.lock() {
        all.push(Arc::clone(&client));
    }
    changed();
    while let Ok(Some(message)) = read_message(&mut reader, &client.stream) {
        let Ok(message) = serde_json::from_slice::<Value>(&message) else { continue };
        match message["type"].as_str() {
            Some("response") => {
                let id = message["id"].as_u64().unwrap_or(0);
                let reply = if message["ok"].as_bool() == Some(true) {
                    Ok(message["result"].clone())
                } else {
                    Err(message["error"].as_str().unwrap_or("the browser couldn't do it").to_string())
                };
                if let Some(tx) = client.pending.lock().ok().and_then(|mut p| p.remove(&id)) {
                    let _ = tx.send(reply);
                }
            }
            Some("focus") if message["focused"].as_bool() == Some(true) => {
                if let Ok(mut at) = client.focused_at.lock() {
                    *at = Some(Instant::now());
                }
            }
            _ => {}
        }
    }
    if let Ok(mut all) = CLIENTS.lock() {
        all.retain(|c| !Arc::ptr_eq(c, &client));
    }
    if let Ok(mut pending) = client.pending.lock() {
        for (_, tx) in pending.drain() {
            let _ = tx.send(Err("the browser disconnected".into()));
        }
    }
    changed();
}

/// The WebSocket upgrade, then the pairing proof. Returns the browser's name.
fn handshake(reader: &mut BufReader<TcpStream>, mut writer: &TcpStream, key: &str) -> Result<String, String> {
    let mut headers = HashMap::new();
    let mut line = String::new();
    reader.read_line(&mut line).map_err(|e| e.to_string())?;
    if !line.starts_with("GET ") {
        return Err("not a WebSocket request".into());
    }
    for _ in 0..100 {
        line.clear();
        reader.read_line(&mut line).map_err(|e| e.to_string())?;
        let header = line.trim_end();
        if header.is_empty() {
            break;
        }
        if let Some((name, value)) = header.split_once(':') {
            headers.insert(name.trim().to_ascii_lowercase(), value.trim().to_string());
        }
    }
    let origin = headers.get("origin").map(String::as_str).unwrap_or("");
    if !ORIGINS.iter().any(|o| origin.starts_with(o)) {
        let _ = writer.write_all(b"HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
        return Err(format!("origin {origin:?} isn't a browser extension"));
    }
    let Some(ws_key) = headers.get("sec-websocket-key") else {
        let _ = writer.write_all(b"HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
        return Err("no Sec-WebSocket-Key".into());
    };
    write!(
        writer,
        "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {}\r\n\r\n",
        accept_key(ws_key)
    )
    .map_err(|e| e.to_string())?;

    let nonce = fresh_key();
    let hello = json!({ "type": "hello", "nonce": nonce, "protocol": PROTOCOL, "version": env!("CARGO_PKG_VERSION") });
    write_frame(&mut writer, OP_TEXT, hello.to_string().as_bytes()).map_err(|e| e.to_string())?;
    let lock = Mutex::new(writer.try_clone().map_err(|e| e.to_string())?);
    let auth = read_message(reader, &lock)?.ok_or("closed during the handshake")?;
    let auth: Value = serde_json::from_slice(&auth).map_err(|e| e.to_string())?;
    let mac = auth["mac"].as_str().unwrap_or("");
    if auth["type"] != "auth" || !same(mac, &hmac_hex(key, &format!("client:{nonce}"))) {
        let denied = json!({ "type": "denied", "reason": "The pairing key doesn't match Blockwork's. Copy it again from Settings > Integrations." });
        let _ = write_frame(&mut writer, OP_TEXT, denied.to_string().as_bytes());
        let _ = write_frame(&mut writer, OP_CLOSE, &[]);
        return Err("wrong pairing key".into());
    }
    let theirs = auth["nonce"].as_str().unwrap_or("");
    let welcome = json!({ "type": "welcome", "mac": hmac_hex(key, &format!("server:{theirs}")), "version": env!("CARGO_PKG_VERSION") });
    write_frame(&mut writer, OP_TEXT, welcome.to_string().as_bytes()).map_err(|e| e.to_string())?;
    let browser = auth["browser"].as_str().unwrap_or("A browser");
    Ok(browser.chars().take(60).collect())
}

/// Compares in time that doesn't depend on where the first difference is.
fn same(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |diff, (x, y)| diff | (x ^ y)) == 0
}

// ── WebSocket frames (RFC 6455) ─────────────────────────────────────────────

const OP_CONTINUE: u8 = 0x0;
const OP_TEXT: u8 = 0x1;
const OP_BINARY: u8 = 0x2;
const OP_CLOSE: u8 = 0x8;
const OP_PING: u8 = 0x9;
const OP_PONG: u8 = 0xA;

fn write_frame(out: &mut impl Write, opcode: u8, payload: &[u8]) -> std::io::Result<()> {
    let mut frame = Vec::with_capacity(payload.len() + 10);
    frame.push(0x80 | opcode);
    match payload.len() {
        n if n < 126 => frame.push(n as u8),
        n if n <= 0xFFFF => {
            frame.push(126);
            frame.extend_from_slice(&(n as u16).to_be_bytes());
        }
        n => {
            frame.push(127);
            frame.extend_from_slice(&(n as u64).to_be_bytes());
        }
    }
    frame.extend_from_slice(payload);
    out.write_all(&frame)?;
    out.flush()
}

/// The next whole text or binary message, answering pings on the way.
/// `None` once the peer closes.
fn read_message(reader: &mut impl Read, writer: &Mutex<TcpStream>) -> Result<Option<Vec<u8>>, String> {
    let mut message = Vec::new();
    loop {
        let mut head = [0u8; 2];
        if reader.read_exact(&mut head).is_err() {
            return Ok(None);
        }
        let fin = head[0] & 0x80 != 0;
        let opcode = head[0] & 0x0F;
        if head[1] & 0x80 == 0 {
            return Err("a client frame wasn't masked".into());
        }
        let len = match head[1] & 0x7F {
            126 => {
                let mut b = [0u8; 2];
                reader.read_exact(&mut b).map_err(|e| e.to_string())?;
                u16::from_be_bytes(b) as usize
            }
            127 => {
                let mut b = [0u8; 8];
                reader.read_exact(&mut b).map_err(|e| e.to_string())?;
                usize::try_from(u64::from_be_bytes(b)).unwrap_or(usize::MAX)
            }
            n => n as usize,
        };
        if len + message.len() > MAX_MESSAGE {
            return Err("a message was too large".into());
        }
        let mut mask = [0u8; 4];
        reader.read_exact(&mut mask).map_err(|e| e.to_string())?;
        let mut payload = vec![0u8; len];
        reader.read_exact(&mut payload).map_err(|e| e.to_string())?;
        for (i, b) in payload.iter_mut().enumerate() {
            *b ^= mask[i % 4];
        }
        match opcode {
            OP_CLOSE => {
                if let Ok(mut w) = writer.lock() {
                    let _ = write_frame(&mut *w, OP_CLOSE, &[]);
                }
                return Ok(None);
            }
            OP_PING => {
                if let Ok(mut w) = writer.lock() {
                    let _ = write_frame(&mut *w, OP_PONG, &payload);
                }
            }
            OP_PONG => {}
            OP_TEXT | OP_BINARY | OP_CONTINUE => {
                message.extend_from_slice(&payload);
                if fin {
                    return Ok(Some(message));
                }
            }
            _ => return Err(format!("unknown frame type {opcode}")),
        }
    }
}

fn accept_key(key: &str) -> String {
    use base64::Engine;
    let digest = sha1(format!("{key}258EAFA5-E914-47DA-95CA-C5AB0DC85B11").as_bytes());
    base64::engine::general_purpose::STANDARD.encode(digest)
}

/// SHA-1, for the WebSocket accept header only.
fn sha1(data: &[u8]) -> [u8; 20] {
    let mut h: [u32; 5] = [0x67452301, 0xEFCDAB89, 0x98BADCFE, 0x10325476, 0xC3D2E1F0];
    let mut padded = data.to_vec();
    padded.push(0x80);
    while padded.len() % 64 != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&((data.len() as u64) * 8).to_be_bytes());
    for chunk in padded.chunks(64) {
        let mut w = [0u32; 80];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([chunk[i * 4], chunk[i * 4 + 1], chunk[i * 4 + 2], chunk[i * 4 + 3]]);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }
        let [mut a, mut b, mut c, mut d, mut e] = h;
        for (i, wi) in w.iter().enumerate() {
            let (f, k) = match i {
                0..=19 => ((b & c) | (!b & d), 0x5A827999),
                20..=39 => (b ^ c ^ d, 0x6ED9EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1BBCDC),
                _ => (b ^ c ^ d, 0xCA62C1D6),
            };
            let t = a.rotate_left(5).wrapping_add(f).wrapping_add(e).wrapping_add(k).wrapping_add(*wi);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = t;
        }
        for (x, y) in h.iter_mut().zip([a, b, c, d, e]) {
            *x = x.wrapping_add(y);
        }
    }
    let mut out = [0u8; 20];
    for (i, word) in h.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    out
}

fn hmac_hex(key: &str, message: &str) -> String {
    let mut block = [0u8; 64];
    if key.len() > 64 {
        block[..32].copy_from_slice(&Sha256::digest(key.as_bytes()));
    } else {
        block[..key.len()].copy_from_slice(key.as_bytes());
    }
    let pad = |byte: u8| block.iter().map(|b| b ^ byte).collect::<Vec<_>>();
    let inner = Sha256::new().chain_update(pad(0x36)).chain_update(message.as_bytes()).finalize();
    let outer = Sha256::new().chain_update(pad(0x5c)).chain_update(inner).finalize();
    outer.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_match_published_vectors() {
        let hex = |b: &[u8]| b.iter().map(|b| format!("{b:02x}")).collect::<String>();
        assert_eq!(hex(&sha1(b"abc")), "a9993e364706816aba3e25717850c26c9cd0d89d");
        assert_eq!(hex(&sha1(&[b'a'; 1000])), "291e9a6c66994949b57ba5e650361e98fc36b1ba");
        assert_eq!(accept_key("dGhlIHNhbXBsZSBub25jZQ=="), "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=");
        assert_eq!(hmac_hex("Jefe", "what do ya want for nothing?"), "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843");
    }

    fn masked(opcode: u8, payload: &[u8]) -> Vec<u8> {
        let mask = [1u8, 2, 3, 4];
        let mut frame = vec![0x80 | opcode];
        if payload.len() < 126 {
            frame.push(0x80 | payload.len() as u8);
        } else {
            frame.push(0x80 | 126);
            frame.extend_from_slice(&(payload.len() as u16).to_be_bytes());
        }
        frame.extend_from_slice(&mask);
        frame.extend(payload.iter().enumerate().map(|(i, b)| b ^ mask[i % 4]));
        frame
    }

    fn unmasked(reader: &mut impl Read) -> Value {
        let mut head = [0u8; 2];
        reader.read_exact(&mut head).unwrap();
        let len = match head[1] {
            126 => {
                let mut b = [0u8; 2];
                reader.read_exact(&mut b).unwrap();
                u16::from_be_bytes(b) as usize
            }
            n => n as usize,
        };
        let mut payload = vec![0u8; len];
        reader.read_exact(&mut payload).unwrap();
        serde_json::from_slice(&payload).unwrap()
    }

    /// A fake extension: upgrades, pairs with `key`, then answers one request.
    fn fake_extension(port: u16, origin: &str, key: &str) -> Result<(TcpStream, BufReader<TcpStream>), String> {
        let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
        write!(stream, "GET /blockwork HTTP/1.1\r\nHost: x\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nOrigin: {origin}\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\n\r\n").unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut status = String::new();
        reader.read_line(&mut status).unwrap();
        if !status.contains("101") {
            return Err(status);
        }
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            if line.trim().is_empty() {
                break;
            }
        }
        let hello = unmasked(&mut reader);
        let nonce = hello["nonce"].as_str().unwrap();
        let auth = json!({ "type": "auth", "nonce": "ours", "mac": hmac_hex(key, &format!("client:{nonce}")), "browser": "Test 1" });
        stream.write_all(&masked(OP_TEXT, auth.to_string().as_bytes())).unwrap();
        let answer = unmasked(&mut reader);
        if answer["type"] != "welcome" {
            return Err(answer.to_string());
        }
        assert_eq!(answer["mac"], hmac_hex(key, "server:ours"));
        Ok((stream, reader))
    }

    #[test]
    fn pairs_and_carries_a_request() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        listener.set_nonblocking(true).unwrap();
        let port = listener.local_addr().unwrap().port();
        let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let server = {
            let stop = Arc::clone(&stop);
            std::thread::spawn(move || accept_while(&listener, "k3y", &|| !stop.load(Ordering::SeqCst)))
        };

        assert!(fake_extension(port, "https://evil.example", "k3y").unwrap_err().contains("403"));
        assert!(fake_extension(port, "moz-extension://abc", "wrong").unwrap_err().contains("denied"));

        let (mut stream, mut reader) = fake_extension(port, "chrome-extension://abc", "k3y").unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while !clients().iter().any(|c| c.browser == "Test 1") {
            assert!(Instant::now() < deadline, "the client never registered");
            std::thread::sleep(Duration::from_millis(10));
        }
        let asker = std::thread::spawn(|| request("click", json!({ "selector": "text=More" }), Duration::from_secs(5)));
        let asked = unmasked(&mut reader);
        assert_eq!((asked["cmd"].as_str(), asked["args"]["selector"].as_str()), (Some("click"), Some("text=More")));
        let reply = json!({ "type": "response", "id": asked["id"], "ok": true, "result": { "done": "<button> More" } });
        stream.write_all(&masked(OP_TEXT, reply.to_string().as_bytes())).unwrap();
        assert_eq!(asker.join().unwrap().unwrap()["done"], "<button> More");

        let failing = std::thread::spawn(|| request("read", json!({}), Duration::from_secs(5)));
        let asked = unmasked(&mut reader);
        let reply = json!({ "type": "response", "id": asked["id"], "ok": false, "error": "nothing on the page matches x" });
        stream.write_all(&masked(OP_TEXT, reply.to_string().as_bytes())).unwrap();
        assert_eq!(failing.join().unwrap().unwrap_err(), "nothing on the page matches x");

        stream.write_all(&masked(OP_CLOSE, &[])).unwrap();
        while clients().iter().any(|c| c.browser == "Test 1") {
            assert!(Instant::now() < deadline, "the client never left");
            std::thread::sleep(Duration::from_millis(10));
        }
        stop.store(true, Ordering::SeqCst);
        server.join().unwrap();
    }
}
