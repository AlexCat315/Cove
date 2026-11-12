//! Runtime instrumentation shared by native and browser hosts.
pub mod inspection;
#[cfg(not(target_arch = "wasm32"))]
pub mod probe_transport;
pub mod runtime_probe;

pub use cove_agent as agent;
pub use cove_agent::transport as agent_transport;
pub use cove_devtools_protocol as protocol;
