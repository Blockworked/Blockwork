//! Skips a slow path for a while after it fails, then tries it again. For
//! desktop services (KWin scripts, grim) that can be missing for good or only
//! for a moment, like while the compositor restarts.

use std::sync::Mutex;
use std::time::{Duration, Instant};

pub struct Backoff {
    retry_at: Mutex<Option<Instant>>,
    wait: Duration,
}

impl Backoff {
    pub const fn new(wait: Duration) -> Self {
        Backoff { retry_at: Mutex::new(None), wait }
    }

    /// Whether a failure was recent enough that the path should be skipped.
    pub fn waiting(&self) -> bool {
        let retry_at = self.retry_at.lock().unwrap_or_else(|e| e.into_inner());
        retry_at.is_some_and(|at| Instant::now() < at)
    }

    pub fn failed(&self) {
        *self.retry_at.lock().unwrap_or_else(|e| e.into_inner()) = Some(Instant::now() + self.wait);
    }

    pub fn succeeded(&self) {
        *self.retry_at.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retries_once_the_wait_is_over() {
        let backoff = Backoff::new(Duration::from_millis(30));
        assert!(!backoff.waiting());
        backoff.failed();
        assert!(backoff.waiting());
        std::thread::sleep(Duration::from_millis(40));
        assert!(!backoff.waiting());
        backoff.failed();
        backoff.succeeded();
        assert!(!backoff.waiting());
    }
}
