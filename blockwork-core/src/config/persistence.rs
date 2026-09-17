use crate::macros::Macro;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::SystemTime;
use tracing::warn;

const MACROS_DIR_NAME: &str = "macros";
const MACRO_FILE_EXTENSION: &str = "json";

/// Per-id cache of the last parse, keyed off the file's mtime at parse time.
/// Exists so a long-lived, stateless reader (the Linux Wine bridge process,
/// which re-resolves the active macro on every single `RunMacro` dispatch -
/// i.e. every level attempt) doesn't re-read-and-reparse the same JSON file
/// from disk on every attempt. A `stat()` to check mtime still happens every
/// call (cheap, low-variance) so an external edit (the desktop app saving a
/// change, or this same process's own post-run variable-persistence save) is
/// picked up immediately rather than needing explicit invalidation.
static MACRO_CACHE: OnceLock<Mutex<HashMap<String, (SystemTime, Macro)>>> = OnceLock::new();

fn macro_cache() -> &'static Mutex<HashMap<String, (SystemTime, Macro)>> {
    MACRO_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn ensure_macros_dir() -> Result<PathBuf, String> {
    let mut path = super::config_root()?;
    path.push(MACROS_DIR_NAME);
    fs::create_dir_all(&path)
        .map_err(|err| format!("Failed to create macros directory '{}': {}", path.display(), err))?;
    Ok(path)
}

fn macro_file_path(id: &str) -> Result<PathBuf, String> {
    if id.trim().is_empty() {
        return Err("Cannot build macro path with empty id".to_string());
    }
    let mut path = ensure_macros_dir()?;
    path.push(format!("{}.{}", id, MACRO_FILE_EXTENSION));
    Ok(path)
}

pub fn read_macro_file(path: &Path) -> Result<Macro, String> {
    let contents = fs::read_to_string(path)
        .map_err(|err| format!("Failed to read macro file '{}': {}", path.display(), err))?;
    let mut mac: Macro = serde_json::from_str(&contents)
        .map_err(|err| format!("Failed to parse macro file '{}': {}", path.display(), err))?;
    mac.ensure_id();
    Ok(mac)
}

/// Reads and parses the single macro file for `id` directly, without
/// scanning or parsing every other macro in the directory - `get_macro_by_id`
/// used to go through `get_macros_from_config()`, which does exactly that
/// fan-out for what's ultimately a single lookup. Backed by `MACRO_CACHE`:
/// skips the read+parse entirely when the file's mtime matches what's
/// cached, which is the common case for a level-attempt retry loop
/// re-resolving the same macro on every attempt.
pub fn read_macro_by_id(id: &str) -> Option<Macro> {
    let path = macro_file_path(id).ok()?;
    let mtime = fs::metadata(&path).and_then(|m| m.modified()).ok()?;

    {
        let cache = macro_cache().lock().unwrap();
        if let Some((cached_mtime, mac)) = cache.get(id) {
            if *cached_mtime == mtime {
                return Some(mac.clone());
            }
        }
    }

    let mac = match read_macro_file(&path) {
        Ok(mac) => mac,
        Err(err) => {
            warn!("{}", err);
            return None;
        }
    };
    macro_cache().lock().unwrap().insert(id.to_string(), (mtime, mac.clone()));
    Some(mac)
}

pub fn write_macro_file(path: &Path, mac: &Macro) -> Result<(), String> {
    let serialized = serde_json::to_string_pretty(mac)
        .map_err(|err| format!("Failed to serialize macro '{}': {}", mac.name, err))?;

    let mut temp_path = path.to_path_buf();
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| format!("Invalid macro file name '{}'", path.display()))?;
    temp_path.set_file_name(format!("{}.tmp", file_name));

    fs::write(&temp_path, serialized)
        .map_err(|err| format!("Failed to write temporary macro file '{}': {}", temp_path.display(), err))?;

    if path.exists() {
        fs::remove_file(path)
            .map_err(|err| format!("Failed to replace macro file '{}': {}", path.display(), err))?;
    }

    fs::rename(&temp_path, path)
        .map_err(|err| format!("Failed to finalize macro file '{}': {}", path.display(), err))?;

    Ok(())
}

fn list_macro_file_paths() -> Result<Vec<PathBuf>, String> {
    let dir = ensure_macros_dir()?;
    let entries = fs::read_dir(&dir)
        .map_err(|err| format!("Failed to scan macros directory '{}': {}", dir.display(), err))?;

    let mut paths = entries
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| path.is_file())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some(MACRO_FILE_EXTENSION))
        .collect::<Vec<_>>();
    paths.sort();
    Ok(paths)
}

impl Macro {
    pub fn save(&self) -> Result<(), String> {
        let path = macro_file_path(&self.id)?;
        write_macro_file(&path, self)?;
        // Belt-and-suspenders alongside the mtime check in `read_macro_by_id`:
        // a rapid save-then-reload in the same process could otherwise land
        // within one mtime tick and see a stale cache entry.
        macro_cache().lock().unwrap().remove(&self.id);
        Ok(())
    }

    pub fn remove(self) -> Result<(), String> {
        let path = macro_file_path(&self.id)?;
        if path.exists() {
            fs::remove_file(&path)
                .map_err(|err| format!("Failed to remove macro file '{}': {}", path.display(), err))?;
        }
        macro_cache().lock().unwrap().remove(&self.id);
        Ok(())
    }
}

pub fn get_macros_from_config() -> Vec<Macro> {
    let mut macros = Vec::new();
    let paths = match list_macro_file_paths() {
        Ok(paths) => paths,
        Err(err) => {
            warn!("Failed to scan macro files: {}", err);
            return macros;
        }
    };

    for path in paths {
        match read_macro_file(&path) {
            Ok(mac) => macros.push(mac),
            Err(err) => warn!("{}", err),
        }
    }

    macros.sort_by(|left, right| {
        let name_order = left.name.to_lowercase().cmp(&right.name.to_lowercase());
        if name_order.is_eq() {
            left.id.cmp(&right.id)
        } else {
            name_order
        }
    });

    macros
}
