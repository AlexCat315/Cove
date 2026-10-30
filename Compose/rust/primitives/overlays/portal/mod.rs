//! Scene-local portal registry.
pub mod portal_registry;
pub mod fit_constraints;
pub mod portal_placement;
pub mod overlay_placement;

pub use fit_constraints::PortalFitConstraints;
pub use portal_registry::{
    PortalEntry, PortalResource, PortalStore, PortalStoreEnvironment, PortalStoreHolder,
};
pub use portal_placement::{portal_window_bounds, PortalPlacement, PortalPlacementMode};
pub use overlay_placement::OverlayPlacement;
pub mod layer_root;
pub mod portal_host;
pub use layer_root::{LayerRoot, PORTAL_LAYER_Z_INDEX};
pub use portal_host::PortalHost;
