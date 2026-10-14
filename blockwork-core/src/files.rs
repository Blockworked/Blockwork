//! File reads and writes behind the file blocks. A leading `~` means the
//! home folder.

use std::io::Write;
use std::path::PathBuf;
use std::time::SystemTime;

pub fn expand(path: &str) -> PathBuf {
    let path = path.trim();
    if let Some(rest) = path.strip_prefix('~')
        && (rest.is_empty() || rest.starts_with('/') || rest.starts_with('\\'))
        && let Some(home) = dirs::home_dir()
    {
        return home.join(rest.trim_start_matches(['/', '\\']));
    }
    PathBuf::from(path)
}

fn check(path: &str) -> Result<PathBuf, String> {
    if path.trim().is_empty() {
        return Err("no file path given".into());
    }
    Ok(expand(path))
}

pub fn read_text(path: &str) -> Result<String, String> {
    let full = check(path)?;
    std::fs::read_to_string(&full).map_err(|e| format!("couldn't read {}: {e}", full.display()))
}

pub fn write_text(path: &str, text: &str, append: bool) -> Result<(), String> {
    let full = check(path)?;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .append(append)
        .truncate(!append)
        .open(&full)
        .map_err(|e| format!("couldn't open {}: {e}", full.display()))?;
    file.write_all(text.as_bytes())
        .map_err(|e| format!("couldn't write {}: {e}", full.display()))
}

pub fn exists(path: &str) -> bool {
    !path.trim().is_empty() && expand(path).exists()
}

/// Entry names in a folder, sorted, one per line.
pub fn list_dir(path: &str) -> Result<String, String> {
    let full = check(path)?;
    let mut names: Vec<String> = std::fs::read_dir(&full)
        .map_err(|e| format!("couldn't list {}: {e}", full.display()))?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    Ok(names.join("\n"))
}

/// What the file watcher compares between polls; `None` if it's missing.
pub fn stamp(path: &str) -> Option<(SystemTime, u64)> {
    let meta = std::fs::metadata(expand(path)).ok()?;
    Some((meta.modified().ok()?, meta.len()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_appends_reads_and_lists() {
        let dir = std::env::temp_dir().join(format!("blockwork-files-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("note.txt");
        let path = file.to_str().unwrap();

        write_text(path, "one", false).unwrap();
        write_text(path, " two", true).unwrap();
        assert_eq!(read_text(path).unwrap(), "one two");
        write_text(path, "fresh", false).unwrap();
        assert_eq!(read_text(path).unwrap(), "fresh");

        assert!(exists(path));
        assert!(!exists(""));
        assert_eq!(list_dir(dir.to_str().unwrap()).unwrap(), "note.txt");
        assert!(read_text(dir.join("missing").to_str().unwrap()).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn tilde_means_home() {
        let home = dirs::home_dir().unwrap();
        assert_eq!(expand("~/a.txt"), home.join("a.txt"));
        assert_eq!(expand("~"), home);
        assert_eq!(expand("~other/a"), PathBuf::from("~other/a"));
    }
}
