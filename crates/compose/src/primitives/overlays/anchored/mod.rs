//! Anchored overlays: placement geometry, portal boundary and bubble chrome.
pub mod anchoring;
pub mod placement;

pub use anchoring::PopoverAnchor;
pub(crate) use anchoring::{AnchoredOverlayHost, MenuKeyHost};
pub use placement::{
    PopoverEdge, PopoverGeometry, PopoverOverflow, PopoverPlacement, PopoverSurface,
};
