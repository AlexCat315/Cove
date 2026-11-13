//! Stack capture for allocation-site samples.
//!
//! Capture is implemented where `libc::backtrace` is exported: Apple platforms
//! and GNU Linux. Every other target reports [`SamplingSupport::Unsupported`] with a
//! reason and never records a site — no synthetic stacks are produced.
//!
//! `backtrace` reads the current thread's stack without taking the allocator's
//! lock. It may lazily load an unwinder the first time it runs, which can
//! allocate; [`prewarm`] runs that first call from `start_sampling`, outside the
//! allocator hook, and the hook's thread-local recursion guard covers any later
//! internal allocation.

use super::{MAX_SITE_FRAMES, SamplingSupport};

#[cfg(any(target_vendor = "apple", all(target_os = "linux", target_env = "gnu")))]
mod supported {
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

#[cfg(not(any(target_vendor = "apple", all(target_os = "linux", target_env = "gnu"))))]
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
        #[cfg(target_os = "windows")]
        {
            "libc::backtrace is not available on Windows; use an ETW heap profiler until a Windows stack walker lands"
        }
        #[cfg(all(target_os = "linux", target_env = "musl"))]
        {
            "musl does not export backtrace; build with glibc or use an external heap profiler"
        }
        #[cfg(target_arch = "wasm32")]
        {
            "stack capture is not available on wasm"
        }
        #[cfg(not(any(
            target_os = "windows",
            all(target_os = "linux", target_env = "musl"),
            target_arch = "wasm32"
        )))]
        {
            "no allocation-site stack capture is implemented for this platform yet"
        }
    }
}

#[cfg(any(target_vendor = "apple", all(target_os = "linux", target_env = "gnu")))]
pub(crate) use supported::{capture_into, prewarm, support};

#[cfg(not(any(target_vendor = "apple", all(target_os = "linux", target_env = "gnu"))))]
pub(crate) use unsupported::{capture_into, prewarm, support};
