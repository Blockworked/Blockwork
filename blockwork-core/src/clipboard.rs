//! Cross-platform clipboard reads/writes, backing `Op::ClipboardText`,
//! `Op::ClipboardHasImage`, `Op::ClipboardHasFiles`, and the `SetClipboard`
//! instruction. Built on `arboard`, which abstracts Windows/macOS/X11/Wayland
//! itself, so (like `battery.rs`) there's no per-platform branching here.

/// Current clipboard contents as UTF-8 text. Errs if the clipboard is
/// unavailable or holds no text (e.g. an image or file list with no text
/// fallback).
pub fn get_text() -> Result<String, String> {
    let mut clipboard =
        arboard::Clipboard::new().map_err(|e| format!("clipboard unavailable: {e}"))?;
    clipboard
        .get_text()
        .map_err(|e| format!("failed to read clipboard text: {e}"))
}

/// Replaces the clipboard contents with the given text.
pub fn set_text(text: String) -> Result<(), String> {
    let mut clipboard =
        arboard::Clipboard::new().map_err(|e| format!("clipboard unavailable: {e}"))?;
    clipboard
        .set_text(text)
        .map_err(|e| format!("failed to set clipboard text: {e}"))
}

/// Whether the clipboard currently holds image data. Treats any failure
/// (clipboard unavailable, no image present) as `false` rather than erroring,
/// since this is a plain yes/no reporter.
pub fn has_image() -> bool {
    let Ok(mut clipboard) = arboard::Clipboard::new() else {
        return false;
    };
    clipboard.get_image().is_ok()
}

/// Whether the clipboard currently holds a non-empty file list. Same
/// permissive-`false` style as `has_image`.
pub fn has_file_list() -> bool {
    let Ok(mut clipboard) = arboard::Clipboard::new() else {
        return false;
    };
    matches!(clipboard.get().file_list(), Ok(paths) if !paths.is_empty())
}

/// Everything the clipboard held, for `SaveClipboard`/`RestoreClipboard`.
#[derive(Clone, Debug, PartialEq)]
pub enum Saved {
    Empty,
    Text(String),
    /// Rich text, with its plain text for apps that only paste that.
    Html(String, Option<String>),
    Image {
        width: usize,
        height: usize,
        rgba: Vec<u8>,
    },
    Files(Vec<std::path::PathBuf>),
}

/// Reads the whole clipboard, preferring files, then an image, then rich
/// text, then plain text.
pub fn save() -> Result<Saved, String> {
    let mut clipboard =
        arboard::Clipboard::new().map_err(|e| format!("clipboard unavailable: {e}"))?;
    if let Ok(files) = clipboard.get().file_list()
        && !files.is_empty()
    {
        return Ok(Saved::Files(files));
    }
    if let Ok(image) = clipboard.get_image() {
        return Ok(Saved::Image {
            width: image.width,
            height: image.height,
            rgba: image.bytes.into_owned(),
        });
    }
    let text = clipboard.get_text().ok();
    if let Ok(html) = clipboard.get().html()
        && !html.is_empty()
    {
        return Ok(Saved::Html(html, text));
    }
    Ok(text.map_or(Saved::Empty, Saved::Text))
}

/// Puts back what [`save`] read.
pub fn restore(saved: &Saved) -> Result<(), String> {
    let mut clipboard =
        arboard::Clipboard::new().map_err(|e| format!("clipboard unavailable: {e}"))?;
    let result = match saved {
        Saved::Empty => clipboard.clear(),
        Saved::Text(text) => clipboard.set_text(text.clone()),
        Saved::Html(html, text) => clipboard.set_html(html.clone(), text.clone()),
        Saved::Image {
            width,
            height,
            rgba,
        } => clipboard.set_image(arboard::ImageData {
            width: *width,
            height: *height,
            bytes: std::borrow::Cow::Borrowed(rgba),
        }),
        Saved::Files(files) => clipboard.set().file_list(files),
    };
    result.map_err(|e| format!("failed to set clipboard: {e}"))
}

/// A hash of the whole clipboard, to tell when it changes.
pub fn fingerprint() -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    match save() {
        Ok(Saved::Empty) | Err(_) => 0u8.hash(&mut hasher),
        Ok(Saved::Text(text)) => (1u8, text).hash(&mut hasher),
        Ok(Saved::Html(html, text)) => (2u8, html, text).hash(&mut hasher),
        Ok(Saved::Image {
            width,
            height,
            rgba,
        }) => (3u8, width, height, rgba).hash(&mut hasher),
        Ok(Saved::Files(files)) => (4u8, files).hash(&mut hasher),
    }
    hasher.finish()
}
