//! Masking secret values in text the editor shows or exports. The secrets
//! live in the keyring, so the host installs the masker; without one,
//! text passes through unchanged.

use std::sync::OnceLock;

static MASKER: OnceLock<fn(&str) -> String> = OnceLock::new();

/// Installs the function that masks secrets. Only the first call counts.
pub fn set_masker(mask: fn(&str) -> String) {
    let _ = MASKER.set(mask);
}

/// `text` with every known secret masked.
pub fn redact(text: &str) -> String {
    MASKER
        .get()
        .map_or_else(|| text.to_string(), |mask| mask(text))
}
