//! Bounded, in-process CPU sampling for Cove's DevTools performance surface.
//!
//! [`CpuSampler`] follows one named thread of the current process, suspends it
//! briefly at a fixed interval, records its stack into a fixed-size ring and
//! resumes it. [`Symbolizer`] then turns the retained instruction addresses
//! into `function file:line` with DWARF symbols. Cove's DevTools probe drives the
//! sampler from a background thread for `start_ui_profile`, `stop_ui_profile`,
//! `get_ui_profile` and `query_ui_hotspots`.
//!
//! # What is real
//!
//! On macOS (arm64 and x86_64) the sampler uses Mach thread state from
//! `thread_get_state` on thread ports obtained with `task_threads` for the
//! current task — no `task_for_pid`, entitlements or elevated rights — and
//! walks the frame-pointer chain by reading the suspended thread's stack.
//! Stacks are real; nothing is estimated. A build that omits frame pointers
//! yields the sampled instruction address but may truncate callers.
//! Symbolization reads the measured image list from dyld and resolves through
//! `addr2line` (DWARF, split-DWARF object files and dSYM bundles), falling back
//! to the dynamic symbol table when DWARF is absent.
//!
//! # Platform gaps
//!
//! Linux and Windows do not have a sampler in this milestone. There,
//! [`CpuSampler::start`] returns [`SamplingError::Unsupported`] with a reason,
//! and [`Symbolizer::new`] returns [`SymbolizationError::Unsupported`];
//! [`platform_support`] reports the gap up front. No samples are synthesized
//! on those platforms.
//!
//! # Sampling rules
//!
//! - Start the sampler from a thread other than the one being sampled; the
//!   caller must stay runnable while the target is suspended.
//! - Thread names are the names set with `pthread_setname_np`, which macOS
//!   truncates to 63 bytes.
//! - `max_samples` bounds memory. When the ring is full, the oldest sample is
//!   overwritten and counted in [`SampledProfile::samples_dropped`].
//! - Only allocations that happened before the target thread was suspended are
//!   used by the sampler loop, so a suspended thread cannot deadlock the
//!   allocator.

mod config;
mod error;
mod platform;
mod profile;
mod sampler;
mod symbolizer;

pub use config::{
    MAX_SAMPLE_INTERVAL, MAX_SAMPLES, MAX_STACK_DEPTH, MIN_SAMPLE_INTERVAL, SampleConfig,
};
pub use error::{SamplingError, SymbolizationError};
pub use profile::{
    Sample, SampledProfile, SourceLocation, SymbolizedFrame, SymbolizedProfile, SymbolizedSample,
};
pub use sampler::CpuSampler;
pub use symbolizer::Symbolizer;

/// Whether CPU sampling and symbolization can produce real data here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlatformSupport {
    /// A real sampler is implemented for this platform and architecture.
    Supported,
    /// The platform has no sampler yet; the reason names the gap.
    Unsupported {
        /// Why it is missing and what to use instead.
        reason: &'static str,
    },
}

/// Reports sampler support before a caller offers a profiling action.
pub fn platform_support() -> PlatformSupport {
    platform::support()
}
