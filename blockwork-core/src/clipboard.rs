//! Cross-platform clipboard reads/writes, backing `Op::ClipboardText`,
//! `Op::ClipboardHasImage`, `Op::ClipboardHasFiles`, and the `SetClipboard`
//! instruction. Built on `arboard`, which abstracts Windows/macOS/X11/Wayland
//! itself, so (like `battery.rs`) there's no per-platform branching here.

/// Current clipboard contents as UTF-8 text. Errs if the clipboard is
/// unavailable or holds no text (e.g. an image or file list with no text
/// fallback).
pub fn get_text() -> Result<String, String> {
    let mut clipboard = arboard::Clipboard::new().map_err(|e| format!("clipboard unavailable: {e}"))?;
    clipboard.get_text().map_err(|e| format!("failed to read clipboard text: {e}"))
}

/// Replaces the clipboard contents with the given text.
pub fn set_text(text: String) -> Result<(), String> {
    let mut clipboard = arboard::Clipboard::new().map_err(|e| format!("clipboard unavailable: {e}"))?;
    clipboard.set_text(text).map_err(|e| format!("failed to set clipboard text: {e}"))
}

/// Whether the clipboard currently holds image data. Treats any failure
/// (clipboard unavailable, no image present) as `false` rather than erroring,
/// since this is a plain yes/no reporter.
pub fn has_image() -> bool {
    let Ok(mut clipboard) = arboard::Clipboard::new() else { return false };
    clipboard.get_image().is_ok()
}

/// Whether the clipboard currently holds a non-empty file list. Same
/// permissive-`false` style as `has_image`.
pub fn has_file_list() -> bool {
    let Ok(mut clipboard) = arboard::Clipboard::new() else { return false };
    matches!(clipboard.get().file_list(), Ok(paths) if !paths.is_empty())
}
