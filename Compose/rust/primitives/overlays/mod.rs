//! Overlay primitives.
pub mod anchored;
pub mod support;
pub mod popover;
pub mod portal;
pub mod focus_scope;
pub mod selection_key_host;
pub mod menu;
pub mod modal;
pub mod roving_focus;

pub use focus_scope::FocusScope;
pub use selection_key_host::SelectionKeyHost;
pub use menu::{MenuDescriptor, MenuItemDescriptor};
pub use anchored::{
    AnchoredOverlayHost, PopoverAnchor, PopoverEdge, PopoverGeometry, PopoverOverflow,
    PopoverPlacement, PopoverSurface, PopoverSurfaceHost,
};
pub use popover::{
    Popover, PopoverSizing, PopoverTriggerDescription,
};
pub use portal::OverlayPlacement;
pub use modal::{
    uses_sheet, Modal, ModalBackdrop, ModalDismissal, ModalGeometry, ModalOptions, ModalPlacement,
    SheetDragHandle, TouchPresentation,
};
pub use roving_focus::{
    roving_identity, RovingFocusHost, RovingFocusRequest, RovingItem, SelectionNavigation,
};
