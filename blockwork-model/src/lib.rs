//! Blockwork's blocks and documents with nothing that touches the machine:
//! the instruction vocabulary, saved macros and their migrations, block
//! packs, canvas edits and the editor's wire shape. `blockwork-core` adds the
//! runner and everything else that reads or drives this computer, and the
//! website's editor (`blockwork-web`) links this crate alone.

pub mod block_packs;
pub mod capabilities;
pub mod editing;
pub mod frontend;
pub mod hotkey_types;
pub mod input;
pub mod key_mapping;
pub mod macros;
pub mod redact;
pub mod text;

/// Registers the pure value operators with blockstitch, so a saved macro
/// using them evaluates. `blockwork_core::init` registers these and the rest.
pub fn init() {
    input::value::register_text_operators();
}
