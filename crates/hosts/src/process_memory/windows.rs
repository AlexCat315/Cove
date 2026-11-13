//! `K32GetProcessMemoryInfo` with `PROCESS_MEMORY_COUNTERS`.
//!
//! `WorkingSetSize` is the resident byte count. The struct exposes no address
//! space size, so the virtual counter carries `PagefileUsage`, the process'
//! committed virtual memory.

use super::ProcessMemory;
use windows_sys::Win32::System::ProcessStatus::{K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
use windows_sys::Win32::System::Threading::GetCurrentProcess;

pub(super) const BACKEND: &str = "k32_get_process_memory_info";

pub(super) fn read() -> ProcessMemory {
    let mut counters = PROCESS_MEMORY_COUNTERS {
        cb: std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
        ..PROCESS_MEMORY_COUNTERS::default()
    };
    let size = counters.cb;
    // SAFETY: `counters` is a live, correctly sized `PROCESS_MEMORY_COUNTERS`
    // and the pseudo handle names this process.
    let ok = unsafe { K32GetProcessMemoryInfo(GetCurrentProcess(), &mut counters, size) };
    if ok == 0 {
        return ProcessMemory::default();
    }
    ProcessMemory {
        resident_bytes: Some(counters.WorkingSetSize as u64).filter(|bytes| *bytes > 0),
        virtual_bytes: Some(counters.PagefileUsage as u64).filter(|bytes| *bytes > 0),
    }
}

pub(super) fn unavailable_reason() -> Option<&'static str> {
    None
}
