//! Which block each running strand is on, so the editor can highlight it
//! live. Hosts poll [`revision`] and show [`snapshot`].

use serde::Serialize;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ActiveBlock {
    pub macro_id: String,
    pub block_id: String,
}

/// (strand key, block) for every strand that is running right now.
static ACTIVE: Mutex<Vec<(u64, ActiveBlock)>> = Mutex::new(Vec::new());
static NEXT_KEY: AtomicU64 = AtomicU64::new(0);
/// Bumped whenever a strand moves to another block, starts or ends.
static REVISION: AtomicU64 = AtomicU64::new(0);

/// One strand's slot in the trace; removed again when dropped.
pub(crate) struct Tracker {
    key: u64,
}

impl Tracker {
    /// `None` for runs that don't belong to a macro (nothing to highlight).
    pub(crate) fn new(macro_id: &str) -> Option<Tracker> {
        if macro_id.is_empty() {
            return None;
        }
        let key = NEXT_KEY.fetch_add(1, Ordering::Relaxed);
        let block = ActiveBlock { macro_id: macro_id.to_string(), block_id: String::new() };
        ACTIVE.lock().ok()?.push((key, block));
        Some(Tracker { key })
    }

    /// The strand is now running `block_id`.
    pub(crate) fn at(&self, block_id: &str) {
        let Ok(mut active) = ACTIVE.lock() else { return };
        self.at_locked(&mut active, block_id);
    }

    fn at_locked(&self, active: &mut [(u64, ActiveBlock)], block_id: &str) {
        if let Some((_, block)) = active.iter_mut().find(|(k, _)| *k == self.key)
            && block.block_id != block_id
        {
            block.block_id.clear();
            block.block_id.push_str(block_id);
            REVISION.fetch_add(1, Ordering::Relaxed);
        }
    }
}

impl Drop for Tracker {
    fn drop(&mut self) {
        let Ok(mut active) = ACTIVE.lock() else { return };
        let Some(pos) = active.iter().position(|(k, _)| *k == self.key) else { return };
        let (_, block) = active.remove(pos);
        REVISION.fetch_add(1, Ordering::Relaxed);
        let last = !active.iter().any(|(_, b)| b.macro_id == block.macro_id);
        drop(active);
        if last {
            crate::macros::debug::run_ended(&block.macro_id);
            crate::permissions::run_ended(&block.macro_id);
        }
    }
}

pub fn revision() -> u64 {
    REVISION.load(Ordering::Relaxed)
}

/// Every block some strand is on, without duplicates.
pub fn snapshot() -> Vec<ActiveBlock> {
    let Ok(active) = ACTIVE.lock() else { return Vec::new() };
    let mut out: Vec<ActiveBlock> = Vec::new();
    for (_, block) in active.iter() {
        if !block.block_id.is_empty() && !out.contains(block) {
            out.push(block.clone());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracks_blocks_until_the_strand_ends() {
        let tracker = Tracker::new("trace-test").unwrap();
        tracker.at("b1");
        {
            // Other runner tests also bump the revision, so hold the trace lock.
            let mut active = ACTIVE.lock().unwrap();
            let before = revision();
            tracker.at_locked(&mut active, "b1");
            assert_eq!(revision(), before, "staying on a block isn't a change");
            tracker.at_locked(&mut active, "b2");
            assert_eq!(
                revision(),
                before.wrapping_add(1),
                "moving to another block is a change"
            );
        }
        let mine = |s: Vec<ActiveBlock>| s.into_iter().filter(|b| b.macro_id == "trace-test").collect::<Vec<_>>();
        assert_eq!(mine(snapshot()), vec![ActiveBlock { macro_id: "trace-test".into(), block_id: "b2".into() }]);
        drop(tracker);
        assert!(mine(snapshot()).is_empty());
        assert!(Tracker::new("").is_none());
    }
}
