//! The community block library: packs listed by the indexes in Settings,
//! downloaded in the background, checked against the index's hash and added
//! to the open macro after a review.

use crate::state::{emit_state_updated, AppState, CapabilityDto, SharedState};
use crate::AppHandle;
use blockwork_core::block_packs::{self, BlockPack, IndexEntry};
use blockwork_core::config::{load_settings, update_settings};
use serde::Serialize;
use std::sync::{Arc, Mutex};

/// One library source as Settings lists it.
#[derive(Serialize, Clone)]
pub(crate) struct SourceDto {
    pub(crate) url: String,
    pub(crate) name: String,
    pub(crate) error: String,
    pub(crate) packs: usize,
}

/// One pack the library offers.
#[derive(Serialize, Clone)]
pub(crate) struct PackDto {
    /// `source` and `id` together; what `review_block_pack` takes.
    pub(crate) key: String,
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) author: String,
    pub(crate) version: String,
    pub(crate) blocks: Vec<String>,
    pub(crate) source: String,
    /// Published by the Blockworked team; shows a verified badge.
    pub(crate) official: bool,
    /// The version the open macro has, or empty.
    pub(crate) installed: String,
    /// Installed, but the library has a different file.
    pub(crate) update: bool,
}

#[derive(Serialize, Clone, Default)]
pub(crate) struct LibraryDto {
    pub(crate) loaded: bool,
    pub(crate) loading: bool,
    pub(crate) sources: Vec<SourceDto>,
    pub(crate) packs: Vec<PackDto>,
    pub(crate) installed: Vec<block_packs::PackRecord>,
    /// The key of the pack being downloaded.
    pub(crate) fetching: String,
    pub(crate) error: String,
}

/// What the user reviews before a pack's blocks join the open macro.
#[derive(Serialize, Clone)]
pub(crate) struct PackReviewDto {
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) author: String,
    pub(crate) version: String,
    /// Where it came from: a library's name, or the file.
    pub(crate) source: String,
    /// Checked against the hash its library lists.
    pub(crate) verified: bool,
    pub(crate) blocks: Vec<String>,
    /// Blocks the macro already has, which this replaces.
    pub(crate) replaces: Vec<String>,
    pub(crate) capabilities: Vec<CapabilityDto>,
    pub(crate) notes: Vec<String>,
    pub(crate) macro_name: String,
}

/// A pack waiting on its review.
pub(crate) struct PendingPack {
    pub(crate) macro_id: String,
    pub(crate) pack: BlockPack,
    pub(crate) sha256: String,
    pub(crate) review: PackReviewDto,
}

struct Cache {
    loaded: bool,
    loading: bool,
    sources: Vec<SourceDto>,
    /// (source url, source name, entry)
    packs: Vec<(String, String, IndexEntry)>,
    fetching: String,
    error: String,
}

static CACHE: Mutex<Cache> = Mutex::new(Cache {
    loaded: false,
    loading: false,
    sources: Vec::new(),
    packs: Vec::new(),
    fetching: String::new(),
    error: String::new(),
});

fn cache() -> std::sync::MutexGuard<'static, Cache> {
    CACHE.lock().unwrap_or_else(|e| e.into_inner())
}

/// The index addresses from Settings; the built-in library until changed.
pub(crate) fn sources() -> Vec<String> {
    let saved = load_settings().block_library_sources;
    upgrade_sources(saved.unwrap_or_else(|| vec![block_packs::DEFAULT_INDEX.to_string()]))
}

/// Swaps old built-in addresses for the current one.
fn upgrade_sources(list: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for url in list {
        let url = if block_packs::OLD_DEFAULT_INDEXES.contains(&url.as_str()) {
            block_packs::DEFAULT_INDEX.to_string()
        } else {
            url
        };
        if !out.contains(&url) {
            out.push(url);
        }
    }
    out
}

pub(crate) fn dto(s: &AppState) -> LibraryDto {
    let cache = cache();
    let records = s.current_macro.as_ref().map(|m| m.settings.block_packs.clone()).unwrap_or_default();
    let packs = cache
        .packs
        .iter()
        .map(|(url, source, entry)| {
            let installed = records.iter().find(|r| r.id == entry.info.id);
            PackDto {
                key: format!("{url}#{}", entry.info.id),
                id: entry.info.id.clone(),
                name: entry.info.name.clone(),
                description: entry.info.description.clone(),
                author: entry.info.author.clone(),
                version: entry.info.version.clone(),
                blocks: entry.blocks.clone(),
                source: source.clone(),
                official: entry.official,
                installed: installed.map(|r| if r.version.is_empty() { "?".into() } else { r.version.clone() }).unwrap_or_default(),
                update: installed.is_some_and(|r| r.sha256 != entry.sha256),
            }
        })
        .collect();
    // Sources not fetched yet still show in Settings.
    let mut sources = cache.sources.clone();
    for url in self::sources() {
        if !sources.iter().any(|s| s.url == url) {
            sources.push(SourceDto { url, name: String::new(), error: String::new(), packs: 0 });
        }
    }
    sources.retain(|s| self::sources().contains(&s.url));
    LibraryDto {
        loaded: cache.loaded,
        loading: cache.loading,
        sources,
        packs,
        installed: records,
        fetching: cache.fetching.clone(),
        error: cache.error.clone(),
    }
}

fn emit(state: &SharedState, app: &AppHandle) {
    if let Ok(s) = state.lock() {
        emit_state_updated(app, &s);
    }
}

fn is_local(url: &str) -> bool {
    url.starts_with("file://") || !url.contains("://")
}

/// Bytes at `url`. Files are read only when `allow_local`, so a remote
/// index can't point at files on this computer. Plain http is only for
/// this computer, since a hash pinned by a tampered index proves nothing.
fn fetch(url: &str, allow_local: bool) -> Result<Vec<u8>, String> {
    if is_local(url) {
        if !allow_local {
            return Err(format!("A library on the web can't point at a file on this computer ({url})"));
        }
        let path = url.strip_prefix("file://").unwrap_or(url);
        let bytes = std::fs::read(path).map_err(|e| format!("Couldn't read {path}: {e}"))?;
        if bytes.len() as u64 > block_packs::MAX_DOWNLOAD {
            return Err(format!("{path} is too big"));
        }
        return Ok(bytes);
    }
    if !url.starts_with("https://") && !is_loopback_http(url) {
        return Err(format!("Libraries and packs must use https:// ({url})"));
    }
    blockwork_core::http::get_bytes(url, block_packs::MAX_DOWNLOAD)
}

/// Whether `url` is plain http to this computer. Judged on the whole host,
/// so `http://localhost:1@example.com` (user info, then another host) and
/// `http://localhost.example.com` don't count.
fn is_loopback_http(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("http://") else { return false };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    if authority.contains('@') {
        return false;
    }
    let host = match authority.strip_prefix('[') {
        Some(v6) => match v6.split_once(']') {
            Some((host, port)) if port.is_empty() || port.starts_with(':') => host,
            _ => return false,
        },
        None => authority.split(':').next().unwrap_or_default(),
    };
    matches!(host.to_ascii_lowercase().as_str(), "127.0.0.1" | "localhost" | "::1")
}

/// Fetches every index in the background.
pub(crate) fn refresh(state: &SharedState, app: &AppHandle) -> Result<(), String> {
    {
        let mut cache = cache();
        if cache.loading {
            return Ok(());
        }
        cache.loading = true;
        cache.error.clear();
    }
    emit(state, app);
    let (state, app) = (Arc::clone(state), app.clone());
    std::thread::spawn(move || {
        let mut sources = Vec::new();
        let mut packs = Vec::new();
        for url in self::sources() {
            let result = fetch(&url, true).and_then(|bytes| {
                let text = String::from_utf8(bytes).map_err(|_| "The library isn't text".to_string())?;
                block_packs::read_index(&text, &url)
            });
            match result {
                Ok(index) => {
                    sources.push(SourceDto { url: url.clone(), name: index.name.clone(), error: String::new(), packs: index.packs.len() });
                    packs.extend(index.packs.into_iter().map(|p| (url.clone(), index.name.clone(), p)));
                }
                Err(error) => sources.push(SourceDto { url, name: String::new(), error, packs: 0 }),
            }
        }
        {
            let mut cache = cache();
            cache.sources = sources;
            cache.packs = packs;
            cache.loading = false;
            cache.loaded = true;
        }
        emit(&state, &app);
    });
    Ok(())
}

pub(crate) fn add_source(state: &SharedState, app: &AppHandle, url: String) -> Result<(), String> {
    let url = url.trim().to_string();
    if url.is_empty() {
        return Err("Enter the address of a library's index.json".into());
    }
    let mut list = sources();
    if !list.contains(&url) {
        list.push(url);
    }
    update_settings(|s| s.block_library_sources = Some(list));
    refresh(state, app)
}

pub(crate) fn remove_source(state: &SharedState, app: &AppHandle, url: String) -> Result<(), String> {
    let list: Vec<String> = sources().into_iter().filter(|u| *u != url).collect();
    update_settings(|s| s.block_library_sources = Some(list));
    {
        let mut cache = cache();
        cache.sources.retain(|s| s.url != url);
        cache.packs.retain(|(u, ..)| *u != url);
    }
    emit(state, app);
    Ok(())
}

/// Stages `pack` for review against the open macro.
fn stage(state: &SharedState, pack: BlockPack, sha256: String, source: String, verified: bool) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    let mac = s.current_macro.as_ref().ok_or("Open a macro to add blocks to")?;
    let review = PackReviewDto {
        name: pack.info.name.clone(),
        description: pack.info.description.clone(),
        author: pack.info.author.clone(),
        version: pack.info.version.clone(),
        source,
        verified,
        blocks: pack.content.block_defs.iter().map(block_packs::block_name).collect(),
        replaces: block_packs::existing_blocks(mac, &pack),
        capabilities: crate::commands::macro_capabilities(&pack.content),
        notes: pack.notes.clone(),
        macro_name: mac.name.clone(),
    };
    s.pending_pack = Some(PendingPack { macro_id: mac.id.clone(), pack, sha256, review });
    Ok(())
}

/// Downloads a library pack, checks its hash and opens the review.
pub(crate) fn review(state: &SharedState, app: &AppHandle, key: String) -> Result<(), String> {
    let (source_url, source, entry) = {
        let mut cache = cache();
        let (source_url, source, entry) = cache
            .packs
            .iter()
            .find(|(url, _, entry)| format!("{url}#{}", entry.info.id) == key)
            .cloned()
            .ok_or("That pack isn't in the library any more. Refresh it")?;
        cache.fetching = key;
        cache.error.clear();
        (source_url, source, entry)
    };
    emit(state, app);
    let (state, app) = (Arc::clone(state), app.clone());
    std::thread::spawn(move || {
        let result = fetch(&entry.url, is_local(&source_url)).and_then(|bytes| {
            block_packs::verify(&bytes, &entry.sha256)?;
            let text = String::from_utf8(bytes).map_err(|_| "The pack isn't text".to_string())?;
            let pack = block_packs::read_pack(&text)?;
            stage(&state, pack, entry.sha256.clone(), source, true)
        });
        {
            let mut cache = cache();
            cache.fetching.clear();
            if let Err(e) = result {
                cache.error = format!("{}: {e}", entry.info.name);
            }
        }
        emit(&state, &app);
    });
    Ok(())
}

/// Opens the review for a pack file the user picked.
pub(crate) fn review_file(state: &SharedState, app: &AppHandle, path: String) -> Result<(), String> {
    let bytes = fetch(&path, true)?;
    let sha256 = block_packs::sha256_hex(&bytes);
    let text = String::from_utf8(bytes).map_err(|_| "This isn't a block pack".to_string())?;
    let pack = block_packs::read_pack(&text)?;
    let name = std::path::Path::new(&path).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or(path);
    stage(state, pack, sha256, name, false)?;
    emit(state, app);
    Ok(())
}

/// Adds the reviewed pack's blocks to the macro it was reviewed for.
pub(crate) fn confirm(state: &SharedState, app: &AppHandle) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    let Some(pending) = s.pending_pack.take() else { return Ok(()) };
    if s.current_macro.as_ref().map(|m| &m.id) != Some(&pending.macro_id) {
        emit_state_updated(app, &s);
        return Err("The macro changed before the blocks were added. Try again".into());
    }
    crate::commands::push_undo(&mut s);
    let mac = s.current_macro.as_mut().ok_or("No macro selected")?;
    block_packs::merge_into(mac, &pending.pack, &pending.sha256);
    crate::commands::auto_save(&mut s);
    crate::commands::sync_variable_values(&mut s);
    crate::commands::sync_list_values(&mut s);
    emit_state_updated(app, &s);
    Ok(())
}

pub(crate) fn remove(state: &SharedState, app: &AppHandle, pack_id: String) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    let mac = s.current_macro.as_ref().ok_or("No macro selected")?;
    if !mac.settings.block_packs.iter().any(|r| r.id == pack_id) {
        return Err("That pack isn't installed in this macro".into());
    }
    crate::commands::push_undo(&mut s);
    block_packs::remove_from(
        s.current_macro.as_mut().ok_or("No macro selected")?,
        &pack_id,
    )?;
    s.invalid_field_buffers.clear();
    crate::commands::auto_save(&mut s);
    emit_state_updated(app, &s);
    Ok(())
}

pub(crate) fn cancel(state: &SharedState, app: &AppHandle) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.pending_pack = None;
    emit_state_updated(app, &s);
    Ok(())
}

/// Writes the chosen blocks of the open macro, and the blocks they call,
/// to a pack file. Returns how many blocks it holds.
pub(crate) fn export(state: &SharedState, block_ids: Vec<String>, info: block_packs::PackInfo, path: String) -> Result<usize, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    let mac = s.current_macro.as_ref().ok_or("No macro selected")?;
    let pack = block_packs::pack_from_macro(mac, &block_ids, info)?;
    let text = block_packs::write_pack(&pack)?;
    std::fs::write(&path, text).map_err(|e| format!("Couldn't write '{path}': {e}"))?;
    Ok(pack.content.block_defs.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_this_computer_may_use_plain_http() {
        for ok in ["http://127.0.0.1", "http://localhost:8080/index.json", "http://[::1]:9/x", "http://LOCALHOST/?a"] {
            assert!(is_loopback_http(ok), "{ok}");
        }
        for bad in [
            "http://localhost:1@example.com/index.json",
            "http://localhost@example.com",
            "http://localhost.example.com/",
            "http://127.0.0.1.example.com",
            "http://[::1].example.com",
            "http://example.com/?http://localhost",
            "https://localhost",
        ] {
            assert!(!is_loopback_http(bad), "{bad}");
        }
    }

    #[test]
    fn old_built_in_addresses_move_to_the_new_one() {
        let mut list: Vec<String> = block_packs::OLD_DEFAULT_INDEXES.iter().map(|u| u.to_string()).collect();
        list.push("https://example.com/index.json".into());
        assert_eq!(
            upgrade_sources(list),
            [block_packs::DEFAULT_INDEX.to_string(), "https://example.com/index.json".into()]
        );
    }

    #[test]
    fn downloads_follow_the_url_rules() {
        let index = concat!(env!("CARGO_MANIFEST_DIR"), "/../blockwork-core/tests/fixtures/blocks/index.json");
        assert!(fetch(index, true).is_ok(), "a local library the user added");
        assert!(fetch(&format!("file://{index}"), true).is_ok());
        let refused = fetch(index, false).unwrap_err();
        assert!(refused.contains("can't point at a file"), "{refused}");
        let refused = fetch("http://example.com/index.json", true).unwrap_err();
        assert!(refused.contains("https"), "{refused}");
        let refused = fetch("http://localhost.example.com/index.json", true).unwrap_err();
        assert!(refused.contains("https"), "loopback means this computer only: {refused}");
        assert!(fetch("ftp://example.com/index.json", true).is_err());
        assert!(is_local("/home/me/index.json") && is_local("file:///x") && !is_local("https://x"));
    }
}
