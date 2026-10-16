//! zbus runs its background tasks on Tokio once any crate in the build
//! turns on its `tokio` feature (the daemon's tray does), and then panics on
//! threads outside a runtime. `enter` gives those threads one.

use std::sync::OnceLock;
use tokio::runtime::{EnterGuard, Handle, Runtime};

/// Enters a Tokio runtime unless this thread is already in one. Hold the
/// guard across zbus calls that start tasks (object servers, signals).
pub fn enter() -> Option<EnterGuard<'static>> {
    if Handle::try_current().is_ok() {
        return None;
    }
    static RUNTIME: OnceLock<Option<Runtime>> = OnceLock::new();
    RUNTIME
        .get_or_init(|| {
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(1)
                .thread_name("blockwork-dbus")
                .enable_all()
                .build()
                .ok()
        })
        .as_ref()
        .map(Runtime::enter)
}

#[cfg(test)]
mod tests {
    #[test]
    fn plain_threads_get_a_runtime() {
        std::thread::spawn(|| {
            let _guard = super::enter();
            assert!(tokio::runtime::Handle::try_current().is_ok());
        })
        .join()
        .unwrap();
    }
}
