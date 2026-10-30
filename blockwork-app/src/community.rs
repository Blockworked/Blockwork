//! The community library site (community.blockworked.com): signing in
//! through the browser, pack pages with their versions and comments, the
//! signed-in account's packs, and sharing packs and updates to it.
//!
//! Network work runs on its own threads and lands in a cache; the snapshot
//! carries a `revision` that moves whenever the cache does, and the UI asks
//! for what it shows with quick commands that never wait on the network.

use crate::state::{emit_state_updated, SharedState};
use crate::AppHandle;
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Where the token is kept, in the keyring or, failing that, a file.
const TOKEN_NAME: &str = "community-library-token";
const TOKEN_FILE: &str = "community-account.json";
/// How long the browser has to come back with a sign-in.
const SIGN_IN_WAIT: Duration = Duration::from_secs(10 * 60);
const MAX_RESPONSE: u64 = 8 * 1024 * 1024;

/// The site's address. `BLOCKWORK_COMMUNITY_URL` points the app at a local copy.
pub(crate) fn site() -> String {
    if let Ok(url) = std::env::var("BLOCKWORK_COMMUNITY_URL") {
        let url = url.trim().trim_end_matches('/');
        if !url.is_empty() {
            return url.to_string();
        }
    }
    blockwork_core::block_packs::DEFAULT_INDEX
        .trim_end_matches("/index.json")
        .to_string()
}

/// The built-in library's index: the community site's.
pub(crate) fn index_url() -> String {
    format!("{}/index.json", site())
}

/// Whether a library index is the community site's, which has pack pages and comments.
pub(crate) fn serves(index_url: &str) -> bool {
    index_url == self::index_url()
}

#[derive(Serialize, Deserialize, Clone, Default, Debug, PartialEq)]
pub(crate) struct UserDto {
    #[serde(default)]
    pub(crate) name: String,
    #[serde(default)]
    pub(crate) avatar: String,
    /// Public id, as in /authors/<id>.
    #[serde(default)]
    pub(crate) id: String,
    #[serde(default)]
    pub(crate) page: String,
    #[serde(default)]
    pub(crate) official: bool,
    #[serde(default)]
    pub(crate) banned: bool,
}

/// One piece of background work the UI waits on: signing in, posting a
/// comment, publishing.
#[derive(Serialize, Clone, Default)]
pub(crate) struct TaskDto {
    pub(crate) running: bool,
    pub(crate) error: String,
    /// What finished, for a confirmation.
    pub(crate) message: String,
    /// Counts finishes, so the UI can tell one from the next.
    pub(crate) done: u64,
}

#[derive(Serialize, Clone, Default)]
pub(crate) struct CommunityDto {
    pub(crate) site: String,
    pub(crate) signed_in: bool,
    pub(crate) user: UserDto,
    /// The page the browser was sent to, while a sign-in waits.
    pub(crate) sign_in_url: String,
    pub(crate) revision: u64,
    pub(crate) tasks: BTreeMap<String, TaskDto>,
}

#[derive(Serialize, Deserialize, Clone)]
struct Account {
    token: String,
    user: UserDto,
}

#[derive(Default)]
struct Cache {
    account: Option<Account>,
    account_loaded: bool,
    sign_in_url: String,
    /// Bumped to stop a sign-in that's waiting.
    sign_in: u64,
    tasks: BTreeMap<String, TaskDto>,
    /// Pack pages by pack id.
    details: HashMap<String, Result<Value, String>>,
    loading: HashSet<String>,
    mine: Option<Result<Value, String>>,
    mine_loading: bool,
}

static CACHE: Mutex<Option<Cache>> = Mutex::new(None);
static REVISION: AtomicU64 = AtomicU64::new(0);

fn with<R>(f: impl FnOnce(&mut Cache) -> R) -> R {
    let mut guard = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    let cache = guard.get_or_insert_with(Cache::default);
    if !cache.account_loaded {
        cache.account_loaded = true;
        cache.account = load_account();
    }
    f(cache)
}

/// Marks the cache changed and sends the snapshot.
pub(crate) fn changed(state: &SharedState, app: &AppHandle) {
    REVISION.fetch_add(1, Ordering::Relaxed);
    if let Ok(s) = state.lock() {
        emit_state_updated(app, &s);
    }
}

pub(crate) fn dto() -> CommunityDto {
    with(|c| CommunityDto {
        site: site(),
        signed_in: c.account.is_some(),
        user: c
            .account
            .as_ref()
            .map(|a| a.user.clone())
            .unwrap_or_default(),
        sign_in_url: c.sign_in_url.clone(),
        revision: REVISION.load(Ordering::Relaxed),
        tasks: c.tasks.clone(),
    })
}

fn token() -> Option<String> {
    with(|c| c.account.as_ref().map(|a| a.token.clone()))
}

// Keeping the account

fn account_file() -> Option<std::path::PathBuf> {
    if cfg!(test) {
        return None;
    }
    blockwork_core::config::config_root()
        .ok()
        .map(|root| root.join(TOKEN_FILE))
}

/// The name and picture are kept in a file; the token in the keyring when
/// there is one, else in that file, readable only by this user.
fn load_account() -> Option<Account> {
    let text = std::fs::read_to_string(account_file()?).ok()?;
    let mut account: Account = serde_json::from_str(&text).ok()?;
    if account.token.is_empty() {
        account.token = blockwork_core::secrets::load_private(TOKEN_NAME)
            .ok()
            .flatten()?;
    }
    (!account.token.is_empty()).then_some(account)
}

fn save_account(account: Option<&Account>) {
    let Some(path) = account_file() else { return };
    let Some(account) = account else {
        let _ = blockwork_core::secrets::remove_private(TOKEN_NAME);
        let _ = std::fs::remove_file(path);
        return;
    };
    let mut saved = account.clone();
    if blockwork_core::secrets::store_private(TOKEN_NAME, &account.token).is_ok() {
        saved.token.clear();
    }
    let Ok(text) = serde_json::to_string_pretty(&saved) else {
        return;
    };
    if let Err(e) = write_private(&path, &text) {
        tracing::warn!("Couldn't save the community library sign-in: {e}");
    }
}

fn write_private(path: &std::path::Path, text: &str) -> std::io::Result<()> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)?.write_all(text.as_bytes())
}

// Talking to the site

/// Calls the site's API. A refusal comes back as the site's own words.
fn call(
    method: &str,
    path: &str,
    body: Option<(&str, Vec<u8>)>,
    signed: bool,
) -> Result<Value, String> {
    let url = format!("{}{path}", site());
    let auth = if signed {
        token().map(|t| format!("Bearer {t}"))
    } else {
        None
    };
    if signed && auth.is_none() {
        return Err("Sign in to the community library first".into());
    }
    let mut headers = vec![("Accept", "application/json")];
    if let Some(auth) = &auth {
        headers.push(("Authorization", auth.as_str()));
    }
    let content_type;
    let bytes = match body {
        Some((kind, bytes)) => {
            content_type = kind.to_string();
            headers.push(("Content-Type", content_type.as_str()));
            Some(bytes)
        }
        None => None,
    };
    let (status, response) =
        blockwork_core::http::send(method, &url, &headers, bytes, MAX_RESPONSE).map_err(|_| {
            "Couldn't reach the community library. Check your connection and try again".to_string()
        })?;
    let value: Value = serde_json::from_slice(&response).unwrap_or(Value::Null);
    if status == 401 && signed {
        // The token was signed out elsewhere, or the account was banned.
        with(|c| c.account = None);
        save_account(None);
        return Err("You've been signed out of the community library. Sign in again".into());
    }
    if status >= 400 {
        return Err(value
            .get("error")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| {
                format!("The community library answered with an error ({status})")
            }));
    }
    Ok(value)
}

fn start_task(name: &str) -> bool {
    with(|c| {
        let task = c.tasks.entry(name.to_string()).or_default();
        if task.running {
            return false;
        }
        task.running = true;
        task.error.clear();
        task.message.clear();
        true
    })
}

fn finish_task(name: &str, result: Result<String, String>) {
    with(|c| {
        let task = c.tasks.entry(name.to_string()).or_default();
        task.running = false;
        task.done += 1;
        match result {
            Ok(message) => task.message = message,
            Err(error) => task.error = error,
        }
    });
}

/// Runs `work` on its own thread as task `name`, then sends the snapshot.
fn spawn_task(
    state: &SharedState,
    app: &AppHandle,
    name: &'static str,
    work: impl FnOnce() -> Result<String, String> + Send + 'static,
) -> Result<(), String> {
    if !start_task(name) {
        return Ok(());
    }
    changed(state, app);
    let (state, app) = (Arc::clone(state), app.clone());
    std::thread::spawn(move || {
        finish_task(name, work());
        changed(&state, &app);
    });
    Ok(())
}

// Signing in

fn random_token() -> String {
    let mut bytes = Vec::with_capacity(32);
    bytes.extend_from_slice(uuid::Uuid::new_v4().as_bytes());
    bytes.extend_from_slice(uuid::Uuid::new_v4().as_bytes());
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

pub(crate) fn challenge_of(verifier: &str) -> String {
    use sha2::Digest;
    base64::engine::general_purpose::URL_SAFE_NO_PAD
        .encode(sha2::Sha256::digest(verifier.as_bytes()))
}

fn computer_name() -> String {
    let name = std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .ok()
        .or_else(|| std::fs::read_to_string("/etc/hostname").ok())
        .unwrap_or_default();
    name.trim().chars().take(60).collect()
}

/// Starts listening on this computer and returns the page to open in the
/// browser. The site sends the browser back to that address with a code,
/// which is swapped for a token along with a verifier only this app knows.
pub(crate) fn sign_in(state: &SharedState, app: &AppHandle) -> Result<Value, String> {
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .map_err(|e| format!("Couldn't start signing in: {e}"))?;
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    let (state_token, verifier) = (random_token(), random_token());
    let mut url = format!(
        "{}/app/login?port={port}&state={state_token}&challenge={}",
        site(),
        challenge_of(&verifier)
    );
    let label = computer_name();
    if !label.is_empty() {
        url.push_str("&label=");
        url.push_str(&percent_encode(&label));
    }
    let attempt = with(|c| {
        c.sign_in += 1;
        c.sign_in_url = url.clone();
        let task = c.tasks.entry("sign_in".into()).or_default();
        task.running = true;
        task.error.clear();
        task.message.clear();
        c.sign_in
    });
    changed(state, app);
    let (state, app) = (Arc::clone(state), app.clone());
    std::thread::spawn(move || {
        let result = wait_for_browser(&listener, attempt, &state_token, &verifier);
        // A newer sign-in or a cancel took over.
        if with(|c| c.sign_in != attempt) {
            return;
        }
        with(|c| c.sign_in_url.clear());
        let result = result.map(|account| {
            let name = account.user.name.clone();
            save_account(Some(&account));
            with(|c| {
                c.account = Some(account);
                c.mine = None;
                c.details.clear();
            });
            format!("Signed in as {name}")
        });
        finish_task("sign_in", result);
        changed(&state, &app);
    });
    Ok(json!({ "url": url }))
}

pub(crate) fn cancel_sign_in(state: &SharedState, app: &AppHandle) -> Result<(), String> {
    with(|c| {
        c.sign_in += 1;
        c.sign_in_url.clear();
        if let Some(task) = c.tasks.get_mut("sign_in") {
            task.running = false;
        }
    });
    changed(state, app);
    Ok(())
}

fn wait_for_browser(
    listener: &TcpListener,
    attempt: u64,
    state_token: &str,
    verifier: &str,
) -> Result<Account, String> {
    let started = Instant::now();
    loop {
        if with(|c| c.sign_in != attempt) {
            return Err(String::new());
        }
        if started.elapsed() > SIGN_IN_WAIT {
            return Err("Signing in took too long. Try again".into());
        }
        let stream = match listener.accept() {
            Ok((stream, _)) => stream,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(150));
                continue;
            }
            Err(e) => return Err(format!("Signing in stopped: {e}")),
        };
        let Some(query) = read_callback(&stream) else {
            respond(stream, 404, "Not found", "");
            continue;
        };
        let get = |key: &str| {
            query
                .iter()
                .find(|(k, _)| k == key)
                .map(|(_, v)| v.clone())
                .unwrap_or_default()
        };
        if get("state") != state_token {
            respond(
                stream,
                400,
                "That sign-in isn't this one",
                "Start signing in again from Blockwork.",
            );
            continue;
        }
        if get("error") == "denied" {
            respond(
                stream,
                200,
                "Sign-in cancelled",
                "You can close this tab and go back to Blockwork.",
            );
            return Err("Sign-in was cancelled".into());
        }
        let result = exchange(&get("code"), verifier);
        match &result {
            Ok(account) => respond(
                stream,
                200,
                "You're signed in",
                &format!(
                    "Blockwork is signed in as {}. You can close this tab and go back to it.",
                    account.user.name
                ),
            ),
            Err(error) => respond(stream, 400, "Sign-in didn't work", error),
        }
        return result;
    }
}

fn exchange(code: &str, verifier: &str) -> Result<Account, String> {
    let body = serde_json::to_vec(&json!({ "code": code, "verifier": verifier }))
        .map_err(|e| e.to_string())?;
    let value = call(
        "POST",
        "/api/v1/app/token",
        Some(("application/json", body)),
        false,
    )?;
    let token = value
        .get("token")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let user: UserDto = value
        .get("user")
        .cloned()
        .and_then(|u| serde_json::from_value(u).ok())
        .unwrap_or_default();
    if token.is_empty() {
        return Err("The community library didn't sign you in. Try again".into());
    }
    Ok(Account { token, user })
}

/// The query of a `GET /callback?...` request, or None for anything else.
fn read_callback(mut stream: &TcpStream) -> Option<Vec<(String, String)>> {
    stream.set_nonblocking(false).ok()?;
    stream.set_read_timeout(Some(Duration::from_secs(5))).ok()?;
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 1024];
    while !buffer.windows(4).any(|w| w == b"\r\n\r\n") && buffer.len() < 16 * 1024 {
        let n = stream.read(&mut chunk).ok()?;
        if n == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..n]);
    }
    let text = String::from_utf8_lossy(&buffer);
    let target = text
        .lines()
        .next()?
        .strip_prefix("GET ")?
        .split(' ')
        .next()?;
    let query = target.strip_prefix("/callback?")?;
    Some(
        query
            .split('&')
            .filter_map(|pair| {
                let (k, v) = pair.split_once('=')?;
                Some((percent_decode(k), percent_decode(v)))
            })
            .collect(),
    )
}

fn respond(mut stream: TcpStream, status: u16, title: &str, text: &str) {
    let escape = |s: &str| {
        s.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
    };
    let body = format!(
        "<!doctype html><meta charset=utf-8><meta name=viewport content='width=device-width'><title>{0}</title>\
         <style>body{{font:16px system-ui,sans-serif;background:#1c1d21;color:#e8e8ea;display:grid;place-items:center;min-height:90vh;margin:0}}\
         main{{max-width:28rem;padding:2rem;text-align:center}}h1{{font-size:1.4rem}}p{{color:#a9abb3}}\
         @media(prefers-color-scheme:light){{body{{background:#f6f6f8;color:#1c1d21}}p{{color:#55575f}}}}</style>\
         <main><h1>{0}</h1><p>{1}</p></main>",
        escape(title), escape(text)
    );
    let reason = if status == 200 {
        "OK"
    } else if status == 404 {
        "Not Found"
    } else {
        "Bad Request"
    };
    let _ = write!(stream, "HTTP/1.1 {status} {reason}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n{body}", body.len());
}

fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < bytes.len() => {
                match std::str::from_utf8(&bytes[i + 1..i + 3])
                    .ok()
                    .and_then(|hex| u8::from_str_radix(hex, 16).ok())
                {
                    Some(b) => {
                        out.push(b);
                        i += 2;
                    }
                    None => out.push(b'%'),
                }
            }
            b => out.push(b),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

pub(crate) fn percent_encode(text: &str) -> String {
    text.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

pub(crate) fn sign_out(state: &SharedState, app: &AppHandle) -> Result<(), String> {
    if token().is_some() {
        // Best effort: the token is forgotten here either way.
        let _ = call("POST", "/api/v1/app/logout", None, true);
    }
    with(|c| {
        c.account = None;
        c.mine = None;
        c.details.clear();
    });
    save_account(None);
    changed(state, app);
    Ok(())
}

// Pack pages

/// A pack's page as the site has it (versions, comments), or `loading`.
/// Starts fetching it when it isn't cached or `refresh` asks.
pub(crate) fn pack_detail(
    state: &SharedState,
    app: &AppHandle,
    pack_id: &str,
    refresh: bool,
) -> Value {
    let cached = with(|c| {
        let loading = c.loading.contains(pack_id);
        let cached = c.details.get(pack_id).cloned();
        let start = !loading && (refresh || cached.is_none());
        if start {
            c.loading.insert(pack_id.to_string());
        }
        (cached, loading || start, start)
    });
    if cached.2 {
        let (state, app, id) = (Arc::clone(state), app.clone(), pack_id.to_string());
        std::thread::spawn(move || {
            let signed = token().is_some();
            let result = call(
                "GET",
                &format!("/api/v1/packs/{}", percent_encode(&id)),
                None,
                signed,
            );
            with(|c| {
                c.loading.remove(&id);
                c.details.insert(id, result);
            });
            changed(&state, &app);
        });
    }
    match cached.0 {
        Some(Ok(value)) => json!({ "loading": cached.1, "detail": value, "error": "" }),
        Some(Err(error)) => json!({ "loading": cached.1, "detail": Value::Null, "error": error }),
        None => json!({ "loading": true, "detail": Value::Null, "error": "" }),
    }
}

fn refetch_detail(pack_id: &str) {
    let result = call(
        "GET",
        &format!("/api/v1/packs/{}", percent_encode(pack_id)),
        None,
        token().is_some(),
    );
    with(|c| {
        c.details.insert(pack_id.to_string(), result);
    });
}

pub(crate) fn post_comment(
    state: &SharedState,
    app: &AppHandle,
    pack_id: String,
    body: String,
) -> Result<(), String> {
    if body.trim().is_empty() {
        return Err("Write something first".into());
    }
    spawn_task(state, app, "comment", move || {
        let bytes = serde_json::to_vec(&json!({ "body": body })).map_err(|e| e.to_string())?;
        call(
            "POST",
            &format!("/api/v1/packs/{}/comments", percent_encode(&pack_id)),
            Some(("application/json", bytes)),
            true,
        )?;
        refetch_detail(&pack_id);
        Ok("Comment posted".into())
    })
}

pub(crate) fn delete_comment(
    state: &SharedState,
    app: &AppHandle,
    pack_id: String,
    id: u64,
) -> Result<(), String> {
    spawn_task(state, app, "comment", move || {
        call("DELETE", &format!("/api/v1/comments/{id}"), None, true)?;
        refetch_detail(&pack_id);
        Ok("Comment deleted".into())
    })
}

// My packs and sharing

/// The account's packs, with each upload's review status.
pub(crate) fn my_packs(state: &SharedState, app: &AppHandle, refresh: bool) -> Value {
    let (cached, start) = with(|c| {
        let start = !c.mine_loading && c.account.is_some() && (refresh || c.mine.is_none());
        if start {
            c.mine_loading = true;
        }
        (c.mine.clone(), start)
    });
    if start {
        let (state, app) = (Arc::clone(state), app.clone());
        std::thread::spawn(move || {
            let result = call("GET", "/api/v1/me", None, true);
            with(|c| {
                c.mine_loading = false;
                // The name or picture may have changed on the provider's side.
                if let (Ok(value), Some(account)) = (&result, c.account.as_mut()) {
                    if let Some(user) = value
                        .get("user")
                        .cloned()
                        .and_then(|u| serde_json::from_value::<UserDto>(u).ok())
                    {
                        if user != account.user {
                            account.user = user;
                            save_account(Some(account));
                        }
                    }
                }
                c.mine = Some(result);
            });
            changed(&state, &app);
        });
    }
    let loading = start || with(|c| c.mine_loading);
    match cached {
        Some(Ok(value)) => {
            json!({ "loading": loading, "packs": value.get("packs").cloned().unwrap_or(json!([])), "error": "" })
        }
        Some(Err(error)) => json!({ "loading": loading, "packs": [], "error": error }),
        None => json!({ "loading": loading, "packs": [], "error": "" }),
    }
}

/// Sends a pack file to the site, where it waits for review.
fn upload(bytes: Vec<u8>) -> Result<String, String> {
    let value = call(
        "POST",
        "/api/v1/packs",
        Some(("application/json", bytes)),
        true,
    )?;
    with(|c| c.mine = None);
    let name = value
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("Your pack");
    let version = value
        .get("version")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let update = value
        .get("update")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let what = if version.is_empty() {
        name.to_string()
    } else {
        format!("{name} {version}")
    };
    Ok(if update {
        format!("{what} was sent. It replaces the listed version once it's reviewed.")
    } else {
        format!("{what} was sent. It's listed once it's reviewed.")
    })
}

pub(crate) fn publish_file(
    state: &SharedState,
    app: &AppHandle,
    path: String,
) -> Result<(), String> {
    let bytes = std::fs::read(&path).map_err(|e| format!("Couldn't read {path}: {e}"))?;
    if bytes.len() as u64 > blockwork_core::block_packs::MAX_DOWNLOAD {
        return Err("That file is too big".into());
    }
    let text =
        String::from_utf8(bytes.clone()).map_err(|_| "This isn't a block pack".to_string())?;
    blockwork_core::block_packs::read_pack(&text)?;
    spawn_task(state, app, "publish", move || upload(bytes))
}

/// Writes the chosen blocks as a pack, as Share does, and sends it.
pub(crate) fn publish_blocks(
    state: &SharedState,
    app: &AppHandle,
    block_ids: Vec<String>,
    info: blockwork_core::block_packs::PackInfo,
    variable_roles: Vec<blockwork_core::block_packs::VariableInterface>,
) -> Result<(), String> {
    let user = with(|c| c.account.as_ref().map(|a| a.user.name.clone()))
        .ok_or("Sign in to the community library first")?;
    // The site puts the account's name on it either way.
    let info = blockwork_core::block_packs::PackInfo {
        author: user,
        ..info
    };
    let path = std::env::temp_dir().join(format!(
        "blockwork-publish-{}.blockpack",
        uuid::Uuid::new_v4()
    ));
    let path_text = path.to_string_lossy().into_owned();
    let written =
        crate::block_library::export(state, app, block_ids, info, path_text, variable_roles);
    let bytes = written.and_then(|_| std::fs::read(&path).map_err(|e| e.to_string()));
    let _ = std::fs::remove_file(&path);
    let bytes = bytes?;
    spawn_task(state, app, "publish", move || upload(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn challenge_is_pkce_s256() {
        // RFC 7636 appendix B.
        assert_eq!(
            challenge_of("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
        assert_eq!(random_token().len(), 43);
    }

    #[test]
    fn percent_round_trip() {
        assert_eq!(
            percent_decode(&percent_encode("Ethan's PC: café")),
            "Ethan's PC: café"
        );
        assert_eq!(percent_decode("a+b%2"), "a b%2");
    }

    #[test]
    fn callback_query_is_read_from_the_request() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let client = std::thread::spawn(move || {
            let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
            stream
                .write_all(
                    b"GET /callback?code=abc%2Dd&state=xyz HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n",
                )
                .unwrap();
            let mut reply = String::new();
            stream.read_to_string(&mut reply).unwrap();
            reply
        });
        let (stream, _) = listener.accept().unwrap();
        let query = read_callback(&stream).unwrap();
        assert_eq!(
            query,
            vec![
                ("code".to_string(), "abc-d".to_string()),
                ("state".to_string(), "xyz".to_string())
            ]
        );
        respond(stream, 200, "Done", "<b>");
        let reply = client.join().unwrap();
        assert!(reply.starts_with("HTTP/1.1 200 OK"));
        assert!(reply.contains("&lt;b&gt;"));
    }
}
