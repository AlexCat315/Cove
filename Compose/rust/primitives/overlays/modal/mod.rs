//! Modals, sheets and their surface presentation policy.
pub mod drag_handle;
pub mod modal;
pub mod modal_options;
pub mod touch_presentation;

pub use drag_handle::SheetDragHandle;
pub use modal::Modal;
pub use modal_options::{
    ModalBackdrop, ModalDismissal, ModalGeometry, ModalOptions, ModalPlacement,
};
pub use touch_presentation::{uses_sheet, TouchPresentation};
