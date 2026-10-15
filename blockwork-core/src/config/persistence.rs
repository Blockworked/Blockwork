use crate::macros::migrate::{self, Upgrade};
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
    read_macro_file_upgrading(path).map(|(mac, _)| mac)
}

/// [`read_macro_file`] plus what upgrading it from an older version changed.
pub fn read_macro_file_upgrading(path: &Path) -> Result<(Macro, Upgrade), String> {
    let contents = fs::read_to_string(path)
        .map_err(|err| format!("Failed to read macro file '{}': {}", path.display(), err))?;
    let (mut mac, upgrade) = migrate::parse(&contents)
        .map_err(|err| format!("Failed to load macro file '{}': {}", path.display(), err))?;
    mac.ensure_id();
    Ok((mac, upgrade))
}

/// Loads one of the app's own macro files, rewriting it in the current
/// format. When blocks changed, the original is kept as `<file>.v<N>.bak`.
fn load_own_macro_file(path: &Path) -> Result<Macro, String> {
    let (mac, upgrade) = read_macro_file_upgrading(path)?;
    if upgrade.changed() {
        if !upgrade.notes.is_empty() {
            let backup = path.with_extension(format!("{MACRO_FILE_EXTENSION}.v{}.bak", upgrade.from));
            if !backup.exists()
                && let Err(e) = fs::copy(path, &backup)
            {
                warn!("Failed to back up '{}' before upgrading it: {e}", path.display());
                return Ok(mac);
            }
            let source = crate::run_log::Source::for_macro(&mac.id, &mac.name);
            for note in &upgrade.notes {
                crate::run_log::info(&source, format!("{note} (the old file is kept as {})", backup.display()));
            }
        }
        if let Err(e) = write_macro_file(path, &mac) {
            warn!("Failed to save upgraded macro: {e}");
        }
    }
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

    let mac = match load_own_macro_file(&path) {
        Ok(mac) => mac,
        Err(err) => {
            warn!("{}", err);
            return None;
        }
    };
    // The upgrade may have rewritten the file.
    let mtime = fs::metadata(&path).and_then(|m| m.modified()).unwrap_or(mtime);
    macro_cache().lock().unwrap().insert(id.to_string(), (mtime, mac.clone()));
    Some(mac)
}

/// A copy of `mac` with secret values masked in its saved variables and
/// lists, or `None` when it holds none.
fn without_secrets(mac: &Macro) -> Option<Macro> {
    use crate::input::value::Evaluated;
    use blockstitch_core::graph::ListItem;
    let secret_var = |v: &Evaluated| matches!(v, Evaluated::Text(t) if crate::secrets::contains_secret(t));
    let secret_item = |i: &ListItem| matches!(i, ListItem::Text(t) if crate::secrets::contains_secret(t));
    if !mac.variables.iter().any(|v| secret_var(&v.value)) && !mac.lists.iter().any(|l| l.items.iter().any(secret_item)) {
        return None;
    }
    let mut clean = mac.clone();
    for var in &mut clean.graph.variables {
        if let Evaluated::Text(t) = &mut var.value {
            *t = crate::secrets::redact(t);
        }
    }
    for list in &mut clean.graph.lists {
        for item in &mut list.items {
            if let ListItem::Text(t) = item {
                *t = crate::secrets::redact(t);
            }
        }
    }
    Some(clean)
}

pub fn write_macro_file(path: &Path, mac: &Macro) -> Result<(), String> {
    let clean = without_secrets(mac);
    let mac = clean.as_ref().unwrap_or(mac);
    let serialized = migrate::to_value(mac)
        .and_then(|value| serde_json::to_string_pretty(&value).map_err(|e| e.to_string()))
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
        crate::permissions::trust(&self.id);
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
        match load_own_macro_file(&path) {
            Ok(mac) => macros.push(mac),
            Err(err) => {
                warn!("{}", err);
                // Listing runs often; report each broken file once.
                static REPORTED: Mutex<Vec<String>> = Mutex::new(Vec::new());
                if let Ok(mut reported) = REPORTED.lock()
                    && !reported.contains(&err)
                {
                    reported.push(err.clone());
                    crate::run_log::error(&crate::run_log::Source::app("Blockwork"), err);
                }
            }
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

/// Top-level key marking a `.blockwork` bundle: several macros in one file.
const BUNDLE_KEY: &str = "blockwork_bundle";

/// What an import file holds, upgraded to the current format.
pub struct ImportFile {
    /// The bundle's name, or the macro's for a single macro file.
    pub name: String,
    pub macros: Vec<Macro>,
    /// What upgrading older macros changed.
    pub notes: Vec<String>,
}

/// Reads a single macro file or a bundle.
pub fn read_import_file(path: &Path) -> Result<ImportFile, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("Couldn't read '{}': {e}", path.display()))?;
    let value: serde_json::Value = serde_json::from_str(&text).map_err(|e| format!("'{}' isn't a macro file: {e}", path.display()))?;
    let Some(version) = value.get(BUNDLE_KEY) else {
        let (mut mac, upgrade) = migrate::from_value(value).map_err(|e| format!("Couldn't import '{}': {e}", path.display()))?;
        mac.ensure_id();
        return Ok(ImportFile { name: mac.name.clone(), macros: vec![mac], notes: upgrade.notes });
    };
    if version.as_u64().is_none_or(|v| v > 1) {
        return Err("This bundle was made by a newer version of Blockwork. Update Blockwork to import it".into());
    }
    let mut out = ImportFile {
        name: value.get("name").and_then(|n| n.as_str()).unwrap_or("Bundle").to_string(),
        macros: Vec::new(),
        notes: Vec::new(),
    };
    for entry in value.get("macros").and_then(|m| m.as_array()).into_iter().flatten() {
        let (mut mac, upgrade) = migrate::from_value(entry.clone()).map_err(|e| format!("Couldn't import a macro in the bundle: {e}"))?;
        mac.ensure_id();
        out.notes.extend(upgrade.notes.into_iter().map(|n| format!("{}: {n}", mac.name)));
        out.macros.push(mac);
    }
    if out.macros.is_empty() {
        return Err("The bundle has no macros in it".into());
    }
    Ok(out)
}

/// Writes `macros` to one `.blockwork` bundle, secrets masked like a single export.
pub fn write_bundle(path: &Path, name: &str, macros: &[Macro]) -> Result<(), String> {
    let entries = macros
        .iter()
        .map(|mac| migrate::to_value(without_secrets(mac).as_ref().unwrap_or(mac)))
        .collect::<Result<Vec<_>, _>>()?;
    let bundle = serde_json::json!({ BUNDLE_KEY: 1, "name": name, "macros": entries });
    let text = serde_json::to_string_pretty(&bundle).map_err(|e| e.to_string())?;
    fs::write(path, text).map_err(|e| format!("Couldn't write '{}': {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundles_round_trip_and_single_files_import_too() {
        let dir = std::env::temp_dir().join(format!("bw-bundle-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let a = Macro::new("A".into(), String::new(), Vec::new());
        let b = Macro::new("B".into(), String::new(), Vec::new());

        let bundle = dir.join("pack.blockwork");
        write_bundle(&bundle, "Pack", &[a.clone(), b.clone()]).unwrap();
        let read = read_import_file(&bundle).unwrap();
        assert_eq!(read.name, "Pack");
        assert_eq!(read.macros, vec![a.clone(), b]);

        let single = dir.join("a.macro");
        write_macro_file(&single, &a).unwrap();
        let read = read_import_file(&single).unwrap();
        assert_eq!((read.name.as_str(), read.macros.len()), ("A", 1));
        let _ = fs::remove_dir_all(&dir);
    }
}
