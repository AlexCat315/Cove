//! Reads the interrupted thread's registers from the kernel's signal context.

use crate::platform::fp::{Registers, canonical_address};

/// x86_64 user addresses are canonical in the lower 47 bits.
#[cfg(target_arch = "x86_64")]
pub(crate) const ADDRESS_MASK: u64 = 0x0000_7FFF_FFFF_FFFF;
/// arm64 Linux keeps 48 bits of user address; the top byte may carry a tag.
#[cfg(target_arch = "aarch64")]
pub(crate) const ADDRESS_MASK: u64 = 0x0000_FFFF_FFFF_FFFF;

/// `sigcontext` register indices, fixed by the kernel ABI rather than by libc.
#[cfg(target_arch = "x86_64")]
mod index {
    pub(super) const RIP: usize = 16;
    pub(super) const RBP: usize = 10;
    pub(super) const RSP: usize = 15;
}

/// Reads the registers the kernel saved when it delivered SIGPROF.
#[cfg(target_arch = "x86_64")]
pub(crate) fn read_registers(context: *const libc::ucontext_t) -> Option<Registers> {
    if context.is_null() {
        return None;
    }
    // SAFETY: the kernel placed a valid `ucontext_t` on the interrupted
    // thread's stack and passed its address as the handler's third argument.
    let mcontext = unsafe { &(*context).uc_mcontext };
    Some(Registers {
        pc: canonical_address(mcontext.gregs[index::RIP] as u64, ADDRESS_MASK),
        fp: mcontext.gregs[index::RBP] as u64,
        sp: mcontext.gregs[index::RSP] as u64,
    })
}

/// Reads the registers the kernel saved when it delivered SIGPROF.
#[cfg(target_arch = "aarch64")]
pub(crate) fn read_registers(context: *const libc::ucontext_t) -> Option<Registers> {
    if context.is_null() {
        return None;
    }
    // SAFETY: the kernel placed a valid `ucontext_t` on the interrupted
    // thread's stack and passed its address as the handler's third argument.
    let mcontext = unsafe { &(*context).uc_mcontext };
    Some(Registers {
        pc: canonical_address(mcontext.pc as u64, ADDRESS_MASK),
        fp: mcontext.regs[29] as u64,
        sp: mcontext.sp as u64,
    })
}
