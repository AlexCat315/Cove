//! Translation of Theme/Styles/Components/DefaultListRowStyle.swift.
//!
//! Grouped-list row chrome. Rows paint a full-height selection band, a neutral
//! hover face, and an inset hairline between neighbours. The host owns the
//! fixed row slot so oversized content cannot expand the hit region into a
//! neighbouring row.
use crate::modifiers::layout::{FlexExt, LayoutFrameExt, PaddingExt};
use crate::modifiers::style::{AppearanceExt, SemanticStyleViewExt};
use crate::{AnyView, Row, VerticalAlignment, ViewExt};
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
        // macOS selection: the accent band with light ink while the list has
        // keyboard focus, a neutral band with the row's own ink otherwise.
        // Hover is painted live by the row host in the same band geometry, so
        // the style never recomposes for it.
        let on_accent = configuration.is_selected && configuration.is_emphasized;
        let fill = if !configuration.is_selected {
            Color::CLEAR
        } else if configuration.is_emphasized {
            colors.accent
        } else {
            colors.surface_pressed
        };
        // Pointer highlights float inside the row as a rounded band; touch
        // keeps the edge-to-edge band with a square edge.
        let radius = if touch { 0.0 } else { theme.radius.md };
        let alpha = if configuration.is_enabled { 1.0 } else { 0.55 };
        let edge = if touch { 16.0 } else { theme.spacing.lg };
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
        // Only the accent band re-inks its label; other rows keep the default
        // ink, so a theme cross-fade doesn't animate a redundant color.
        let context = super::list_row_style::ListRowContext {
            is_selected: configuration.is_selected,
            is_emphasized: configuration.is_emphasized,
            is_enabled: configuration.is_enabled,
        };
        let row = AnyView::new(
            row.composition_local(super::list_row_style::ListRowContext::key(), context),
        );
        let selected_ink = if on_accent {
            AnyView::new(row.semantic_foreground(crate::theme::SemanticColorRef::on_accent()))
        } else {
            row
        };
        let result = selected_ink.padding_edges(edge, if touch { 0.0 } else { theme.spacing.xs });
        let separator = ListRowSeparatorModifier {
            color: theme.colors.divider,
            is_enabled: configuration.shows_separator && !configuration.is_selected,
            inset: configuration.separator_inset.unwrap_or(edge),
        };
        AnyView::new(
            result
                .modifier(separator)
                .opacity(alpha)
                .modifier(AnimatedRowFill {
                    fill,
                    on_accent,
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
    inset: f32,
}

impl cove_composition::ViewModifier for ListRowSeparatorModifier {
    fn apply_node(&self, node: &Node) {
        if !self.is_enabled {
            // A recycled row may still carry the hairline it drew before.
            if node.draw().is_some() {
                node.set_draw(None);
            }
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
        let inset = self.inset;
        let weak = node.downgrade();
        node.update_draw(rtl, move |list, origin| {
            let Some(owner) = Node::upgrade(&weak) else {
                return;
            };
            let frame = owner.frame();
            let left = if rtl { 0.0 } else { inset };
            // Inset at both ends, so the hairline lines up with the rounded
            // selection band rather than running past it.
            let _ = left;
            let rect = Rect::new(
                origin.x + inset,
                origin.y + frame.height - 0.5,
                (frame.width - inset * 2.0).max(0.0),
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
    on_accent: bool,
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
        node.set_foreground_color(
            self.on_accent
                .then(|| crate::theme::theme_for_node(node).colors.on_accent),
        );
    }
}

/// Edge-to-edge row chrome for containers that already paint their own card
/// (tables, grids). No inset box, no row padding: the content keeps the column
/// geometry the header was laid out with, and the separator runs the full row.
#[derive(Clone, Copy, Debug, Default)]
pub struct FlushListRowStyle;

impl ListRowStyle for FlushListRowStyle {
    fn make_body(&self, configuration: ListRowStyleConfiguration) -> AnyView {
        let theme = configuration.theme.clone();
        let on_accent = configuration.is_selected && configuration.is_emphasized;
        let fill = if !configuration.is_selected {
            Color::CLEAR
        } else if configuration.is_emphasized {
            theme.colors.accent
        } else {
            theme.colors.surface_pressed
        };
        let alpha = if configuration.is_enabled { 1.0 } else { 0.55 };
        let row = AnyView::new(Row::new(
            VerticalAlignment::Center,
            0.0,
            AnyView::new(
                configuration
                    .content
                    .clone()
                    .frame_min_width(0.0)
                    .flex_with(1.0, 1.0, Some(0.0)),
            ),
        )
        .frame_height(configuration.row_height));
        let context = super::list_row_style::ListRowContext {
            is_selected: configuration.is_selected,
            is_emphasized: configuration.is_emphasized,
            is_enabled: configuration.is_enabled,
        };
        let row = AnyView::new(
            row.composition_local(super::list_row_style::ListRowContext::key(), context),
        );
        let row = if on_accent {
            AnyView::new(row.semantic_foreground(crate::theme::SemanticColorRef::on_accent()))
        } else {
            row
        };
        let separator = ListRowSeparatorModifier {
            color: theme.colors.divider,
            is_enabled: configuration.shows_separator && !configuration.is_selected,
            inset: configuration.separator_inset.unwrap_or(0.0),
        };
        AnyView::new(
            row.modifier(separator)
                .opacity(alpha)
                .modifier(AnimatedRowFill {
                    fill,
                    on_accent,
                    radius: 0.0,
                }),
        )
    }
}
