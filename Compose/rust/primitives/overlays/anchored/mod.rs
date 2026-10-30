//! Anchored overlays: placement geometry, portal boundary and bubble chrome.
pub mod anchoring;
pub mod placement;

pub use anchoring::{AnchoredOverlayHost, PopoverAnchor, PopoverSurfaceHost};
pub use placement::{
    PopoverEdge, PopoverGeometry, PopoverOverflow, PopoverPlacement, PopoverSurface,
};
