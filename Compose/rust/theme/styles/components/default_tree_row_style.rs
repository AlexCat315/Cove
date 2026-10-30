//! Translation of Theme/Styles/Components/DefaultTreeRowStyle.swift.
//!
//! Tree row default: same density and hierarchy cues as the list style, with
//! enough selection contrast to read as an outline tree in an editor shell. The
//! navigation highlight leaves the hierarchy gutter clear; subtle vertical guides
//! carry depth without connecting horizontal strokes to the row chrome.
use cove_core::scene::Node;
use cove_core::Color;
use crate::modifiers::layout::{FlexExt, PaddingExt};
use crate::modifiers::style::{AppearanceExt, SemanticStyleViewExt};
use crate::theme::SemanticColorRef;
use crate::{AnyView, Row, Spacer, TupleView, VerticalAlignment, View, ViewExt};

use super::tree_row_style::{TreeRowStyle, TreeRowStyleConfiguration};

#[derive(Clone, Copy, Debug, Default)]
pub struct DefaultTreeRowStyle;

impl TreeRowStyle for DefaultTreeRowStyle {
    fn make_body(&self, configuration: TreeRowStyleConfiguration) -> AnyView {
        let theme = configuration.theme.clone();
        let colors = theme.colors;
        // A selected row is a solid accent band with a light label, matching the
        // reference; an unselected hover is a neutral surface face.
        let background = if configuration.is_selected {
            if configuration.is_hovered { colors.accent_hover } else { colors.accent }
        } else if configuration.is_search_hit {
            colors.state_layer_selected
        } else if configuration.is_hovered {
            colors.surface_variant
        } else {
            Color::CLEAR
        };
        let alpha = if configuration.is_enabled { 1.0 } else { 0.55 };
        let content = AnyView::new(
            Row::new(
                VerticalAlignment::Center,
                theme.spacing.sm,
                AnyView::new(TupleView::new(vec![
                    configuration.content.clone(),
                    AnyView::new(Spacer::new(0.0)),
                ])),
            )
            .flex(1.0)
            .padding_edges(theme.spacing.sm + 1.0, theme.spacing.xs + 1.0),
        );
        let outer = AnyView::new(Row::new(
            VerticalAlignment::Center,
            0.0,
            AnyView::new(content),
        ));
        // Only the selected band re-inks its label; unselected rows keep the
        // default ink, so a theme cross-fade doesn't animate a redundant color.
        let inked = if configuration.is_selected {
            AnyView::new(outer.semantic_foreground(SemanticColorRef::on_accent()))
        } else {
            outer
        };
        AnyView::new(
            inked
                .modifier(TreeRowFill { background })
                .corner_radius(theme.radius.md)
                .opacity(alpha),
        )
    }
}

/// Animated selection/search/hover band driven by the shared semantic fast
/// motion, mirroring `.animation(.semantic(.fast, in: t), value: visualKey)`.
struct TreeRowFill {
    background: Color,
}

impl cove_composition::ViewModifier for TreeRowFill {
    fn apply_node(&self, node: &Node) {
        let background = self.background;
        let weak = node.downgrade();
        node.animatable_set_optional("treeRow.background", node.background_color(), Some(background), move |value| {
            if let Some(owner) = Node::upgrade(&weak) {
                owner.set_background_color(value);
            }
        });
    }
}
