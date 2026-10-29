//! Translation of NativeRHI/Device/Queue.swift: queue submission descriptors
//! and timeline semaphores.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TimelineSemaphore {
    pub id: u32,
    pub value: u64,
}
impl TimelineSemaphore {
    pub const fn new(id: u32, value: u64) -> Self {
        Self { id, value }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SubmitDescriptor {
    pub wait_semaphores: Vec<TimelineSemaphore>,
    pub signal_semaphores: Vec<TimelineSemaphore>,
}
impl SubmitDescriptor {
    pub fn new() -> Self {
        Self::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_submit_descriptor_waits_on_nothing() {
        let descriptor = SubmitDescriptor::new();
        assert!(descriptor.wait_semaphores.is_empty());
        assert!(descriptor.signal_semaphores.is_empty());
        assert_eq!(TimelineSemaphore::new(1, 7).value, 7);
    }
}
