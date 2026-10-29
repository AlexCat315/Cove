//! NativeRHI: the backend-neutral rendering hardware interface.
//!
//! The module layout follows `Graphics/Sources/NativeRHI`: value types, the
//! backend trait, the recorded command model, submission plumbing and the
//! validation caches.
mod access_tracker;
mod backend;
mod backend_factory;
mod caches;
mod capabilities;
mod commands;
mod descriptors;
mod device;
mod error;
mod handles;
mod module;
mod pipeline_cache;
mod pipeline_interface;
mod pipeline_validation;
mod presentation;
mod queue;
mod resource_preparation;
mod resources;
mod shader_artifact;
mod specialization;
mod resource_ownership;
mod state_tracker;
mod submission;
mod submission_planner;
#[cfg(test)]
mod testing;
mod types;
mod validation;
mod visibility;

pub use access_tracker::*;
pub use backend::*;
pub use backend_factory::*;
pub use caches::*;
pub use capabilities::*;
pub use commands::*;
pub use descriptors::*;
pub use device::*;
pub use error::*;
pub use handles::*;
pub use module::*;
pub use pipeline_cache::*;
pub use pipeline_interface::*;
pub use pipeline_validation::*;
pub use presentation::*;
pub use queue::*;
pub use resource_preparation::*;
pub use resources::*;
pub use shader_artifact::*;
pub use specialization::*;
pub use resource_ownership::*;
pub use state_tracker::*;
pub use submission::*;
pub use submission_planner::*;
pub use types::*;
pub use validation::*;
pub use visibility::*;
