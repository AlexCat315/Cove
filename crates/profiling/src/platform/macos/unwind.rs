//! Reads suspended-thread registers and walks the frame-pointer chain.

use mach2::kern_return::KERN_SUCCESS;
use mach2::port::mach_port_t;
use mach2::thread_act::thread_get_state;

use crate::SamplingError;
use crate::config::MAX_STACK_DEPTH;

/// User-space addresses on macOS fit in 47 bits; the mask also drops pointer
/// authentication bits from arm64e return addresses.
const ADDRESS_MASK: u64 = 0x0000_7FFF_FFFF_FFFF;

/// Instruction, frame and stack pointers of the suspended thread.
#[derive(Clone, Copy, Debug)]
pub(super) struct Registers {
    pub(super) pc: u64,
    pub(super) fp: u64,
    pub(super) sp: u64,
}

/// Mapped stack range of the sampled thread.
#[derive(Clone, Copy, Debug)]
pub(super) struct StackBounds {
    low: u64,
    top: u64,
}

impl StackBounds {
    /// Accepts only a plausible stack large enough to hold a frame record.
    pub(super) fn new(top: u64, size: u64) -> Option<Self> {
        let low = top.checked_sub(size)?;
        (size >= 64 && low > 0).then_some(Self { low, top })
    }
}

/// Drops pointer-authentication tags so an address can be looked up.
pub(super) fn canonical_address(address: u64) -> u64 {
    address & ADDRESS_MASK
}

/// Reads the target's registers while it is suspended.
#[cfg(target_arch = "aarch64")]
pub(super) fn read_registers(port: mach_port_t) -> Result<Registers, SamplingError> {
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
        pc: canonical_address(state.__pc),
        fp: state.__fp,
        sp: state.__sp,
    })
}

/// Reads the target's registers while it is suspended.
#[cfg(target_arch = "x86_64")]
pub(super) fn read_registers(port: mach_port_t) -> Result<Registers, SamplingError> {
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
        pc: canonical_address(state.__rip),
        fp: state.__rbp,
        sp: state.__rsp,
    })
}

/// Records `pc` followed by the frame-pointer chain, newest first.
pub(super) fn unwind(
    registers: Registers,
    stack: Option<StackBounds>,
    max_stack_depth: usize,
    frames: &mut [u64; MAX_STACK_DEPTH],
) -> usize {
    let max_depth = max_stack_depth.min(MAX_STACK_DEPTH);
    if max_depth == 0 {
        return 0;
    }
    let mut depth = 0;
    if registers.pc != 0 {
        frames[0] = registers.pc;
        depth = 1;
    }
    // Without a known stack mapping, walking frame pointers would mean reading
    // possibly unmapped memory; report the sampled PC only.
    let Some(bounds) = stack else {
        return depth;
    };
    let (low, top) = (bounds.low, bounds.top);
    let mut fp = registers.fp;
    while depth < max_depth {
        if !fp.is_multiple_of(8) || fp < registers.sp || fp < low || top.saturating_sub(fp) < 16 {
            break;
        }
        let Some((saved_fp, return_address)) = read_frame(fp) else {
            break;
        };
        let caller = canonical_address(return_address);
        if caller != 0 {
            frames[depth] = caller;
            depth += 1;
            if depth >= max_depth {
                break;
            }
        }
        if saved_fp <= fp {
            break;
        }
        fp = saved_fp;
    }
    depth
}

/// Reads `{saved fp, return address}` from one frame record.
fn read_frame(fp: u64) -> Option<(u64, u64)> {
    if fp < 4096 || !fp.is_multiple_of(8) {
        return None;
    }
    let record = fp as *const u64;
    // SAFETY: `unwind` checked that `fp` is 8-byte aligned and that at least
    // 16 bytes remain inside the target thread's stack mapping. The target is
    // suspended, and the sampler never runs on the same thread.
    unsafe {
        let saved_fp = record.read_volatile();
        let return_address = record.add(1).read_volatile();
        Some((saved_fp, return_address))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_address_drops_authentication_bits() {
        assert_eq!(canonical_address(0x1234_5678_9abc_def0), 0x5678_9abc_def0);
        assert_eq!(canonical_address(0x1000), 0x1000);
    }

    #[test]
    fn rejects_implausible_stack_bounds() {
        assert!(StackBounds::new(4096, 0).is_none());
        assert!(StackBounds::new(0, 4096).is_none());
        assert!(StackBounds::new(1 << 20, 64).is_some());
    }

    #[test]
    fn without_stack_bounds_only_the_sampled_pc_is_kept() {
        let mut frames = [0; MAX_STACK_DEPTH];
        let registers = Registers {
            pc: 0x1234,
            fp: 0x8000,
            sp: 0x7000,
        };
        let depth = unwind(registers, None, 8, &mut frames);
        assert_eq!(depth, 1);
        assert_eq!(frames[0], 0x1234);
    }
}
