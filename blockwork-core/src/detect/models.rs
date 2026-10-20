//! Built-in detection models: `.onnx` files in the config folder's `models`
//! directory, plus known models fetched on first use (with consent) and
//! pinned by hash.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// A model Blockwork knows how to fetch.
pub struct KnownModel {
    pub id: &'static str,
    pub about: &'static str,
    /// Where it's hosted; `None` until it's published.
    pub url: Option<&'static str>,
    pub sha256: &'static str,
    pub megabytes: u32,
}

pub const KNOWN: &[KnownModel] = &[KnownModel {
    id: "mc-player-320-v1",
    about: "YOLOv8n fine-tuned on Minecraft players, 320 px input",
    url: Some("https://github.com/Blockworked/blockwork-mc-player-model/releases/download/mc-player-320-v1/mc-player-320-v1.onnx"),
    sha256: "e49f9fbd475894bca3fdca2074d52e68454ca71dd5b9b36fe8f427686e61ca7c",
    megabytes: 12,
}];

/// Downloads larger than this are refused, whatever the registry says.
const MAX_BYTES: u64 = 512 * 1024 * 1024;

pub fn models_dir() -> Result<PathBuf, String> {
    Ok(crate::config::config_root()?.join("models"))
}

/// Ids of the models in the models folder.
pub fn installed() -> Vec<String> {
    models_dir().map(|dir| installed_in(&dir)).unwrap_or_default()
}

fn installed_in(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "onnx"))
        .filter_map(|p| Some(p.file_stem()?.to_string_lossy().into_owned()))
        .collect();
    names.sort();
    names
}

/// One row of the model picker: a model that's installed, or a known one
/// that can be downloaded.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ModelEntry {
    pub id: String,
    pub about: String,
    pub installed: bool,
    /// Whether it can be fetched (a known model that has been published).
    pub downloadable: bool,
    pub megabytes: Option<u32>,
}

/// Known models first, then any other `.onnx` files in the models folder.
pub fn catalog() -> Result<Vec<ModelEntry>, String> {
    Ok(catalog_in(&models_dir()?))
}

fn catalog_in(dir: &Path) -> Vec<ModelEntry> {
    let have = installed_in(dir);
    let mut rows: Vec<ModelEntry> = KNOWN
        .iter()
        .map(|k| ModelEntry {
            id: k.id.to_string(),
            about: k.about.to_string(),
            installed: have.iter().any(|h| h == k.id),
            downloadable: k.url.is_some(),
            megabytes: Some(k.megabytes),
        })
        .collect();
    for id in have {
        if !KNOWN.iter().any(|k| k.id == id) {
            rows.push(ModelEntry { id, about: "A model file you added".into(), installed: true, downloadable: false, megabytes: None });
        }
    }
    rows
}

/// Downloads a known model (the caller has the user's consent) and returns its id.
pub fn install(id: &str) -> Result<String, String> {
    let known = KNOWN.iter().find(|k| k.id == id).ok_or_else(|| format!("there's no model '{id}'"))?;
    let url = known.url.ok_or_else(|| format!("the model '{id}' isn't published yet"))?;
    let dest = models_dir()?.join(format!("{id}.onnx"));
    download(url, known.sha256, &dest)?;
    Ok(id.to_string())
}

/// Copies a `.onnx` file into the models folder and returns the id it's
/// known by (its file name without the extension).
pub fn import(path: &str) -> Result<String, String> {
    import_in(&models_dir()?, path)
}

fn import_in(dir: &Path, path: &str) -> Result<String, String> {
    let source = crate::files::expand(path.trim());
    if !source.is_file() || source.extension().is_none_or(|x| !x.eq_ignore_ascii_case("onnx")) {
        return Err("pick a .onnx model file".into());
    }
    let id = source.file_stem().and_then(|s| s.to_str()).ok_or("that file name isn't usable")?.to_string();
    std::fs::create_dir_all(dir).map_err(|e| format!("couldn't create {}: {e}", dir.display()))?;
    std::fs::copy(&source, dir.join(format!("{id}.onnx"))).map_err(|e| format!("couldn't copy the model: {e}"))?;
    Ok(id)
}

/// The model file for a block's detector text: a path to a `.onnx` file,
/// or the id of an installed or known model. `allow_download` is asked
/// before fetching a known model that isn't installed yet.
pub fn resolve(name: &str, allow_download: &dyn Fn(&KnownModel) -> bool) -> Result<PathBuf, String> {
    resolve_in(&models_dir()?, name, allow_download)
}

fn resolve_in(dir: &Path, name: &str, allow_download: &dyn Fn(&KnownModel) -> bool) -> Result<PathBuf, String> {
    let name = name.trim();
    if name.to_ascii_lowercase().ends_with(".onnx") {
        let path = crate::files::expand(name);
        return if path.is_file() { Ok(path) } else { Err(format!("there's no model file at {name}")) };
    }
    let path = dir.join(format!("{name}.onnx"));
    if path.is_file() {
        return Ok(path);
    }
    let Some(known) = KNOWN.iter().find(|k| k.id == name) else {
        return Err(format!(
            "there's no model '{name}' (put a .onnx file in {}, or use a local detector URL)",
            dir.display()
        ));
    };
    let Some(url) = known.url else {
        return Err(format!("the model '{name}' isn't published yet; put its .onnx file in {}", dir.display()));
    };
    if !allow_download(known) {
        return Err(format!("downloading the model '{name}' wasn't allowed"));
    }
    download(url, known.sha256, &path)?;
    Ok(path)
}

/// Fetches `url` into `dest`, keeping it only if its SHA-256 is `sha256`.
pub fn download(url: &str, sha256: &str, dest: &Path) -> Result<(), String> {
    let bytes = crate::http::get_bytes(url, MAX_BYTES)?;
    let got = hex(&Sha256::digest(&bytes));
    if !got.eq_ignore_ascii_case(sha256.trim()) {
        return Err(format!("the downloaded model doesn't match its pinned hash (got {got})"));
    }
    if let Some(dir) = dest.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("couldn't create {}: {e}", dir.display()))?;
    }
    let partial = dest.with_extension("onnx.part");
    std::fs::write(&partial, &bytes).map_err(|e| format!("couldn't save the model: {e}"))?;
    std::fs::rename(&partial, dest).map_err(|e| format!("couldn't save the model: {e}"))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Asks before a macro downloads a model: the network permission for
/// restricted macros, then a yes/no for everyone.
pub fn ask_to_download(model: &KnownModel) -> bool {
    if crate::permissions::require("network").is_err() {
        return false;
    }
    let message = format!(
        "Download the detection model '{}' ({}, about {} MB) from {}? It's saved in Blockwork's models folder.",
        model.id,
        model.about,
        model.megabytes,
        model.url.unwrap_or_default()
    );
    let request = crate::prompt::Request::new(crate::prompt::Kind::Confirm, "Blockwork", message);
    crate::prompt::ask(&request, &|| false).is_ok_and(|r| r.answer == "yes")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("bw-models-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn names_resolve_to_files_ids_or_clear_errors() {
        let dir = temp_dir("resolve");
        std::fs::write(dir.join("players.onnx"), b"x").unwrap();
        let never = |_: &KnownModel| panic!("nothing should download");
        assert_eq!(resolve_in(&dir, " players ", &never), Ok(dir.join("players.onnx")));
        let file = dir.join("players.onnx").display().to_string();
        assert_eq!(resolve_in(&dir, &file, &never), Ok(dir.join("players.onnx")));
        assert!(resolve_in(&dir, "/nope/missing.onnx", &never).unwrap_err().contains("no model file"));
        assert!(resolve_in(&dir, "mc-player-320-v1", &|_| false).unwrap_err().contains("wasn't allowed"));
        assert!(resolve_in(&dir, "mystery", &never).unwrap_err().contains("there's no model 'mystery'"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn downloads_keep_only_files_matching_their_hash() {
        use std::io::{Read, Write};
        let dir = temp_dir("download");
        let body = b"pretend model bytes";
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/model.onnx", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            for _ in 0..2 {
                let (mut stream, _) = listener.accept().unwrap();
                let mut request = [0u8; 1024];
                let _ = stream.read(&mut request);
                let head = format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
                stream.write_all(head.as_bytes()).unwrap();
                stream.write_all(body).unwrap();
            }
        });
        let good = hex(&Sha256::digest(body));
        let dest = dir.join("m.onnx");
        assert!(download(&url, &"0".repeat(64), &dest).unwrap_err().contains("pinned hash"));
        assert!(!dest.exists());
        download(&url, &good, &dest).unwrap();
        assert_eq!(std::fs::read(&dest).unwrap(), body);
        server.join().unwrap();
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn the_catalog_lists_known_models_then_added_ones_and_import_copies_files() {
        let dir = temp_dir("catalog");
        let src = temp_dir("catalog-src");
        std::fs::write(src.join("mine.onnx"), b"x").unwrap();
        std::fs::write(src.join("notes.txt"), b"x").unwrap();
        assert!(import_in(&dir, &src.join("notes.txt").display().to_string()).is_err());
        assert_eq!(import_in(&dir, &src.join("mine.onnx").display().to_string()), Ok("mine".to_string()));
        let rows = catalog_in(&dir);
        assert_eq!(rows[0].id, "mc-player-320-v1");
        assert!(!rows[0].installed);
        assert!(rows[0].downloadable);
        assert_eq!(rows[0].megabytes, Some(12));
        assert_eq!(rows.last().map(|r| (r.id.as_str(), r.installed)), Some(("mine", true)));
        let _ = std::fs::remove_dir_all(dir);
        let _ = std::fs::remove_dir_all(src);
    }
}
