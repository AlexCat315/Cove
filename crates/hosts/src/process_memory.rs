//! Resident and virtual byte readings for the current process.
//!
//! The host reports these through `query_ui_memory` as the
//! [`RESIDENT_COUNTER`] and [`VIRTUAL_COUNTER`] counters. Every backend reads a
//! real platform counter; a target without a reader omits the counters and
//! reports why through [`unavailable_reason`], so a missing reading is never
//! published as zero.
//!
//! The meaning of the virtual counter follows the platform's own definition:
//!
//! | Target | Resident | Virtual | Backend |
//! |---|---|---|---|
//! | macOS | `pti_resident_size` | `pti_virtual_size` (address space) | `proc_pidinfo(PROC_PIDTASKINFO)` |
//! | iOS | `resident_size` | `virtual_size` (address space) | `task_info(MACH_TASK_BASIC_INFO)` |
//! | Linux, Android | `VmRSS` | `VmSize` (address space) | `/proc/self/status` |
//! | Windows | `WorkingSetSize` | `PagefileUsage` (private commit) | `K32GetProcessMemoryInfo` |
//! | Wasm, others | — | — | no reader; counters omitted |
//!
//! Windows has no address-space size in `PROCESS_MEMORY_COUNTERS`, so its
//! virtual counter is the process's committed virtual memory (the value the
//! Task Manager shows as "Commit size"), not reserved address space.

use std::collections::BTreeMap;

/// Counter carrying the resident byte count in `query_ui_memory`.
pub const RESIDENT_COUNTER: &str = "processResidentBytes";

/// Counter carrying the virtual byte count in `query_ui_memory`.
pub const VIRTUAL_COUNTER: &str = "processVirtualBytes";

#[cfg(target_os = "macos")]
#[path = "process_memory/macos.rs"]
mod platform;

#[cfg(target_os = "ios")]
#[path = "process_memory/ios.rs"]
mod platform;

#[cfg(any(target_os = "linux", target_os = "android"))]
#[path = "process_memory/procfs.rs"]
mod platform;

#[cfg(target_os = "windows")]
#[path = "process_memory/windows.rs"]
mod platform;

#[cfg(not(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "linux",
    target_os = "android",
    target_os = "windows"
)))]
#[path = "process_memory/unsupported.rs"]
mod platform;

/// Process memory the platform could read. A field is `None` when the target
/// has no reading for it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProcessMemory {
    /// Bytes currently resident in physical memory.
    pub resident_bytes: Option<u64>,
    /// Virtual bytes under the platform's definition (see the module docs).
    pub virtual_bytes: Option<u64>,
}

impl ProcessMemory {
    /// Reads the current process' memory once.
    pub fn read() -> Self {
        platform::read()
    }

    /// Whether any counter can be reported.
    pub fn is_available(self) -> bool {
        self.resident_bytes.is_some() || self.virtual_bytes.is_some()
    }

    /// Adds the counters that were read. Absent readings are omitted rather
    /// than written as zero.
    pub fn insert_counters(self, counters: &mut BTreeMap<String, u64>) {
        if let Some(bytes) = self.resident_bytes.filter(|bytes| *bytes > 0) {
            counters.insert(RESIDENT_COUNTER.to_owned(), bytes);
        }
        if let Some(bytes) = self.virtual_bytes.filter(|bytes| *bytes > 0) {
            counters.insert(VIRTUAL_COUNTER.to_owned(), bytes);
        }
    }
}

/// Adds the process memory counters this target can read to `counters`.
pub fn insert_counters(counters: &mut BTreeMap<String, u64>) {
    ProcessMemory::read().insert_counters(counters);
}

/// The platform backend compiled into this build.
pub fn backend() -> &'static str {
    platform::BACKEND
}

/// Why no process memory counter is reported, or `None` when this target has a
/// reader.
pub fn unavailable_reason() -> Option<&'static str> {
    platform::unavailable_reason()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backend_matches_the_target() {
        let expected = if cfg!(target_os = "macos") {
            "proc_pidinfo"
        } else if cfg!(target_os = "ios") {
            "mach_task_info"
        } else if cfg!(any(target_os = "linux", target_os = "android")) {
            "procfs_status"
        } else if cfg!(target_os = "windows") {
            "k32_get_process_memory_info"
        } else {
            "none"
        };
        assert_eq!(backend(), expected);
    }

    /// Every target with a reader must produce a real positive reading for the
    /// current process; a target without one must say so.
    #[test]
    fn reads_the_current_process() {
        let memory = ProcessMemory::read();
        if cfg!(any(
            target_os = "macos",
            target_os = "ios",
            target_os = "linux",
            target_os = "android",
            target_os = "windows"
        )) {
            let resident = memory
                .resident_bytes
                .expect("the platform reader must report resident bytes");
            assert!(resident > 0, "resident bytes must be a real reading");
            let virtual_bytes = memory
                .virtual_bytes
                .expect("the platform reader must report virtual bytes");
            assert!(virtual_bytes > 0, "virtual bytes must be a real reading");
            assert_eq!(unavailable_reason(), None);

            let mut counters = BTreeMap::new();
            memory.insert_counters(&mut counters);
            assert_eq!(counters.get(RESIDENT_COUNTER), Some(&resident));
            assert_eq!(counters.get(VIRTUAL_COUNTER), Some(&virtual_bytes));
        } else {
            assert_eq!(memory, ProcessMemory::default());
            assert!(unavailable_reason().is_some());

            let mut counters = BTreeMap::new();
            memory.insert_counters(&mut counters);
            assert!(counters.is_empty(), "no counter may be fabricated");
        }
    }

    /// Resident cannot exceed the address space where the virtual counter is an
    /// address-space size. Windows reports commit charge instead, which is a
    /// different measure and has no such ordering.
    #[test]
    fn resident_does_not_exceed_virtual_address_space() {
        if !cfg!(any(
            target_os = "macos",
            target_os = "ios",
            target_os = "linux",
            target_os = "android"
        )) {
            return;
        }
        let memory = ProcessMemory::read();
        if let (Some(resident), Some(virtual_bytes)) = (memory.resident_bytes, memory.virtual_bytes)
        {
            assert!(
                resident <= virtual_bytes,
                "resident {resident} exceeds the {virtual_bytes}-byte address space"
            );
        }
    }
}
