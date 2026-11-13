//! No process memory reader on this target.

use super::ProcessMemory;

pub(super) const BACKEND: &str = "none";

pub(super) fn read() -> ProcessMemory {
    ProcessMemory::default()
}

pub(super) fn unavailable_reason() -> Option<&'static str> {
    #[cfg(target_arch = "wasm32")]
    {
        Some("Wasm has no process address space to sample; the process memory counters are omitted")
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        Some(
            "this target has no in-process resident/virtual memory reader; the process memory counters are omitted",
        )
    }
}
