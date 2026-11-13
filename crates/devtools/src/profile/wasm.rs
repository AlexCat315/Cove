//! The browser runtime's UI profiler.
//!
//! WebAssembly runs on the page's single thread, so nothing can suspend it and
//! read a stack: that is why the browser used to answer `unavailable`. It can
//! still measure where a frame's CPU time goes, so this profiler accumulates the
//! phases the render path times and answers with
//! [`cove_devtools_protocol::ProfileSource::Instrumented`]. Every millisecond is
//! a measured duration; no sample is synthesized.

use cove_devtools_protocol::control::Fault;
use serde_json::Value;

use super::cooperative;

/// A cooperative profiler over the browser runtime's frame phases. It always
/// works, so the runtime advertises `cpu_profile` and there is no gap to report.
pub(crate) fn unavailable_reason() -> Option<&'static str> {
    None
}

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

    /// `hz` is accepted for protocol compatibility; phase timing has no rate,
    /// so the answer reports `hz: 0` instead of implying a sampling frequency.
    pub(crate) fn start(&mut self, _hz: u32, duration_ms: u32) -> Result<Value, Fault> {
        cooperative::start(duration_ms)?;
        Ok(profile_value(&cooperative::read(&self.thread)))
    }

    pub(crate) fn stop(&mut self) -> Result<Value, Fault> {
        cooperative::stop()?;
        Ok(profile_value(&cooperative::read(&self.thread)))
    }

    pub(crate) fn read(&self) -> Result<Value, Fault> {
        Ok(profile_value(&cooperative::read(&self.thread)))
    }

    pub(crate) fn hotspots(&self, limit: usize) -> Result<Value, Fault> {
        Ok(profile_value(&cooperative::hotspots(&self.thread, limit)))
    }

    pub(crate) fn reset(&mut self) {
        cooperative::reset();
    }
}

fn profile_value<T: serde::Serialize>(payload: &T) -> Value {
    serde_json::to_value(payload).unwrap_or_else(|error| serde_json::json!({ "error": error.to_string() }))
}
