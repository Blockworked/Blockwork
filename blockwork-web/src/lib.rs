//! Blockwork's editor without the app: one in-memory session per browser tab,
//! answering the same commands the desktop editor sends, over the shared
//! blockstitch C ABI (`bs_*` in blockstitch's `include/blockstitch_ffi.h`).

mod session;

pub use session::{COMMANDS, Demo};

blockstitch_ffi::export_abi!(Demo);
