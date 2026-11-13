//! Runtime instrumentation shared by native and browser hosts.
mod allocation_sites;
pub mod frame_analysis;
pub mod gpu_breakdown;
pub mod inspection;
#[cfg(not(target_arch = "wasm32"))]
pub mod probe_transport;
mod profile;
#[cfg(not(target_arch = "wasm32"))]
pub mod runtime_capture;
mod runtime_input;
pub mod runtime_probe;

pub use cove_agent as agent;
pub use cove_agent::transport as agent_transport;
pub use cove_devtools_protocol as protocol;
