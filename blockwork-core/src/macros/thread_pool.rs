use std::thread::JoinHandle;

pub struct ThreadPool {
    pub workers: Vec<JoinHandle<()>>,
}

impl Default for ThreadPool {
    fn default() -> Self {
        Self::new()
    }
}

impl ThreadPool {
    pub fn new() -> Self {
        ThreadPool {
            workers: Vec::new(),
        }
    }

    pub fn add_worker(&mut self, worker: JoinHandle<()>) {
        self.workers.push(worker);
    }

    pub fn cleanup_completed_threads(&mut self) {
        self.workers.retain(|worker| !worker.is_finished());
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        for worker in self.workers.drain(..) {
            worker.join().expect("Failed to join worker thread");
        }
    }
}
