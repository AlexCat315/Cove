//! Translation of Theme/Styles/Components/SelectionAppearance.swift.
use cove_core::scene::{LayoutNode, Node};
use cove_core::Color;

use crate::theme::theme_for_node;
use crate::{impl_primitive_view, AnyView, View};

/// One definition of "this row is selected", shared by every selectable
/// surface.
///
/// A style still chooses a *role* — that is what says why a surface looks the
/// way it does — but the colours and the geometry of each role live here and
/// nowhere else, so a new row style inherits the house selection instead of
/// inventing one.
///
/// Three roles, because the surfaces answer different questions:
///
/// - `Sidebar`: the iOS/macOS source list pill. A neutral grouped fill keeps
///   the selected destination quiet; the accent is reserved for controls.
/// - `Navigation`: an accent-tinted pill for tabs and compact navigation.
/// - `DataRow`: an edge-to-edge band, for dense grids whose rows share
///   horizontal rules with their neighbours and must keep column alignment.
///
/// Hover/press are state layers on top of the resting fill, so a selected row
/// still answers the pointer without turning into a second accent colour.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SelectionAppearance {
    /// Fill of the resting selection.
    pub fill: Color,
    pub hover_fill: Color,
    pub pressed_fill: Color,
    /// 0 paints an edge-to-edge band; anything else rounds the highlight.
    pub corner_radius: f32,
    /// Canvas the caller keeps clear on the leading and trailing edge, so the
    /// pill does not touch the row it belongs to.
    pub horizontal_inset: f32,
    /// Canvas kept clear above and below the highlight.
    pub vertical_inset: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SelectionRole {
    Sidebar,
    Navigation,
    DataRow,
}

impl SelectionAppearance {
    pub fn resolve(role: SelectionRole, theme: &crate::theme::Theme) -> Self {
        let colors = &theme.colors;
        let inset_role = role != SelectionRole::DataRow;
        Self {
            fill: if role == SelectionRole::Sidebar { colors.surface_variant } else { colors.selection },
            hover_fill: colors.state_layer_hover,
            pressed_fill: colors.state_layer_pressed,
            corner_radius: if inset_role { theme.radius.md } else { 0.0 },
            horizontal_inset: if inset_role { theme.spacing.sm } else { 0.0 },
            vertical_inset: if inset_role { theme.spacing.xs } else { 0.0 },
        }
    }

    /// The fill for one interaction state. Selection is a resting state, not a
    /// replacement for feedback: a selected row still answers hover and press.
    pub fn fill(&self, is_selected: bool, is_hovered: bool, is_pressed: bool) -> Color {
        if is_pressed {
            return if is_selected { self.pressed_fill.composited(self.fill) } else { self.pressed_fill };
        }
        if is_hovered {
            return if is_selected { self.hover_fill.composited(self.fill) } else { self.hover_fill };
        }
        if is_selected { self.fill } else { Color::CLEAR }
    }
}

/// Paints the shared selection behind this view.
///
/// Every selectable style routes its background through here, which is what
/// keeps a list row and a sidebar item from drifting apart. The insets are
/// exposed rather than applied: a surface that owns a full-height row needs the
/// highlight floated inside it, while a surface whose rows are their own
/// highlight only needs the colour and the radius.
pub trait SelectionHighlightViewExt: View + Sized + 'static {
    fn selection_highlight(
        self,
        role: SelectionRole,
        is_selected: bool,
        is_hovered: bool,
        is_pressed: bool,
    ) -> AnyView {
        AnyView::new(SelectionHighlight {
            content: AnyView::new(self),
            role,
            is_selected,
            is_hovered,
            is_pressed,
        })
    }
}
impl<T: View + Sized + 'static> SelectionHighlightViewExt for T {}

struct SelectionHighlight {
    content: AnyView,
    role: SelectionRole,
    is_selected: bool,
    is_hovered: bool,
    is_pressed: bool,
}

impl crate::PrimitiveView for SelectionHighlight {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn update_node(&self, node: &Node) {
        // `ThemeReader` in Swift resolves the highlight against the nearest
        // provider; `theme_for_node` is the Rust equivalent and keeps the
        // highlight tracking a theme swap without re-creating the view.
        let theme = theme_for_node(node);
        let appearance = SelectionAppearance::resolve(self.role, &theme);
        node.set_background_color(Some(appearance.fill(
            self.is_selected,
            self.is_hovered,
            self.is_pressed,
        )));
        node.set_corner_radius(appearance.corner_radius);
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        None
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![self.content.clone()]
    }
}

impl_primitive_view!(SelectionHighlight);
