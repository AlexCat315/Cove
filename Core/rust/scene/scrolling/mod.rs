//! Translation of CoveUIScene/Scrolling/*.
mod animator;
mod physics;
mod reveal;

pub use animator::*;
pub use physics::*;
/// Wheel-routing policy is consumed by scroll containers, which live above the
/// input layer, so re-export it with the rest of the scrolling vocabulary.
pub use crate::scene::input::{ScrollConsumePolicy, WheelRoutingPriority};
