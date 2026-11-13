//! CPU profiles of the runtime's UI thread.
//!
//! The native build drives [`cove_profiling::CpuSampler`] from a background
//! thread: the sampler follows the thread name the probe recorded when it was
//! built, the worker stops at the requested bound and symbolizes the retained
//! stacks. Native platforms without a sampler report `unavailable` with the
//! reason.
//!
//! WebAssembly cannot be suspended and read from the outside, so the browser
//! build measures instead of sampling: [`cooperative`] accumulates the durations
//! the render path times around its frame phases and answers with
//! [`cove_devtools_protocol::ProfileSource::Instrumented`].

/// Most hotspots one query returns.
pub(crate) const MAX_HOTSPOTS: usize = 200;

mod cooperative;
pub use cooperative::{is_recording, record_frame, record_phase};

#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) use native::{UiProfiler, unavailable_reason};

#[cfg(target_arch = "wasm32")]
mod wasm;
#[cfg(target_arch = "wasm32")]
pub(crate) use wasm::{UiProfiler, unavailable_reason};
