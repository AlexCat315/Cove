//! Stack capture for allocation-site samples.
//!
//! Four backends cover the platforms Cove ships on:
//!
//! - Apple and GNU Linux use `libc::backtrace`.
//! - Windows uses `RtlCaptureStackBackTrace`, which walks the current thread
//!   without suspending it.
//! - Android and musl Linux export no `backtrace`, so they drive the Itanium
//!   unwinder's `_Unwind_Backtrace`/`_Unwind_GetIP` pair directly.
//!
//! Every other target reports [`SamplingSupport::Unsupported`] with a reason and
//! never records a site — no synthetic stacks are produced.
//!
//! All four read the current thread's stack without taking the allocator's lock.
//! The unwinder may lazily initialize on its first call, which can allocate;
//! [`prewarm`] runs that first call from `start_sampling`, outside the allocator
//! hook, and the hook's thread-local recursion guard covers any later internal
//! allocation.
//!
//! Each backend skips its own `capture_into` frame, so the first retained address
//! is always the caller of `capture_into` — the sampling hook — on every platform.

use super::{MAX_SITE_FRAMES, SamplingSupport};

#[cfg(any(target_vendor = "apple", all(target_os = "linux", target_env = "gnu")))]
mod backtrace {
    use super::MAX_SITE_FRAMES;
    use super::SamplingSupport;
    use std::ffi::c_void;

    /// Frames that belong to the capture machinery rather than the caller.
    ///
    /// On Apple platforms `backtrace` returns the address that called it, so
    /// the first entry is this function. glibc additionally reports its own `backtrace`
    /// frame first, so both it and this function are skipped.
    #[cfg(target_vendor = "apple")]
    const SKIP: usize = 1;
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    const SKIP: usize = 2;

    pub(crate) fn support() -> SamplingSupport {
        SamplingSupport::Supported
    }

    /// Captures up to `MAX_SITE_FRAMES` caller addresses, innermost first.
    ///
    /// Returns `None` when the platform's unwinder produced no usable caller
    /// frame; the caller counts that as a capture failure instead of
    /// publishing an empty or invented site.
    #[inline(never)]
    pub(crate) fn capture_into(frames: &mut [usize; MAX_SITE_FRAMES]) -> Option<usize> {
        let mut buffer = [std::ptr::null_mut::<c_void>(); MAX_SITE_FRAMES + SKIP];
        // SAFETY: `backtrace` writes at most `buffer.len()` pointers into
        // `buffer`, which is valid for that many elements.
        let captured = unsafe { libc::backtrace(buffer.as_mut_ptr(), buffer.len() as libc::c_int) };
        if captured <= 0 {
            return None;
        }
        let captured = (captured as usize).min(buffer.len());
        if captured <= SKIP {
            return None;
        }
        let depth = (captured - SKIP).min(MAX_SITE_FRAMES);
        for (frame, address) in frames.iter_mut().zip(&buffer[SKIP..]) {
            *frame = *address as usize;
        }
        Some(depth)
    }

    /// Runs `backtrace` once so its lazy initialization happens off the hook.
    pub(crate) fn prewarm() {
        let mut frames = [0usize; MAX_SITE_FRAMES];
        let _ = capture_into(&mut frames);
    }
}

#[cfg(target_os = "windows")]
mod windows {
    use super::MAX_SITE_FRAMES;
    use super::SamplingSupport;
    use std::ffi::c_void;
    use windows_sys::Win32::System::Diagnostics::Debug::RtlCaptureStackBackTrace;

    pub(crate) fn support() -> SamplingSupport {
        SamplingSupport::Supported
    }

    /// Captures up to `MAX_SITE_FRAMES` caller addresses, innermost first.
    ///
    /// `FramesToSkip = 1` drops this function, matching the other backends. The
    /// API caps one call at 62 frames, well above `MAX_SITE_FRAMES`.
    #[inline(never)]
    pub(crate) fn capture_into(frames: &mut [usize; MAX_SITE_FRAMES]) -> Option<usize> {
        let mut buffer = [std::ptr::null_mut::<c_void>(); MAX_SITE_FRAMES];
        // SAFETY: `RtlCaptureStackBackTrace` writes at most `buffer.len()`
        // pointers into `buffer`, which is valid for that many elements. The
        // hash out-parameter is optional; null means "do not compute it".
        let captured = unsafe {
            RtlCaptureStackBackTrace(
                1,
                buffer.len() as u32,
                buffer.as_mut_ptr(),
                std::ptr::null_mut(),
            )
        };
        let depth = usize::from(captured).min(MAX_SITE_FRAMES);
        if depth == 0 {
            return None;
        }
        for (frame, address) in frames.iter_mut().zip(&buffer[..depth]) {
            *frame = *address as usize;
        }
        Some(depth)
    }

    /// Runs the walker once so any lazy setup happens off the hook.
    pub(crate) fn prewarm() {
        let mut frames = [0usize; MAX_SITE_FRAMES];
        let _ = capture_into(&mut frames);
    }
}

#[cfg(any(target_os = "android", all(target_os = "linux", target_env = "musl")))]
mod unwind {
    use super::MAX_SITE_FRAMES;
    use super::SamplingSupport;
    use std::ffi::{c_int, c_void};
    use std::ptr;

    /// Opaque unwinder context; only `_Unwind_GetIP` may read it.
    #[repr(C)]
    struct UnwindContext {
        _private: [u8; 0],
    }

    type TraceFn = extern "C" fn(*mut UnwindContext, *mut c_void) -> c_int;

    // SAFETY: both symbols belong to the Itanium unwinder Rust links on these
    // targets — bionic exports them from libc.so and the musl sysroot supplies
    // them from its bundled libunwind. The signatures match `_Unwind_Trace_Fn`
    // and `_Unwind_GetIP` in unwind.h.
    unsafe extern "C" {
        fn _Unwind_Backtrace(trace: TraceFn, argument: *mut c_void) -> c_int;
        fn _Unwind_GetIP(context: *mut UnwindContext) -> usize;
    }

    /// Stack-local sink handed to the unwinder through its `void *` argument.
    struct Capture {
        frames: *mut usize,
        capacity: usize,
        written: usize,
        skip: usize,
    }

    /// `_URC_NO_REASON` (0) continues the walk; any other value stops it.
    extern "C" fn visit(context: *mut UnwindContext, argument: *mut c_void) -> c_int {
        // SAFETY: `capture_into` passes the address of a live `Capture` that
        // outlives this synchronous walk, and no other thread can reach it.
        let capture = unsafe { &mut *argument.cast::<Capture>() };
        if capture.skip > 0 {
            capture.skip -= 1;
            return 0;
        }
        if capture.written >= capture.capacity {
            return 1;
        }
        // SAFETY: `context` is the unwinder's own frame context, valid for the
        // duration of this callback.
        let address = unsafe { _Unwind_GetIP(context) };
        // A null instruction pointer means the walk ran off the end of the
        // stack; stop instead of recording an invented frame.
        if address == 0 {
            return 1;
        }
        // SAFETY: `written < capacity` was checked above, so this slot is inside
        // the caller's array and has not been written yet.
        unsafe { capture.frames.add(capture.written).write(address) };
        capture.written += 1;
        c_int::from(capture.written >= capture.capacity)
    }

    pub(crate) fn support() -> SamplingSupport {
        SamplingSupport::Supported
    }

    /// Captures up to `MAX_SITE_FRAMES` caller addresses, innermost first.
    #[inline(never)]
    pub(crate) fn capture_into(frames: &mut [usize; MAX_SITE_FRAMES]) -> Option<usize> {
        let mut capture = Capture {
            frames: frames.as_mut_ptr(),
            capacity: MAX_SITE_FRAMES,
            written: 0,
            // The unwinder starts at this function, which is not a caller.
            skip: 1,
        };
        // SAFETY: `capture` lives until after the call returns and
        // `_Unwind_Backtrace` invokes `visit` synchronously on this thread, so
        // the pointer stays valid and unaliased for the whole walk.
        unsafe { _Unwind_Backtrace(visit, ptr::from_mut(&mut capture).cast::<c_void>()) };
        (capture.written > 0).then_some(capture.written)
    }

    /// Runs the unwinder once so its lazy frame-table lookup happens off the hook.
    pub(crate) fn prewarm() {
        let mut frames = [0usize; MAX_SITE_FRAMES];
        let _ = capture_into(&mut frames);
    }
}

#[cfg(not(any(
    target_vendor = "apple",
    all(target_os = "linux", target_env = "gnu"),
    target_os = "windows",
    target_os = "android",
    all(target_os = "linux", target_env = "musl"),
)))]
mod unsupported {
    use super::MAX_SITE_FRAMES;
    use super::SamplingSupport;

    pub(crate) fn support() -> SamplingSupport {
        SamplingSupport::Unsupported {
            platform: std::env::consts::OS,
            reason: reason(),
        }
    }

    pub(crate) fn capture_into(_frames: &mut [usize; MAX_SITE_FRAMES]) -> Option<usize> {
        None
    }

    pub(crate) fn prewarm() {}

    fn reason() -> &'static str {
        #[cfg(target_arch = "wasm32")]
        {
            "stack capture is not available on wasm"
        }
        #[cfg(all(
            target_os = "linux",
            not(target_env = "gnu"),
            not(target_env = "musl")
        ))]
        {
            "this Linux libc exports neither backtrace nor _Unwind_Backtrace; build with glibc or musl"
        }
        #[cfg(not(any(target_arch = "wasm32", target_os = "linux")))]
        {
            "no allocation-site stack capture is implemented for this platform yet"
        }
    }
}

#[cfg(any(target_vendor = "apple", all(target_os = "linux", target_env = "gnu")))]
pub(crate) use backtrace::{capture_into, prewarm, support};

#[cfg(target_os = "windows")]
pub(crate) use windows::{capture_into, prewarm, support};

#[cfg(any(target_os = "android", all(target_os = "linux", target_env = "musl")))]
pub(crate) use unwind::{capture_into, prewarm, support};

#[cfg(not(any(
    target_vendor = "apple",
    all(target_os = "linux", target_env = "gnu"),
    target_os = "windows",
    target_os = "android",
    all(target_os = "linux", target_env = "musl"),
)))]
pub(crate) use unsupported::{capture_into, prewarm, support};
