//! Translation of Primitives/Layout/Sidebar/AdaptiveOptions.swift.
//!
//! The container decides between an expanded sidebar, a rail and modal
//! navigation; the caller only supplies the breakpoints.
/// Which navigation presentation a container width resolves to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SidebarMode {
    /// Both the sidebar and its content fit side by side.
    Expanded,
    /// A narrow rail sits beside the content.
    Compact,
    /// Navigation moves into a modal drawer.
    Modal,
}

impl Default for SidebarMode {
    fn default() -> Self {
        Self::Expanded
    }
}

/// Breakpoints for the adaptive sidebar.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AdaptiveSidebarOptions {
    /// Below this width the sidebar becomes a rail.
    pub rail_below: f32,
    /// Below this width navigation becomes modal.
    pub modal_below: f32,
    /// Width the expanded sidebar wants.
    pub expanded_width: f32,
    /// Whether the rail shows the destination labels.
    pub rail_shows_labels: bool,
    /// How the rail is presented in compact mode.
    pub prefer_rail: bool,
}

impl Default for AdaptiveSidebarOptions {
    fn default() -> Self {
        Self {
            rail_below: 900.0,
            modal_below: 600.0,
            expanded_width: 240.0,
            rail_shows_labels: false,
            prefer_rail: true,
        }
    }
}

impl AdaptiveSidebarOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn validate(&mut self) {
        self.rail_below = if self.rail_below.is_finite() { self.rail_below.max(0.0) } else { 900.0 };
        self.modal_below = if self.modal_below.is_finite() { self.modal_below.max(0.0) } else { 600.0 };
        // Modal is the narrower state, so it must never trigger at a wider
        // container than the rail breakpoint.
        if self.modal_below > self.rail_below {
            self.modal_below = self.rail_below;
        }
        self.expanded_width = if self.expanded_width.is_finite() { self.expanded_width.max(44.0) } else { 240.0 };
    }

    pub fn validated(mut self) -> Self {
        self.validate();
        self
    }

    /// Presentation for a container width.
    pub fn mode(&self, width: f32) -> SidebarMode {
        if !width.is_finite() {
            return SidebarMode::Expanded;
        }
        if width < self.modal_below {
            SidebarMode::Modal
        } else if width < self.rail_below && self.prefer_rail {
            SidebarMode::Compact
        } else {
            SidebarMode::Expanded
        }
    }

    /// Whether the modal drawer should be presented at this width.
    pub fn uses_drawer(&self, width: f32) -> bool {
        self.mode(width) == SidebarMode::Modal
    }
}
