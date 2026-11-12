//! Native providers. Kept outside portable contact planning and scheduling.
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::{cancel, frame, prepare};
#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::{cancel, frame, prepare};
