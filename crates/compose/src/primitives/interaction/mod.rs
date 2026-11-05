//! Interactive primitives.
pub(crate) mod drag_and_drop;
pub(crate) mod drag_threshold;
pub mod edge_auto_scroll;
pub mod file_drop;
pub(crate) mod horizontal_swipe;
pub(crate) mod scrollbar;

pub use api::*;

pub(crate) mod api {
    pub use super::drag_and_drop::{DragScope, DragSession, Draggable, DropTarget};
    pub use super::drag_threshold::{
        DRAG_THRESHOLD, crossed as drag_crossed, crossed_along_axis as drag_crossed_along_axis,
    };
    pub use super::edge_auto_scroll::{EdgeAutoScroll, velocity as edge_auto_scroll_velocity};
    pub use super::file_drop::{
        DroppedFile, FileDropOutcome, FileDropPrompt, FileDropRegistry, FileDropRegistryHolder,
        FileDropZone, format_bytes,
    };
    pub use super::horizontal_swipe::{
        HorizontalSwipe, HorizontalSwipeDirection, HorizontalSwipeSession,
    };
    pub use super::scrollbar::{Scrollbar, ScrollbarAxis};
}
