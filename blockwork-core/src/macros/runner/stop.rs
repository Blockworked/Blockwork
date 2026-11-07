use crate::macros::StopScope;
use std::sync::{Arc, Mutex, Weak};

/// Run cancellation plus a separate flag for each executing strand.
#[derive(Clone)]
pub(crate) struct StrandStop {
    run: Arc<Mutex<bool>>,
    local: Arc<Mutex<bool>>,
    siblings: Arc<Mutex<Vec<Weak<Mutex<bool>>>>>,
}

impl StrandStop {
    pub(super) fn new(run: Option<Arc<Mutex<bool>>>) -> Self {
        Self {
            run: run.unwrap_or_else(|| Arc::new(Mutex::new(true))),
            local: Arc::new(Mutex::new(true)),
            siblings: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub(super) fn fork(&self) -> Self {
        let mut siblings = self.siblings.lock().unwrap();
        let local = Arc::new(Mutex::new(!stop_requested(self)));
        siblings.retain(|flag| flag.strong_count() > 0);
        siblings.push(Arc::downgrade(&local));
        Self {
            run: Arc::clone(&self.run),
            local,
            siblings: Arc::clone(&self.siblings),
        }
    }

    pub(super) fn stop(&self, scope: StopScope) {
        match scope {
            StopScope::All => *self.run.lock().unwrap() = false,
            StopScope::ThisStrand => *self.local.lock().unwrap() = false,
            StopScope::OtherStrands => {
                let mut siblings = self.siblings.lock().unwrap();
                siblings.retain(|flag| {
                    let Some(flag) = flag.upgrade() else {
                        return false;
                    };
                    if !Arc::ptr_eq(&flag, &self.local) {
                        *flag.lock().unwrap() = false;
                    }
                    true
                });
            }
        }
    }
}

pub(super) fn stop_requested(flag: &StrandStop) -> bool {
    !flag.run.lock().map(|g| *g).unwrap_or(false) || !flag.local.lock().map(|g| *g).unwrap_or(false)
}
