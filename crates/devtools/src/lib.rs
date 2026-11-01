//! DevTools client, inspector, agent and local server.
//!
//! Layering, bottom-up:
//! - `client` — per-connection protocol state (re-exported from the protocol crate)
//! - `inspection` — scene snapshots, temporary styles, read-only diagnostics
//! - `agent` — re-export of `cove-agent` (schema, tools, skills, MCP, transports)
//! - `server` — WebSocket transport, host adapters, protocol dispatch
//! - `devtools` — frame mirroring, timing, input bridging, one-stop host entry
//! - `probe` — headless diagnostic host
pub mod client;
pub mod devtools;
pub mod inspection;
pub mod probe;
pub mod server;

pub use cove_agent as agent;
pub use cove_agent::transport as agent_transport;
pub use cove_devtools_protocol as protocol;
