//! Translation of NativeRHI/Vulkan/Submission/VulkanSubmissionStatus.swift.
//!
//! GPU errors are recorded on the fence waiter and surfaced on the API thread at
//! `wait_until_idle`, so the original cause survives a cascade.
use std::sync::Mutex;

use crate::rhi::{RhiError, RhiResult};

#[derive(Debug, Default)]
pub struct VulkanSubmissionStatus {
    failure: Mutex<Option<RhiError>>,
}

impl VulkanSubmissionStatus {
    pub fn new() -> Self {
        Self::default()
    }

    /// Records the first GPU failure. Later failures never overwrite it, so the
    /// API thread reports the original cause.
    pub fn record(&self, description: impl Into<String>) {
        let mut failure = self.failure.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        if failure.is_none() {
            *failure = Some(RhiError::submit_failed(description));
        }
    }

    /// Returns the recorded failure once, on the API thread.
    pub fn check(&self) -> RhiResult<()> {
        let failure = self.failure.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        match failure.as_ref() {
            Some(error) => Err(error.clone()),
            None => Ok(()),
        }
    }
}
