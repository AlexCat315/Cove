//! `proc_pidinfo(PROC_PIDTASKINFO)` for the current process.

use super::ProcessMemory;

pub(super) const BACKEND: &str = "proc_pidinfo";

pub(super) fn read() -> ProcessMemory {
    let mut info = std::mem::MaybeUninit::<libc::proc_taskinfo>::uninit();
    let size = std::mem::size_of::<libc::proc_taskinfo>();
    // SAFETY: `info` has room for exactly `size` bytes of `proc_taskinfo`, and
    // `getpid()` names this process, so libproc fills or leaves it untouched.
    let written = unsafe {
        libc::proc_pidinfo(
            libc::getpid(),
            libc::PROC_PIDTASKINFO,
            0,
            info.as_mut_ptr().cast(),
            size as libc::c_int,
        )
    };
    if written != size as libc::c_int {
        return ProcessMemory::default();
    }
    // SAFETY: `proc_pidinfo` returned the full struct size, so every field is
    // initialized.
    let info = unsafe { info.assume_init() };
    ProcessMemory {
        resident_bytes: Some(info.pti_resident_size),
        virtual_bytes: Some(info.pti_virtual_size),
    }
}

pub(super) fn unavailable_reason() -> Option<&'static str> {
    None
}
