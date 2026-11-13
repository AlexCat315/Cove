//! `task_info(MACH_TASK_BASIC_INFO)` for the current task.

use super::ProcessMemory;

pub(super) const BACKEND: &str = "mach_task_info";

pub(super) fn read() -> ProcessMemory {
    let mut info = std::mem::MaybeUninit::<libc::mach_task_basic_info>::uninit();
    let mut count = libc::MACH_TASK_BASIC_INFO_COUNT;
    // `mach_task_self` is the deprecated wrapper around the task port libc
    // already exports; iOS offers no `proc_pidinfo`.
    #[allow(deprecated)]
    let task = unsafe { libc::mach_task_self() };
    // SAFETY: `info` is sized and aligned for `mach_task_basic_info`, `count`
    // states that capacity, and `task` is this process' task port.
    let result = unsafe {
        libc::task_info(
            task,
            libc::MACH_TASK_BASIC_INFO,
            info.as_mut_ptr().cast(),
            &mut count,
        )
    };
    if result != libc::KERN_SUCCESS {
        return ProcessMemory::default();
    }
    // SAFETY: `task_info` succeeded, so the kernel wrote the basic info struct.
    let info = unsafe { info.assume_init() };
    ProcessMemory {
        resident_bytes: Some(info.resident_size),
        virtual_bytes: Some(info.virtual_size),
    }
}

pub(super) fn unavailable_reason() -> Option<&'static str> {
    None
}
