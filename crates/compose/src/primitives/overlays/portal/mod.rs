//! Scene-local portal registry.
pub mod fit_constraints;
pub mod overlay_placement;
pub mod portal_placement;
pub mod portal_registry;
pub mod segments;

pub use fit_constraints::PortalFitConstraints;
pub use overlay_placement::OverlayPlacement;
pub use portal_placement::{PortalPlacement, PortalPlacementMode, portal_window_bounds};
pub use portal_registry::{
    PortalEntry, PortalResource, PortalStore, PortalStoreAmbient, PortalStoreEnvironment,
    PortalStoreHolder,
};
pub use segments::{anchored_bounds, overlay_segments, presentation_bounds};
pub mod layer_root;
pub mod portal_host;
pub use layer_root::{LayerRoot, PORTAL_LAYER_Z_INDEX};
pub use portal_host::PortalHost;
