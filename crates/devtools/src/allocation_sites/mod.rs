//! Allocation sites collected by the runtime's own `cove_alloc` sampler.
//!
//! `start` drives `cove_alloc::start_sampling` in the runtime's process, so
//! every allocation that passes through the installed tracking allocator is
//! sampled. `stop` reads the retained raw stacks, resolves them with
//! `cove_profiling::Symbolizer` and returns bounded sites. The counters are
//! cumulative sampled allocations, not the live heap, and no object type,
//! retaining path or leak is inferred.
//!
//! The browser build cannot capture stacks: every command reports the same
//! explicit gap instead of synthesizing sites.

#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) use native::{AllocationSites, unavailable_reason};

#[cfg(target_arch = "wasm32")]
mod unavailable;
#[cfg(target_arch = "wasm32")]
pub(crate) use unavailable::{AllocationSites, unavailable_reason};
