//! Translation of Primitives/Layout/Sidebar/Presentation.swift.
//!
//! Present the same destination nodes as a full sidebar or a narrow rail.
use crate::Scope;
use crate::modifiers::layout::LayoutFrameExt;
use crate::primitives::controls::button::{ActionControlAccessibility, Button, ButtonIconSource};
use crate::theme::styles::buttons::{ButtonRole, ButtonStyleViewExt, GhostButtonStyle};
use crate::{AnyView, PrimitiveView, View, ViewExt};

/// Full sidebar or narrow rail presentation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum SidebarPresentation {
    #[default]
    Expanded,
    Rail {
        shows_labels: bool,
    },
}

impl SidebarPresentation {
    pub fn rail_width(self) -> Option<f32> {
        match self {
            Self::Expanded => None,
            Self::Rail { shows_labels } => Some(if shows_labels { 72.0 } else { 56.0 }),
        }
    }

    /// Canvas between the pane's edges and everything it lays out, so rows,
    /// section titles and the header and footer slots share one column. A rail
    /// spends its whole width on the row, which keeps its own inner padding.
    pub fn gutter(self) -> f32 {
        match self {
            Self::Expanded => 10.0,
            Self::Rail { .. } => 0.0,
        }
    }

    pub fn is_rail(self) -> bool {
        self.rail_width().is_some()
    }

    pub fn shows_labels(self) -> bool {
        match self {
            Self::Expanded => true,
            Self::Rail { shows_labels } => shows_labels,
        }
    }
}

/// Bounds for a caller-owned sidebar width. Layout fitting never writes a
/// narrower temporary allocation back into the caller's requested width.
#[derive(Clone, Debug, PartialEq)]
pub struct SidebarResizeOptions {
    pub minimum_width: f32,
    pub maximum_width: f32,
    pub minimum_content_width: f32,
    pub label: String,
}

impl Default for SidebarResizeOptions {
    fn default() -> Self {
        Self {
            minimum_width: 180.0,
            maximum_width: 480.0,
            minimum_content_width: 320.0,
            label: crate::i18n::tr("Resize navigation"),
        }
    }
}

impl SidebarResizeOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn validate(&mut self) {
        self.minimum_width = if self.minimum_width.is_finite() {
            self.minimum_width.max(44.0)
        } else {
            180.0
        };
        self.maximum_width = if self.maximum_width.is_finite() {
            self.maximum_width.max(self.minimum_width)
        } else {
            self.minimum_width.max(480.0)
        };
        self.minimum_content_width = if self.minimum_content_width.is_finite() {
            self.minimum_content_width.max(44.0)
        } else {
            320.0
        };
    }

    pub fn validated(mut self) -> Self {
        self.validate();
        self
    }

    /// Width range for the current container width and grab handle.
    pub fn range(&self, width: f32, handle: f32) -> (f32, f32) {
        let available = (width - handle - self.minimum_content_width).max(0.0);
        let low = self.minimum_width.min(available);
        let high = self.maximum_width.min(available).max(low);
        (low, high)
    }
}

/// The same affordance opens and collapses modal navigation at the leading edge.
pub struct SidebarToggle {
    pub label: String,
    pub is_expanded: bool,
    pub action: std::rc::Rc<dyn Fn()>,
}

impl SidebarToggle {
    pub fn new(label: impl Into<String>, is_expanded: bool, action: impl Fn() + 'static) -> Self {
        Self {
            label: label.into(),
            is_expanded,
            action: std::rc::Rc::new(action),
        }
    }
}

impl View for SidebarToggle {
    fn body(&self, _cx: &Scope) -> AnyView {
        AnyView::new(SidebarToggleBody {
            label: self.label.clone(),
            is_expanded: self.is_expanded,
            action: self.action.clone(),
        })
    }
}

/// Sizes the toggle and builds its button. The button host is the one node that
/// carries the accessible name, so the toggle is named exactly once.
struct SidebarToggleBody {
    label: String,
    is_expanded: bool,
    action: std::rc::Rc<dyn Fn()>,
}

impl PrimitiveView for SidebarToggleBody {
    fn make_node(&self) -> cove_core::scene::Node {
        let node = cove_core::scene::Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        None
    }

    fn update_node(&self, _node: &cove_core::scene::Node) {}

    fn children(&self, _node: &cove_core::scene::Node) -> Vec<AnyView> {
        vec![AnyView::new(SidebarToggleSized {
            label: self.label.clone(),
            is_expanded: self.is_expanded,
            action: self.action.clone(),
        })]
    }
}
crate::impl_primitive_view!(SidebarToggleBody);

/// Builds the ghost icon button and applies the 44-point touch floor to it.
struct SidebarToggleSized {
    label: String,
    is_expanded: bool,
    action: std::rc::Rc<dyn Fn()>,
}

impl PrimitiveView for SidebarToggleSized {
    fn make_node(&self) -> cove_core::scene::Node {
        let node = cove_core::scene::Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        None
    }

    fn update_node(&self, _node: &cove_core::scene::Node) {}

    fn children(&self, _node: &cove_core::scene::Node) -> Vec<AnyView> {
        let action = self.action.clone();
        let is_expanded = self.is_expanded;
        let label = self.label.clone();
        let styled = Button::icon_with_options(
            ButtonIconSource::resource(
                crate::primitives::media::ui_icon_catalog::layout::panel_left(),
            ),
            Some(16.0),
            None,
            ButtonRole::Normal,
            true,
            false,
            false,
            Some(label.clone()),
            move || action(),
        )
        .button_style(GhostButtonStyle)
        .composition_local(ActionControlAccessibility::label(), Some(label))
        .composition_local(ActionControlAccessibility::expanded(), Some(is_expanded))
        .frame(None, None, Some(44.0), Some(44.0), None, None);
        vec![AnyView::new(styled)]
    }
}
crate::impl_primitive_view!(SidebarToggleSized);
