//! Frame-pointer chain walking shared by every sampler backend.
//!
//! A backend supplies the sampled thread's registers and, when it knows them,
//! the bounds of that thread's stack mapping. The walk never reads outside
//! those bounds, so a bad frame pointer ends the stack instead of faulting.

use crate::config::MAX_STACK_DEPTH;

/// Instruction, frame and stack pointers of the sampled thread.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Registers {
    pub(crate) pc: u64,
    pub(crate) fp: u64,
    pub(crate) sp: u64,
}

/// Mapped stack range of the sampled thread.
#[derive(Clone, Copy, Debug)]
pub(crate) struct StackBounds {
    low: u64,
    top: u64,
}

impl StackBounds {
    /// Accepts only a plausible stack large enough to hold a frame record.
    pub(crate) fn new(top: u64, size: u64) -> Option<Self> {
        let low = top.checked_sub(size)?;
        (size >= 64 && low > 0).then_some(Self { low, top })
    }

    /// Lowest address the walk may read.
    pub(crate) fn low(self) -> u64 {
        self.low
    }

    /// One past the highest address the walk may read.
    pub(crate) fn top(self) -> u64 {
        self.top
    }
}

/// Drops tag bits so an address can be looked up in a loaded image.
pub(crate) fn canonical_address(address: u64, mask: u64) -> u64 {
    address & mask
}

/// Records `pc` followed by the frame-pointer chain, newest first.
///
/// `address_mask` removes the platform's pointer tags: arm64e authentication
/// bits on macOS, the top-byte tag on Linux and Windows arm64.
pub(crate) fn unwind(
    registers: Registers,
    stack: Option<StackBounds>,
    max_stack_depth: usize,
    address_mask: u64,
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
    let (low, top) = (bounds.low(), bounds.top());
    let mut fp = registers.fp;
    while depth < max_depth {
        if !fp.is_multiple_of(8) || fp < registers.sp || fp < low || top.saturating_sub(fp) < 16 {
            break;
        }
        let Some((saved_fp, return_address)) = read_frame(fp) else {
            break;
        };
        let caller = canonical_address(return_address, address_mask);
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
    // 16 bytes remain inside the sampled thread's stack mapping. The thread is
    // either suspended by the sampler or interrupted inside its own signal
    // handler, and the sampler never runs on that thread.
    unsafe {
        let saved_fp = record.read_volatile();
        let return_address = record.add(1).read_volatile();
        Some((saved_fp, return_address))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MASK: u64 = 0x0000_7FFF_FFFF_FFFF;

    #[test]
    fn canonical_address_drops_authentication_bits() {
        assert_eq!(canonical_address(0x1234_5678_9abc_def0, MASK), 0x5678_9abc_def0);
        assert_eq!(canonical_address(0x1000, MASK), 0x1000);
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
        let depth = unwind(registers, None, 8, MASK, &mut frames);
        assert_eq!(depth, 1);
        assert_eq!(frames[0], 0x1234);
    }

    #[test]
    fn a_zero_depth_records_nothing() {
        let mut frames = [0; MAX_STACK_DEPTH];
        let registers = Registers {
            pc: 0x1234,
            fp: 0x8000,
            sp: 0x7000,
        };
        assert_eq!(unwind(registers, None, 0, MASK, &mut frames), 0);
    }

    #[test]
    fn a_frame_pointer_below_the_stack_pointer_ends_the_walk() {
        let mut frames = [0; MAX_STACK_DEPTH];
        let registers = Registers {
            pc: 0x1234,
            fp: 0x6000,
            sp: 0x7000,
        };
        let bounds = StackBounds::new(0x10000, 0x9000);
        assert_eq!(unwind(registers, bounds, 8, MASK, &mut frames), 1);
    }
}
