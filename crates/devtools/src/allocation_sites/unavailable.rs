//! The browser build cannot capture allocation stacks.
//!
//! Returning sites here would mean inventing stacks, so every command reports
//! the same explicit gap instead.

use cove_devtools_protocol::control::Fault;
use serde_json::Value;

const REASON: &str = "The browser runtime cannot capture allocation stacks; allocation sites are available from native Apple and GNU Linux hosts.";

/// Always `Some`: the browser cannot sample its allocator.
pub(crate) fn unavailable_reason() -> Option<String> {
    Some(REASON.to_owned())
}

/// A sampler that cannot sample; every command reports [`REASON`].
pub(crate) struct AllocationSites;

impl AllocationSites {
    pub(crate) fn new() -> Self {
        Self
    }

    pub(crate) fn start(&mut self, _rate: u32) -> Result<Value, Fault> {
        Err(Fault::new("unavailable", REASON))
    }

    pub(crate) fn stop(&mut self) -> Result<Value, Fault> {
        Err(Fault::new("unavailable", REASON))
    }

    pub(crate) fn sites(&mut self, _limit: usize) -> Result<Value, Fault> {
        Err(Fault::new("unavailable", REASON))
    }

    pub(crate) fn reset(&mut self) {}
}
