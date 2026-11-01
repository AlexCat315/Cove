//! Responsive custom layouts.
pub mod action_group;
pub mod candidates;
pub mod stack;

pub use action_group::ActionGroupLayout;
pub use candidates::LayoutThatFits;
pub use stack::{StackAxis, StackLayout};
pub mod view_that_fits;
pub use view_that_fits::{ViewThatFits, ViewThatFitsMode};
