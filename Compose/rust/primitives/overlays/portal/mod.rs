//! Scene-local portal registry.
pub mod portal_registry;
pub mod fit_constraints;
pub mod portal_placement;
pub mod overlay_placement;

pub use fit_constraints::PortalFitConstraints;
pub use portal_placement::{portal_window_bounds, PortalPlacement, PortalPlacementMode};
pub use overlay_placement::OverlayPlacement;
