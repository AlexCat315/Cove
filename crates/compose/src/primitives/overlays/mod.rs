//! Overlay primitives.
pub mod anchored;
pub mod asset_drop_target;
pub(crate) mod command_palette;
pub(crate) mod command_palette_model;
pub(crate) mod context_menu;
pub(crate) mod focus_scope;
pub mod hover_card;
pub mod menu;
pub mod modal;
pub(crate) mod overlay_host;
pub(crate) mod popconfirm;
pub(crate) mod popover;
pub mod portal;
pub(crate) mod roving_focus;
pub(crate) mod selection_key_host;
pub(crate) mod shortcut_host;
pub mod support;
pub mod toast;
pub mod tooltip;

pub use api::*;
pub use overlay_host::OverlayHost;

pub use command_palette_model::{
    CommandEntry, CommandItem, destination, display_entries, entries, normalize, ranked,
    record_recent,
};
pub use roving_focus::{
    RovingFocusHost, RovingFocusRequest, RovingItem, SelectionNavigation, roving_identity,
};
pub use shortcut_host::ShortcutHost;
pub use tooltip::{
    TooltipArbitration, TooltipArbitrationAmbient, TooltipArbitrationHolder, TooltipSession,
};

pub(crate) mod api {
    pub use super::anchored::{
        PopoverAnchor, PopoverEdge, PopoverGeometry, PopoverOverflow, PopoverPlacement,
        PopoverSurface,
    };
    pub use super::asset_drop_target::AssetDropTarget;
    pub use super::command_palette::{CommandPalette, PaletteCommand};
    pub use super::context_menu::{ContextMenu, ContextMenuExt};
    pub use super::focus_scope::FocusScope;
    pub use super::hover_card::HoverCard;
    pub use super::menu::{
        Menu, MenuDescriptor, MenuEntry, MenuItem, MenuItemDescriptor, MenuItemRole, MenuSubmenu,
    };
    pub use super::modal::{
        ActionSheet, ActionSheetAction, ActionSheetQuestion, AlertDialog, AlertDialogOptions,
        BottomSheet, BottomSheetOptions, Dialog, DialogOptions, Modal, ModalBackdrop,
        ModalDismissal, ModalGeometry, ModalOptions, ModalPlacement, Sheet, SheetDragHandle,
        SheetOptions, TouchPresentation, uses_sheet,
    };
    pub use super::popconfirm::Popconfirm;
    pub use super::popover::{Popover, PopoverSizing, PopoverTriggerDescription};
    pub use super::portal::{LayerRoot, OverlayPlacement, PortalHost};
    pub use super::toast::{
        NotificationHost, ToastAction, ToastController, ToastNotification, ToastPosition,
    };
    pub use super::tooltip::{Tooltip, TooltipAnchor, TooltipExt, TooltipOptions};
}
