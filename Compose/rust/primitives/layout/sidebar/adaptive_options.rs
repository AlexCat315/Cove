//! Translation of Primitives/Layout/Sidebar/AdaptiveOptions.swift.
//!
//! The container decides between an expanded sidebar, a rail and modal
//! navigation; the caller only supplies the breakpoints. `SidebarGeometry`
//! resolves every derived measurement once so the shell, the resize lane and the
//! drawer agree on the same numbers.
use super::presentation::SidebarResizeOptions;

/// Breakpoints and authored widths for the adaptive sidebar.
#[derive(Clone, Debug, PartialEq)]
pub struct SidebarLayoutPolicy {
    pub compact_maximum_width: f32,
    pub expanded_minimum_width: f32,
    pub sidebar_width: f32,
    pub shows_rail_labels: bool,
}

impl Default for SidebarLayoutPolicy {
    fn default() -> Self {
        Self {
            compact_maximum_width: cove_core::Viewport::COMPACT_MAXIMUM_WIDTH,
            expanded_minimum_width: cove_core::Viewport::EXPANDED_MINIMUM_WIDTH,
            sidebar_width: 240.0,
            shows_rail_labels: true,
        }
    }
}

impl SidebarLayoutPolicy {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn validate(&mut self) {
        self.compact_maximum_width = if self.compact_maximum_width.is_finite() {
            self.compact_maximum_width.max(0.0)
        } else {
            600.0
        };
        self.expanded_minimum_width = if self.expanded_minimum_width.is_finite() {
            self.expanded_minimum_width.max(self.compact_maximum_width)
        } else {
            self.compact_maximum_width.max(1000.0)
        };
        self.sidebar_width = if self.sidebar_width.is_finite() {
            self.sidebar_width.max(44.0)
        } else {
            240.0
        };
    }

    pub fn validated(mut self) -> Self {
        self.validate();
        self
    }
}

/// Which navigation presentation a container width resolves to.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SidebarMode {
    /// Navigation moves into a modal drawer.
    Compact,
    /// A narrow rail sits beside the content.
    Rail,
    /// Both the sidebar and its content fit side by side.
    #[default]
    Expanded,
}

/// Authored options for the adaptive sidebar.
#[derive(Clone, Debug, PartialEq)]
pub struct AdaptiveSidebarOptions {
    pub layout: SidebarLayoutPolicy,
    pub resizing: SidebarResizeOptions,
    pub open_label: String,
    pub collapse_label: String,
    pub expand_label: String,
    /// A rightward gesture beginning within the leading 24 pt opens compact
    /// navigation. Enabled only on surfaces that support touch; ordinary taps
    /// still reach content.
    pub allows_edge_swipe: bool,
}

impl Default for AdaptiveSidebarOptions {
    fn default() -> Self {
        Self {
            layout: SidebarLayoutPolicy::default(),
            resizing: SidebarResizeOptions::default(),
            open_label: "Open navigation".to_string(),
            collapse_label: "Collapse sidebar".to_string(),
            expand_label: "Expand sidebar".to_string(),
            allows_edge_swipe: false,
        }
    }
}

impl AdaptiveSidebarOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn validated(mut self) -> Self {
        self.layout.validate();
        self.resizing.validate();
        self
    }
}

/// Every derived measurement the adaptive shell needs, resolved in one place.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SidebarGeometry {
    pub mode: SidebarMode,
    pub viewport_width: f32,
    pub docked_width: f32,
    pub panel_width: f32,
    pub is_modal: bool,
    pub resize_handle_width: f32,
    pub resize_range: (f32, f32),
    /// A rail the person chose at a width that fits the full sidebar.
    pub is_collapsed: bool,
}

impl SidebarGeometry {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        width: f32,
        policy: &SidebarLayoutPolicy,
        is_presented: bool,
        is_collapsed: bool,
        requested_width: Option<f32>,
        resizing: &SidebarResizeOptions,
        handle_extent: f32,
    ) -> Self {
        let viewport_width = if width.is_finite() { width.max(0.0) } else { 0.0 };
        let fitting = if viewport_width < policy.compact_maximum_width {
            SidebarMode::Compact
        } else if viewport_width < policy.expanded_minimum_width {
            SidebarMode::Rail
        } else {
            SidebarMode::Expanded
        };
        let is_collapsed = fitting == SidebarMode::Expanded && is_collapsed;
        let mode = if is_collapsed { SidebarMode::Rail } else { fitting };
        let initial_range = resizing.range(viewport_width, handle_extent);
        let resize_handle_width = if mode == SidebarMode::Expanded
            && requested_width.is_some()
            && initial_range.1 > initial_range.0
        {
            handle_extent
        } else {
            0.0
        };
        let resize_range = resizing.range(viewport_width, resize_handle_width);
        let requested = requested_width
            .filter(|value| value.is_finite())
            .unwrap_or(policy.sidebar_width);
        let resolved = match requested_width {
            None => policy.sidebar_width,
            Some(_) => requested.clamp(resize_range.0, resize_range.1),
        };
        let docked_width = match mode {
            SidebarMode::Expanded => resolved.min(viewport_width),
            SidebarMode::Rail => if policy.shows_rail_labels { 72.0 } else { 56.0 },
            SidebarMode::Compact => 0.0,
        };
        let is_modal = mode != SidebarMode::Expanded && is_presented;
        let panel_width = if is_modal || mode == SidebarMode::Compact {
            (viewport_width - 56.0).max(0.0).min(320.0)
        } else {
            docked_width
        };
        Self {
            mode,
            viewport_width,
            docked_width,
            panel_width,
            is_modal,
            resize_handle_width,
            resize_range,
            is_collapsed,
        }
    }

    pub fn is_visible(&self) -> bool {
        self.mode != SidebarMode::Compact || self.is_modal
    }

    /// Rail is the presentation used whenever the shell is not docked.
    pub fn presentation(&self, shows_rail_labels: bool) -> super::presentation::SidebarPresentation {
        match self.mode {
            SidebarMode::Expanded => super::presentation::SidebarPresentation::Expanded,
            SidebarMode::Rail | SidebarMode::Compact => {
                super::presentation::SidebarPresentation::Rail { shows_labels: shows_rail_labels }
            }
        }
    }
}
