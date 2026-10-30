//! Navigation sidebar: authored items, hierarchy projection, presentation,
//! adaptive breakpoints, the resize lane and focus handoff.
pub mod adaptive_options;
pub mod focus_handoff;
pub mod hierarchy;
pub mod items;
pub mod presentation;
pub mod rail;
pub mod resize_handle;
pub mod resizing;
pub mod sidebar;

pub use adaptive_options::{AdaptiveSidebarOptions, SidebarMode};
pub use focus_handoff::SidebarFocusHandoff;
pub use hierarchy::{SidebarEntry, SidebarHierarchy};
pub use items::{SidebarItem, SidebarItemSlots, SidebarSection};
pub use presentation::{SidebarPresentation, SidebarResizeOptions, SidebarToggle};
pub use rail::{RailDestination, SidebarRail};
pub use resize_handle::SidebarResizeHandle;
pub use resizing::{clamp_width, resolved_width};
pub use sidebar::Sidebar;
