//! Translation of Theme/Styles/Buttons/StatusButtonStyle.swift.
use cove_core::Color;
use cove_core::scene::{Align, Edge, FlexDirection, LayoutNode, Node};

use crate::primitives::feedback::StatusTone;
use crate::theme::{ControlSize, SemanticColorRef, SemanticFontRef, SemanticStyleViewExt, Theme};
use crate::{AnyView, impl_primitive_view};

use super::button_style::{ButtonStyle, ButtonStyleConfiguration};

/// Tinted semantic actions. Uses status tokens for legible text instead of
/// assuming white ink.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StatusButtonStyle {
    pub tone: StatusTone,
}
impl StatusButtonStyle {
    pub fn new(tone: StatusTone) -> Self {
        Self { tone }
    }
}

impl ButtonStyle for StatusButtonStyle {
    fn make_body(&self, c: ButtonStyleConfiguration) -> AnyView {
        AnyView::new(StatusChrome {
            label: c.label.clone(),
            theme: c.theme.clone(),
            control_size: c.control_size,
            is_icon_only: c.is_icon_only,
            tone: self.tone,
            is_pressed: c.is_pressed,
            is_hovered: c.is_hovered,
            is_focused: c.is_focused,
            is_enabled: c.is_enabled,
        })
    }

    /// Swift's `Hashable` conformance includes the tone. `StatusTone` is a
    /// field-less enum, so its discriminant is a stable style identity.
    fn style_identity(&self) -> u64 {
        self.tone as u64
    }
}

struct StatusChrome {
    label: AnyView,
    theme: Theme,
    control_size: ControlSize,
    is_icon_only: bool,
    tone: StatusTone,
    is_pressed: bool,
    is_hovered: bool,
    is_focused: bool,
    is_enabled: bool,
}

impl crate::PrimitiveView for StatusChrome {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn update_node(&self, node: &Node) {
        // A tinted wash in the tone's hue, without an outline: the hue alone
        // marks the meaning, and an opaque pastel block would read heavier
        // than primary.
        let accent = self.tone.color().resolve(&self.theme);
        let background = if !self.is_enabled {
            self.theme.colors.surface_sunken
        } else if self.is_pressed {
            accent
                .multiplied_alpha(0.24)
                .composited(self.theme.colors.state_layer_pressed)
        } else if self.is_hovered {
            accent
                .multiplied_alpha(0.18)
                .composited(self.theme.colors.state_layer_hover)
        } else {
            accent.multiplied_alpha(0.12)
        };
        let border = if self.is_focused {
            self.theme.colors.focus_ring
        } else {
            Color::CLEAR
        };
        let border_width = if self.is_focused { 2.0 } else { 0.0 };

        node.set_background_color(Some(background));
        node.set_border_color(Some(border));
        node.set_border_width(border_width);
        node.set_corner_radius(self.theme.radius.md);
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        Some(LayoutNode::new())
    }

    fn update_layout(&self, layout: &LayoutNode) {
        let vertical_padding =
            ((self.control_size.button_height() - self.theme.typography.label.line_height) * 0.5)
                .max(2.0);
        layout.set_flex_direction(FlexDirection::Row);
        layout.set_align_items(Align::Center);
        layout.set_flex_grow(1.0);
        layout.set_flex_shrink(1.0);
        let horizontal = if self.is_icon_only {
            vertical_padding
        } else {
            self.control_size.horizontal_padding()
        };
        layout.set_padding(horizontal, Edge::Start);
        layout.set_padding(horizontal, Edge::End);
        layout.set_padding(vertical_padding, Edge::Top);
        layout.set_padding(vertical_padding, Edge::Bottom);
        layout.set_min_width(Some(if self.is_icon_only {
            self.control_size.button_height()
        } else {
            0.0
        }));
        layout.set_min_height(Some(self.control_size.button_height()));
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        let foreground = if self.is_enabled {
            self.tone.color()
        } else {
            SemanticColorRef::on_surface_disabled()
        };
        let label = AnyView::new(self.label.clone())
            .semantic_font(SemanticFontRef::label())
            .semantic_foreground(foreground);
        vec![AnyView::new(label)]
    }
}

impl_primitive_view!(StatusChrome);
