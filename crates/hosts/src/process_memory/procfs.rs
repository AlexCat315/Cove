//! `VmRSS` and `VmSize` from `/proc/self/status` on Linux and Android.

use super::ProcessMemory;

pub(super) const BACKEND: &str = "procfs_status";

pub(super) fn read() -> ProcessMemory {
    let Ok(status) = std::fs::read_to_string("/proc/self/status") else {
        return ProcessMemory::default();
    };
    let mut resident_bytes = None;
    let mut virtual_bytes = None;
    for line in status.lines() {
        if let Some(value) = line.strip_prefix("VmRSS:") {
            resident_bytes = kilobytes(value);
        } else if let Some(value) = line.strip_prefix("VmSize:") {
            virtual_bytes = kilobytes(value);
        }
    }
    ProcessMemory {
        resident_bytes,
        virtual_bytes,
    }
}

/// Parses the `kB` suffix `/proc/self/status` uses. A zero reading is treated
/// as absent so the counter is omitted instead of published as a real zero.
fn kilobytes(field: &str) -> Option<u64> {
    let value = field.trim().strip_suffix("kB")?.trim();
    value
        .parse::<u64>()
        .ok()
        .and_then(|kilobytes| kilobytes.checked_mul(1024))
        .filter(|bytes| *bytes > 0)
}

pub(super) fn unavailable_reason() -> Option<&'static str> {
    None
}
