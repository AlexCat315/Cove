//! Request dispatch groups.
//!
//! Swift split dispatch across `Requests/{Checkpoint,Diagnostics,Mirror,
//! Observation,Recording,Scene,Window}.swift`, all as extensions of `DevServer`.
//! Rust methods on one type are already grouped by name, so the handlers live in
//! `server/mod.rs` next to the state they mutate; keeping seven files that each
//! reach into the same private fields would add indirection without a boundary.
//!
//! This module documents that mapping and re-exports the transport and config
//! surfaces.
pub use super::config::*;
pub use super::transport::*;
