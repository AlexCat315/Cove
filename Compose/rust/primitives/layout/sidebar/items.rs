//! Translation of Primitives/Layout/Sidebar/Items.swift.
//!
//! Authored navigation content. Session state (expansion, focus, reveal) stays
//! outside these values so they remain comparable inputs.
use cove_imaging::BundleImageResource;
use crate::primitives::overlays::menu::MenuEntry;
use crate::theme::SemanticIconSize;
use crate::AnyView;

/// Inline views for one sidebar row.
///
/// `leading` and `trailing` sit inside the destination's hit area, so they are
/// decorative and hidden from assistive technology (the title remains the row's
/// name). `accessory` sits outside the destination button at the trailing edge
/// and may be interactive; it reserves `accessory_width` so the title never runs
/// under it. Rail presentations keep only the icon.
#[derive(Clone, Default)]
pub struct SidebarItemSlots {
    pub leading: Option<AnyView>,
    pub trailing: Option<AnyView>,
    pub accessory: Option<AnyView>,
    pub accessory_width: f32,
}

impl SidebarItemSlots {
    pub fn new() -> Self {
        Self { leading: None, trailing: None, accessory: None, accessory_width: 44.0 }
    }

    pub fn accessory_width(mut self, width: f32) -> Self {
        self.accessory_width = if width.is_finite() { width.clamp(0.0, 240.0) } else { 44.0 };
        self
    }

    /// Leading lane content wins over the item's icon.
    pub fn occupies_leading_lane(&self) -> bool {
        self.leading.is_some()
    }
}

/// One authored navigation destination or branch.
#[derive(Clone)]
pub struct SidebarItem<Id: PartialEq + Clone> {
    pub id: Id,
    pub title: String,
    /// Uses the active icon theme, including resource replacements and scaling.
    pub icon: Option<BundleImageResource>,
    pub icon_size: SemanticIconSize,
    pub badge: Option<String>,
    pub is_enabled: bool,
    /// A non-empty group expands instead of activating a destination.
    pub children: Vec<SidebarItem<Id>>,
    /// Per-item actions. Right-click opens them; a trailing ellipsis keeps the
    /// same menu reachable by pointer, keyboard and touch.
    pub context_menu: Vec<MenuEntry>,
    /// Caller views placed inside the row (expanded presentation only).
    pub slots: SidebarItemSlots,
}

impl<Id: PartialEq + Clone> SidebarItem<Id> {
    pub fn new(id: Id, title: impl Into<String>) -> Self {
        Self {
            id,
            title: title.into(),
            icon: None,
            icon_size: SemanticIconSize::medium(),
            badge: None,
            is_enabled: true,
            children: Vec::new(),
            context_menu: Vec::new(),
            slots: SidebarItemSlots::new(),
        }
    }

    pub fn icon(mut self, icon: BundleImageResource) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn icon_size(mut self, size: SemanticIconSize) -> Self {
        self.icon_size = size;
        self
    }

    pub fn badge(mut self, badge: impl Into<String>) -> Self {
        self.badge = Some(badge.into());
        self
    }

    pub fn enabled(mut self, is_enabled: bool) -> Self {
        self.is_enabled = is_enabled;
        self
    }

    pub fn children(mut self, children: Vec<SidebarItem<Id>>) -> Self {
        self.children = children;
        self
    }

    pub fn context_menu(mut self, entries: Vec<MenuEntry>) -> Self {
        self.context_menu = entries;
        self
    }

    pub fn slots(mut self, slots: SidebarItemSlots) -> Self {
        self.slots = slots;
        self
    }

    pub fn is_branch(&self) -> bool {
        !self.children.is_empty()
    }
}

/// One titled group of destinations.
#[derive(Clone)]
pub struct SidebarSection<Id: PartialEq + Clone> {
    pub id: String,
    pub title: String,
    pub items: Vec<SidebarItem<Id>>,
}

impl<Id: PartialEq + Clone> SidebarSection<Id> {
    pub fn new(id: impl Into<String>, items: Vec<SidebarItem<Id>>) -> Self {
        Self { id: id.into(), title: String::new(), items }
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }
}
