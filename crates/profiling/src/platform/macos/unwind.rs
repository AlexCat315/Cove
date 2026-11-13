//! Reads suspended-thread registers from Mach thread state.

use mach2::kern_return::KERN_SUCCESS;
use mach2::port::mach_port_t;
use mach2::thread_act::thread_get_state;

use crate::SamplingError;

pub(crate) use crate::platform::fp::{Registers, StackBounds, canonical_address, unwind};

/// User-space addresses on macOS fit in 47 bits; the mask also drops pointer
/// authentication bits from arm64e return addresses.
pub(crate) const ADDRESS_MASK: u64 = 0x0000_7FFF_FFFF_FFFF;

/// Reads the target's registers while it is suspended.
#[cfg(target_arch = "aarch64")]
pub(crate) fn read_registers(port: mach_port_t) -> Result<Registers, SamplingError> {
    use mach2::structs::arm_thread_state64_t;
    use mach2::thread_status::ARM_THREAD_STATE64;

    let mut state = arm_thread_state64_t::new();
    let mut count = arm_thread_state64_t::count();
    // SAFETY: the caller suspended `port`; `state` matches ARM_THREAD_STATE64
    // and `count` is its size in natural_t units as required by the kernel.
    let result = unsafe {
        thread_get_state(
            port,
            ARM_THREAD_STATE64,
            std::ptr::addr_of_mut!(state).cast::<u32>(),
            &mut count,
        )
    };
    if result != KERN_SUCCESS {
        return Err(SamplingError::TargetUnavailable(result));
    }
    Ok(Registers {
        pc: canonical_address(state.__pc, ADDRESS_MASK),
        fp: state.__fp,
        sp: state.__sp,
    })
}

/// Reads the target's registers while it is suspended.
#[cfg(target_arch = "x86_64")]
pub(crate) fn read_registers(port: mach_port_t) -> Result<Registers, SamplingError> {
    use mach2::structs::x86_thread_state64_t;
    use mach2::thread_status::x86_THREAD_STATE64;

    let mut state = x86_thread_state64_t::new();
    let mut count = x86_thread_state64_t::count();
    // SAFETY: the caller suspended `port`; `state` matches x86_THREAD_STATE64
    // and `count` is its size in natural_t units as required by the kernel.
    let result = unsafe {
        thread_get_state(
            port,
            x86_THREAD_STATE64,
            std::ptr::addr_of_mut!(state).cast::<u32>(),
            &mut count,
        )
    };
    if result != KERN_SUCCESS {
        return Err(SamplingError::TargetUnavailable(result));
    }
    Ok(Registers {
        pc: canonical_address(state.__rip, ADDRESS_MASK),
        fp: state.__rbp,
        sp: state.__rsp,
    })
}
