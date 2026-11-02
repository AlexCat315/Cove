//! Interactive primitives.
pub(crate) mod drag_and_drop;
pub mod edge_auto_scroll;
pub(crate) mod horizontal_swipe;
pub(crate) mod scrollbar;

pub use api::*;

pub(crate) mod api {
    pub use super::drag_and_drop::{DragScope, DragSession, Draggable, DropTarget};
    pub use super::edge_auto_scroll::{EdgeAutoScroll, velocity as edge_auto_scroll_velocity};
    pub use super::horizontal_swipe::{
        HorizontalSwipe, HorizontalSwipeDirection, HorizontalSwipeSession,
    };
    pub use super::scrollbar::{Scrollbar, ScrollbarAxis};
}
