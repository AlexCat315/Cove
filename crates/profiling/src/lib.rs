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
//! On macOS and iOS (arm64 and x86_64) the sampler uses Mach thread state from
//! `thread_get_state` on thread ports obtained with `task_threads` for the
//! current task — no `task_for_pid`, entitlements or elevated rights — and
//! walks the frame-pointer chain by reading the suspended thread's stack.
//! Symbolization reads the measured image list from dyld and resolves through
//! `addr2line` (DWARF, split-DWARF object files and dSYM bundles), falling back
//! to the dynamic symbol table when DWARF is absent.
//!
//! On Linux and Android (arm64 and x86_64) the sampler delivers `SIGPROF` to
//! one thread with `tgkill`. The handler runs on that thread, reads the
//! registers the kernel saved in its signal context and walks the same
//! frame-pointer chain inside the thread's own stack mapping. It allocates
//! nothing, takes no lock and opens no file: stack bounds come from a
//! `/proc/self/maps` snapshot taken before the handler was installed. No
//! ptrace, perf event or elevated right is needed. Symbolization reads the
//! executable mappings from `/proc/self/maps`, derives each ELF image's load
//! bias from its `PT_LOAD` headers and resolves through `addr2line`, falling
//! back to `dladdr`.
//!
//! On Windows (arm64 and x86_64) the sampler enumerates this process's threads
//! with a toolhelp snapshot, matches the description Rust sets with
//! `SetThreadDescription`, then suspends the target, reads `GetThreadContext`
//! and walks the frame-pointer chain inside the committed region
//! `VirtualQuery` reports. The target is resumed on every exit path.
//! Symbolization goes through dbghelp (`SymFromAddrW`, `SymGetLineFromAddrW64`),
//! which reads PDB and DWARF alike; dbghelp is process-global, so its calls are
//! serialized and the last symbolizer cleans it up.
//!
//! Stacks are real on every backend; nothing is estimated. A build that omits
//! frame pointers yields the sampled instruction address but may truncate
//! callers.
//!
//! # Platform gaps
//!
//! Targets other than the ones above — including 32-bit and wasm builds — have
//! no sampler. There, [`CpuSampler::start`] returns
//! [`SamplingError::Unsupported`] with a reason, and [`Symbolizer::new`]
//! returns [`SymbolizationError::Unsupported`]; [`platform_support`] reports
//! the gap up front. No samples are synthesized on those targets.
//!
//! # Sampling rules
//!
//! - Start the sampler from a thread other than the one being sampled; the
//!   caller must stay runnable while the target is suspended.
//! - Thread names are the names set with `pthread_setname_np`. Darwin truncates
//!   them to 63 bytes; the Linux kernel truncates `comm` to 15 bytes, so a
//!   longer name is matched by its truncated prefix. Windows matches the full
//!   UTF-16 thread description.
//! - Linux and Android install a process-wide `SIGPROF` handler, so only one
//!   sampler can run at a time; a second [`CpuSampler::start`] returns
//!   [`SamplingError::SamplerBusy`]. The handler refuses to displace one the
//!   application already owns, and while it runs `SIGPROF` delivered to other
//!   threads is ignored.
//! - On Linux and Android a sample is recorded when the signal is delivered,
//!   so a target that blocks `SIGPROF` delays its samples. macOS and Windows
//!   suspend the target instead and sample synchronously.
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
