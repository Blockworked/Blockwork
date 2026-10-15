//! Named locks for `WithLock`: one strand at a time, across every macro.
//! Re-entrant per thread, so a lock block nested in the same lock is fine.

use std::collections::HashMap;
use std::sync::{Condvar, Mutex};
use std::thread::ThreadId;
use std::time::Duration;

struct Held {
    owner: ThreadId,
    depth: u32,
}

static LOCKS: Mutex<Option<HashMap<String, Held>>> = Mutex::new(None);
static RELEASED: Condvar = Condvar::new();

/// Releases the lock when dropped.
pub struct LockGuard {
    name: String,
}

impl Drop for LockGuard {
    fn drop(&mut self) {
        let mut locks = LOCKS.lock().unwrap_or_else(|e| e.into_inner());
        let map = locks.get_or_insert_with(HashMap::new);
        if let Some(held) = map.get_mut(&self.name) {
            held.depth -= 1;
            if held.depth == 0 {
                map.remove(&self.name);
                RELEASED.notify_all();
            }
        }
    }
}

/// Waits for the lock called `name`. Returns `None` if `cancelled` says to
/// give up first; it's checked every `poll`.
pub fn acquire(name: &str, poll: Duration, mut cancelled: impl FnMut() -> bool) -> Option<LockGuard> {
    let me = std::thread::current().id();
    let mut locks = LOCKS.lock().unwrap_or_else(|e| e.into_inner());
    loop {
        let map = locks.get_or_insert_with(HashMap::new);
        match map.get_mut(name) {
            None => {
                map.insert(name.to_string(), Held { owner: me, depth: 1 });
                return Some(LockGuard { name: name.to_string() });
            }
            Some(held) if held.owner == me => {
                held.depth += 1;
                return Some(LockGuard { name: name.to_string() });
            }
            Some(_) => {}
        }
        if cancelled() {
            return None;
        }
        locks = RELEASED.wait_timeout(locks, poll).unwrap_or_else(|e| e.into_inner()).0;
    }
}

/// Whether some strand holds the lock called `name`.
pub fn is_held(name: &str) -> bool {
    LOCKS.lock().unwrap_or_else(|e| e.into_inner()).as_ref().is_some_and(|m| m.contains_key(name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[test]
    fn one_holder_at_a_time_and_reentrant() {
        let inside = Arc::new(AtomicU32::new(0));
        let most = Arc::new(AtomicU32::new(0));
        let threads: Vec<_> = (0..4)
            .map(|_| {
                let (inside, most) = (Arc::clone(&inside), Arc::clone(&most));
                std::thread::spawn(move || {
                    for _ in 0..20 {
                        let _outer = acquire("test-shared", Duration::from_millis(5), || false).unwrap();
                        let _inner = acquire("test-shared", Duration::from_millis(5), || false).unwrap();
                        let n = inside.fetch_add(1, Ordering::SeqCst) + 1;
                        most.fetch_max(n, Ordering::SeqCst);
                        std::thread::sleep(Duration::from_micros(200));
                        inside.fetch_sub(1, Ordering::SeqCst);
                    }
                })
            })
            .collect();
        for t in threads {
            t.join().unwrap();
        }
        assert_eq!(most.load(Ordering::SeqCst), 1);
        assert!(!is_held("test-shared"));
    }

    #[test]
    fn waiting_can_be_cancelled() {
        let _held = acquire("test-cancel", Duration::from_millis(5), || false).unwrap();
        let waited = std::thread::spawn(|| acquire("test-cancel", Duration::from_millis(5), || true).is_none()).join().unwrap();
        assert!(waited);
    }
}
