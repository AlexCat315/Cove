//! The browser build has no in-process CPU sampler.
//!
//! Returning a profile here would mean inventing samples, so every profiling
//! command reports the same explicit gap instead.

use cove_devtools_protocol::control::Fault;
use serde_json::Value;

const REASON: &str = "The browser runtime has no in-process CPU sampler; UI CPU profiles are available from native macOS hosts.";

/// Always `Some`: the browser cannot sample itself.
pub(crate) fn unavailable_reason() -> Option<&'static str> {
    Some(REASON)
}

/// A profiler that cannot sample; every command reports [`REASON`].
pub(crate) struct UiProfiler {
    thread: String,
}

impl UiProfiler {
    pub(crate) fn new(thread: String) -> Self {
        Self {
            thread: if thread.is_empty() {
                "main".to_owned()
            } else {
                thread
            },
        }
    }

    pub(crate) fn thread_name(&self) -> &str {
        &self.thread
    }

    pub(crate) fn start(&mut self, _hz: u32, _duration_ms: u32) -> Result<Value, Fault> {
        Err(Fault::new("unavailable", REASON))
    }

    pub(crate) fn stop(&mut self) -> Result<Value, Fault> {
        Err(Fault::new("unavailable", REASON))
    }

    pub(crate) fn read(&self) -> Result<Value, Fault> {
        Err(Fault::new("unavailable", REASON))
    }

    pub(crate) fn hotspots(&self, _limit: usize) -> Result<Value, Fault> {
        Err(Fault::new("unavailable", REASON))
    }

    pub(crate) fn reset(&mut self) {}
}
