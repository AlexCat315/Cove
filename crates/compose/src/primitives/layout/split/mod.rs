//! Translation of Primitives/Layout/SplitView.swift and
//! Primitives/Layout/Split/{Constraints,Retention}.swift.
pub mod constraints;
pub mod handle;
pub mod retention;
pub mod split_view;

pub use constraints::SplitConstraints;
pub use handle::SplitHandle;
pub use retention::{
    CONTAINER_MARKER, PANE_MARKER, SplitFocusHandoff, SplitFocusTransfer, SplitRetainedRegion,
};
pub use split_view::{SplitAxis, SplitView};
