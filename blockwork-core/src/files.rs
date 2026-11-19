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

pub fn read_bytes(path: &str) -> Result<Vec<u8>, String> {
    let full = check(path)?;
    std::fs::read(&full).map_err(|e| format!("couldn't read {}: {e}", full.display()))
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

/// Full paths of a folder's entries, sorted by name.
pub fn list_paths(path: &str) -> Result<Vec<String>, String> {
    let full = check(path)?;
    let mut paths: Vec<_> = std::fs::read_dir(&full)
        .map_err(|e| format!("couldn't list {}: {e}", full.display()))?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .collect();
    paths.sort();
    Ok(paths
        .into_iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect())
}

/// Copies `from` to `to`, folders with everything in them. When `to` is a
/// folder that already exists, the copy goes inside it under its own name.
pub fn copy(from: &str, to: &str) -> Result<(), String> {
    let (from, to) = (check(from)?, target(from, to)?);
    copy_path(&from, &to).map_err(|e| format!("couldn't copy {}: {e}", from.display()))
}

/// Moves `from` to `to`, like [`copy`] does, across drives too.
pub fn move_path(from: &str, to: &str) -> Result<(), String> {
    let (from, to) = (check(from)?, target(from, to)?);
    let fail = |e: std::io::Error| format!("couldn't move {}: {e}", from.display());
    if std::fs::rename(&from, &to).is_ok() {
        return Ok(());
    }
    // Renames can't cross drives; copy, then remove the original.
    copy_path(&from, &to).map_err(fail)?;
    remove(&from).map_err(fail)
}

/// Deletes a file, or a folder and everything in it. Already gone is fine.
pub fn delete(path: &str) -> Result<(), String> {
    let full = check(path)?;
    match remove(&full) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
            Err(format!("couldn't delete {}: {e}", full.display()))
        }
        _ => Ok(()),
    }
}

/// Makes a folder and any missing folders above it.
pub fn make_folder(path: &str) -> Result<(), String> {
    let full = check(path)?;
    std::fs::create_dir_all(&full).map_err(|e| format!("couldn't make {}: {e}", full.display()))
}

/// A file's size in bytes, or a folder's with everything in it.
pub fn size(path: &str) -> Result<f64, String> {
    let full = check(path)?;
    fn walk(path: &std::path::Path) -> std::io::Result<u64> {
        let meta = std::fs::symlink_metadata(path)?;
        if !meta.is_dir() {
            return Ok(meta.len());
        }
        let mut total = 0;
        for entry in std::fs::read_dir(path)? {
            total += walk(&entry?.path()).unwrap_or(0);
        }
        Ok(total)
    }
    walk(&full)
        .map(|n| n as f64)
        .map_err(|e| format!("couldn't read {}: {e}", full.display()))
}

/// When a file was last changed, in seconds since 1970 like the timestamp reporter.
pub fn modified(path: &str) -> Result<f64, String> {
    let full = check(path)?;
    let at = std::fs::metadata(&full)
        .and_then(|m| m.modified())
        .map_err(|e| format!("couldn't read {}: {e}", full.display()))?;
    Ok(at
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0.0, |d| d.as_secs_f64()))
}

/// Where a copy or move of `from` lands.
fn target(from: &str, to: &str) -> Result<PathBuf, String> {
    let to = check(to)?;
    if to.is_dir()
        && let Some(name) = expand(from).file_name()
    {
        return Ok(to.join(name));
    }
    Ok(to)
}

fn copy_path(from: &std::path::Path, to: &std::path::Path) -> std::io::Result<()> {
    if !std::fs::symlink_metadata(from)?.is_dir() {
        return std::fs::copy(from, to).map(|_| ());
    }
    if to.starts_with(from) {
        return Err(std::io::Error::other("can't copy a folder into itself"));
    }
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        copy_path(&entry.path(), &to.join(entry.file_name()))?;
    }
    Ok(())
}

fn remove(path: &std::path::Path) -> std::io::Result<()> {
    if std::fs::symlink_metadata(path)?.is_dir() {
        std::fs::remove_dir_all(path)
    } else {
        std::fs::remove_file(path)
    }
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
    fn copies_moves_and_deletes_files_and_folders() {
        let dir = std::env::temp_dir().join(format!("blockwork-copy-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let p = |rest: &str| dir.join(rest).to_string_lossy().into_owned();
        make_folder(&p("a/b")).unwrap();
        write_text(&p("a/b/note.txt"), "hello", false).unwrap();
        assert_eq!(size(&p("a")).unwrap(), 5.0);
        assert!(modified(&p("a/b/note.txt")).unwrap() > 1.0e9);

        // Into an existing folder keeps the name.
        make_folder(&p("c")).unwrap();
        copy(&p("a"), &p("c")).unwrap();
        assert_eq!(read_text(&p("c/a/b/note.txt")).unwrap(), "hello");
        assert!(copy(&p("a"), &p("a/b")).is_err());

        move_path(&p("a/b/note.txt"), &p("moved.txt")).unwrap();
        assert!(!exists(&p("a/b/note.txt")) && exists(&p("moved.txt")));

        delete(&p("c")).unwrap();
        delete(&p("c")).unwrap();
        assert!(!exists(&p("c")));
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
