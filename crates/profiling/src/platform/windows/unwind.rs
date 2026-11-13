//! Reads the suspended thread's registers and locates its stack mapping.

use std::ffi::c_void;

use windows_sys::Win32::Foundation::{GetLastError, HANDLE};
use windows_sys::Win32::System::Diagnostics::Debug::{CONTEXT, GetThreadContext};
use windows_sys::Win32::System::Memory::{MEM_COMMIT, MEMORY_BASIC_INFORMATION, VirtualQuery};

use crate::SamplingError;
use crate::platform::fp::{Registers, StackBounds, canonical_address};

/// x86_64 user addresses are canonical in the lower 47 bits.
#[cfg(target_arch = "x86_64")]
pub(crate) const ADDRESS_MASK: u64 = 0x0000_7FFF_FFFF_FFFF;
/// arm64 Windows keeps 48 bits of user address; the top byte may carry a tag.
#[cfg(target_arch = "aarch64")]
pub(crate) const ADDRESS_MASK: u64 = 0x0000_FFFF_FFFF_FFFF;

/// `CONTEXT` must be 16-byte aligned for the register sets it can carry.
#[repr(C, align(16))]
struct AlignedContext {
    value: CONTEXT,
}

impl AlignedContext {
    fn new(flags: u32) -> Self {
        // SAFETY: CONTEXT is plain old data; zeroed is a valid starting state
        // and ContextFlags is set before the kernel reads the buffer.
        let mut value = unsafe { std::mem::zeroed::<CONTEXT>() };
        value.ContextFlags = flags;
        Self { value }
    }
}

fn last_error() -> i32 {
    // SAFETY: GetLastError only reads the calling thread's last-error code.
    unsafe { GetLastError() as i32 }
}

/// Reads the target's registers while it is suspended.
#[cfg(target_arch = "x86_64")]
pub(crate) fn read_registers(thread: HANDLE) -> Result<Registers, SamplingError> {
    use windows_sys::Win32::System::Diagnostics::Debug::{
        CONTEXT_CONTROL_AMD64, CONTEXT_INTEGER_AMD64,
    };

    let mut context = AlignedContext::new(CONTEXT_CONTROL_AMD64 | CONTEXT_INTEGER_AMD64);
    // SAFETY: the caller suspended `thread`; the buffer is 16-byte aligned and
    // ContextFlags names exactly the control and integer register sets.
    if unsafe { GetThreadContext(thread, &mut context.value) } == 0 {
        return Err(SamplingError::TargetUnavailable(last_error()));
    }
    Ok(Registers {
        pc: canonical_address(context.value.Rip, ADDRESS_MASK),
        fp: context.value.Rbp,
        sp: context.value.Rsp,
    })
}

/// Reads the target's registers while it is suspended.
#[cfg(target_arch = "aarch64")]
pub(crate) fn read_registers(thread: HANDLE) -> Result<Registers, SamplingError> {
    use windows_sys::Win32::System::Diagnostics::Debug::{
        CONTEXT_CONTROL_ARM64, CONTEXT_INTEGER_ARM64,
    };

    let mut context = AlignedContext::new(CONTEXT_CONTROL_ARM64 | CONTEXT_INTEGER_ARM64);
    // SAFETY: the caller suspended `thread`; the buffer is 16-byte aligned and
    // ContextFlags names exactly the control and integer register sets.
    if unsafe { GetThreadContext(thread, &mut context.value) } == 0 {
        return Err(SamplingError::TargetUnavailable(last_error()));
    }
    // SAFETY: the integer set was requested, so the anonymous struct holds the
    // general-purpose registers; `Fp` is the active union member.
    let fp = unsafe { context.value.Anonymous.Anonymous.Fp };
    Ok(Registers {
        pc: canonical_address(context.value.Pc, ADDRESS_MASK),
        fp,
        sp: context.value.Sp,
    })
}

/// The committed region holding `sp`, which is the sampled thread's stack.
///
/// The target is suspended in user mode, so it holds no kernel lock that this
/// query could wait on.
pub(crate) fn stack_bounds(sp: u64) -> Option<StackBounds> {
    if sp == 0 {
        return None;
    }
    let mut info = std::mem::MaybeUninit::<MEMORY_BASIC_INFORMATION>::zeroed();
    // SAFETY: VirtualQuery writes at most one MEMORY_BASIC_INFORMATION into
    // the zeroed storage and never dereferences `sp` itself.
    let written = unsafe {
        VirtualQuery(
            sp as *const c_void,
            info.as_mut_ptr(),
            std::mem::size_of::<MEMORY_BASIC_INFORMATION>(),
        )
    };
    if written == 0 {
        return None;
    }
    // SAFETY: a non-zero return means the whole struct was filled.
    let info = unsafe { info.assume_init() };
    if info.State != MEM_COMMIT {
        return None;
    }
    let base = info.BaseAddress as u64;
    let top = base.checked_add(info.RegionSize as u64)?;
    StackBounds::new(top, top.checked_sub(base)?)
}
