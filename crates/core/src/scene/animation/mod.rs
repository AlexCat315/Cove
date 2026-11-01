// `animation.rs` keeps the name of the Swift file it translates
// (`Animation/Animation.swift`) and is recorded under that path in
// `Docs/rust-migration-files.json`, so the module keeps its name.
#[allow(clippy::module_inception)]
mod animation;
mod context;
mod controller;
mod interpolatable;
mod linear;
mod motion_vector;
mod oscillation;
mod playback;
mod scheduler;
mod spring_solution;
mod timeline;
pub use animation::*;
pub use context::*;
pub use controller::*;
pub use interpolatable::*;
pub use linear::*;
pub use motion_vector::*;
pub use playback::*;
pub use scheduler::*;
pub use spring_solution::*;
pub use timeline::*;
