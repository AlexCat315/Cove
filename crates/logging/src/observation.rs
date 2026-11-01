//! Translation of `Logging/Sources/CoveLogging/Runtime/Observation.swift`.
use std::sync::{Arc, Mutex};

type Release = Box<dyn FnOnce() + Send + 'static>;

/// Cancellation token returned by `LogRuntime::observe`. Dropping or cancelling
/// releases the subscriber exactly once.
pub struct LogObservation {
    release: Arc<Mutex<Option<Release>>>,
}
impl LogObservation {
    pub fn new(release: impl FnOnce() + Send + 'static) -> Self {
        Self {
            release: Arc::new(Mutex::new(Some(Box::new(release)))),
        }
    }

    pub fn cancel(&self) {
        let work = self
            .release
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take();
        if let Some(work) = work {
            work();
        }
    }
}
impl Drop for LogObservation {
    fn drop(&mut self) {
        self.cancel();
    }
}
