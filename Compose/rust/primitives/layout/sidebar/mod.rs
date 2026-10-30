//! Navigation sidebar: authored items, hierarchy projection, presentation and
//! the resize lane.
pub mod hierarchy;
pub mod items;
pub mod presentation;
pub mod rail;
pub mod resize_handle;
pub mod resizing;

pub use hierarchy::{SidebarEntry, SidebarHierarchy};
pub use items::{SidebarItem, SidebarItemSlots, SidebarSection};
pub use presentation::{SidebarPresentation, SidebarResizeOptions, SidebarToggle};
pub use rail::{RailDestination, SidebarRail};
pub use resize_handle::SidebarResizeHandle;
pub use resizing::{clamp_width, resolved_width};
