//! Geometry animations.
pub mod matched;
pub mod sliding_selection;
pub mod transforms;

pub use matched::{Matched, MatchedSurface};
pub use sliding_selection::{
    SlidingSelection, SlidingSelectionMark, SlidingSelectionStyle, install_sliding_selection,
    is_in_sliding_selection, sliding_selection_scope,
};
pub use transforms::{TransformViewExt, offset, scale_effect, scale_effect_anchored};
