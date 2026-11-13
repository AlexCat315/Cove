//! CPU profiles of the runtime's UI thread.
//!
//! The native build drives [`cove_profiling::CpuSampler`] from a background
//! thread: the sampler follows the thread name the probe recorded when it was
//! built, the worker stops at the requested bound and symbolizes the retained
//! stacks. Platforms without a sampler report `unavailable` with the reason;
//! the browser build has no in-process sampler at all.

#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) use native::{UiProfiler, unavailable_reason};

#[cfg(target_arch = "wasm32")]
mod unavailable;
#[cfg(target_arch = "wasm32")]
pub(crate) use unavailable::{UiProfiler, unavailable_reason};
