//! DevTools client, inspector, agent and local server.
//!
//! Layering, bottom-up:
//! - `client` — per-connection protocol state (re-exported from the protocol crate)
//! - `inspection` — scene snapshots, temporary styles, read-only diagnostics
//! - `agent` — JSON Schema subset, tool loop, skills, MCP
//! - `server` — WebSocket transport, host adapters, protocol dispatch
//! - `devtools` — frame mirroring, timing, input bridging, one-stop host entry
//! - `probe` — headless diagnostic host
pub mod agent;
pub mod agent_transport;
pub mod client;
pub mod devtools;
pub mod inspection;
pub mod probe;
pub mod server;

pub use cove_devtools_protocol as protocol;
