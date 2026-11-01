//! Translation of Theme/Styles/Components/DefaultListRowStyle.swift.
//!
//! Pointer rows use the shared navigation highlight; touch rows use a
//! full-width band with an inset separator. The host owns the fixed row slot so
//! oversized content cannot expand the hit region into a neighbouring row.
use crate::modifiers::interaction::AccessibilityExt;
use crate::modifiers::layout::{FlexExt, LayoutFrameExt, PaddingExt};
use crate::modifiers::style::{AppearanceExt, SemanticStyleViewExt};
use crate::{AnyView, Row, VerticalAlignment, View, ViewExt};
use cove_core::scene::{Direction, Node};
use cove_core::{Color, InputCapabilities, Rect};

use super::list_row_style::{ListRowStyle, ListRowStyleConfiguration};

/// Pointer rows use the shared navigation highlight; touch rows use a
/// full-width band with an inset separator.
#[derive(Clone, Copy, Debug, Default)]
pub struct DefaultListRowStyle;

impl ListRowStyle for DefaultListRowStyle {
    fn make_body(&self, configuration: ListRowStyleConfiguration) -> AnyView {
        let theme = configuration.theme.clone();
        let touch = configuration.input.contains(InputCapabilities::TOUCH);
        let colors = theme.colors;
        // A selected row is a solid accent band with a light label, like the
        // reference; an unselected hover is a neutral surface face.
        let fill = if configuration.is_selected {
            if configuration.is_hovered {
                colors.accent_hover
            } else {
                colors.accent
            }
        } else if configuration.is_hovered {
            colors.surface_variant
        } else {
            Color::CLEAR
        };
        let radius = if touch { 0.0 } else { theme.radius.md };
        let alpha = if configuration.is_enabled { 1.0 } else { 0.55 };
        let inner_height =
            (configuration.row_height - 2.0 * if touch { 0.0 } else { theme.spacing.xs }).max(0.0);
        let row = AnyView::new(
            Row::new(
                VerticalAlignment::Center,
                theme.spacing.sm,
                AnyView::new(
                    configuration
                        .content
                        .clone()
                        .frame_min_width(0.0)
                        .flex_with(1.0, 1.0, Some(0.0)),
                ),
            )
            .frame_height(inner_height),
        );
        // Only the selected band re-inks its label; unselected rows keep the
        // default ink, so a theme cross-fade doesn't animate a redundant color.
        let selected_ink = if configuration.is_selected {
            AnyView::new(row.semantic_foreground(crate::theme::SemanticColorRef::on_accent()))
        } else {
            row
        };
        let result = selected_ink.padding_edges(if touch { 16.0 } else { theme.spacing.sm }, 0.0);
        let separator = ListRowSeparatorModifier {
            color: theme.colors.divider,
            is_enabled: touch,
        };
        AnyView::new(
            result
                .modifier(separator)
                .opacity(alpha)
                .modifier(AnimatedRowFill {
                    fill,
                    is_selected: configuration.is_selected,
                    radius,
                }),
        )
    }
}

/// Hairline separator for touch rows: it marks the row edge without drawing a
/// rule.
struct ListRowSeparatorModifier {
    color: Color,
    is_enabled: bool,
}

impl cove_composition::ViewModifier for ListRowSeparatorModifier {
    fn apply_node(&self, node: &Node) {
        if !self.is_enabled {
            return;
        }
        let mut parent = Some(node.clone());
        let mut rtl = false;
        while let Some(current) = parent {
            if let Some(layout) = current.layout_node() {
                let direction = layout.direction();
                if direction != Direction::Inherit {
                    rtl = direction == Direction::Rtl;
                    break;
                }
            }
            parent = current.parent();
        }
        let color = self.color;
        let weak = node.downgrade();
        node.update_draw(rtl, move |list, origin| {
            let Some(owner) = Node::upgrade(&weak) else {
                return;
            };
            let frame = owner.frame();
            let left = if rtl { 0.0 } else { 16.0 };
            let rect = Rect::new(
                origin.x + left,
                origin.y + frame.height - 0.5,
                (frame.width - 16.0).max(0.0),
                0.5,
            );
            let _ = list.add_rect(rect, color);
        });
    }
}

/// Interaction-state fill that animates through the shared semantic fast
/// motion, mirroring Swift's `.animation(.semantic(.fast, in: theme), value:)`.
struct AnimatedRowFill {
    fill: Color,
    is_selected: bool,
    radius: f32,
}

impl cove_composition::ViewModifier for AnimatedRowFill {
    fn apply_node(&self, node: &Node) {
        let fill = self.fill;
        let weak = node.downgrade();
        node.animatable_set_optional(
            "listRow.fill",
            node.background_color(),
            Some(fill),
            move |value| {
                if let Some(owner) = Node::upgrade(&weak) {
                    owner.set_background_color(value);
                }
            },
        );
        let weak = node.downgrade();
        let radius = self.radius;
        node.animatable_set_with(
            "listRow.radius",
            node.corner_radius(),
            radius,
            move |value| {
                if let Some(owner) = Node::upgrade(&weak) {
                    owner.set_corner_radius(value);
                }
            },
        );
        if self.is_selected {
            node.set_foreground_color(Some(crate::theme::theme_for_node(node).colors.on_accent));
        }
    }
}
