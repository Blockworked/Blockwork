//! The background watchers that start trigger blocks (`When...`), one per
//! kind of event. Each `start` runs on its own thread for the life of the
//! daemon, finds the strands its trigger heads in every macro, and runs them
//! when the event happens.

pub(crate) mod battery;
pub(crate) mod clipboard;
pub(crate) mod file;
pub(crate) mod key;
pub(crate) mod mouse;
pub(crate) mod system;
pub(crate) mod time;
pub(crate) mod typed;
pub(crate) mod window;
