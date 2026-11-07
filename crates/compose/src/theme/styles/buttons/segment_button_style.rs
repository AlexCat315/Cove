//! Translation of Theme/Styles/Buttons/SegmentButtonStyle.swift.
use cove_core::scene::{Align, Animation, Edge, FlexDirection, LayoutNode, Node, with_animation};
use cove_core::{Color, SurfaceShadow};

use crate::animation::geometry::{SlidingSelectionMark, is_in_sliding_selection};
use crate::theme::{ControlSize, SemanticColorRef, SemanticFontRef, SemanticStyleViewExt, Theme};
use crate::{AnyView, ViewModifier, impl_primitive_view};

use super::button_style::{ButtonInteractionKey, ButtonStyle, ButtonStyleConfiguration};

/// A segmented selection uses one raised thumb inside the group's shared well.
/// Keep action-button emphasis for commands elsewhere in the interface.
#[derive(Clone, Copy, Debug, Default)]
pub struct SegmentButtonStyle;

impl ButtonStyle for SegmentButtonStyle {
    fn make_body(&self, c: ButtonStyleConfiguration) -> AnyView {
        AnyView::new(SegmentChrome {
            label: c.label.clone(),
            theme: c.theme.clone(),
            control_size: c.control_size,
            is_selected: c.is_selected,
            is_hovered: c.is_hovered,
            is_pressed: c.is_pressed,
            is_focused: c.is_focused,
            is_enabled: c.is_enabled,
            interaction: c.interaction_key(),
        })
    }
}

const SEGMENT_KEY: &str = "segment.interaction";

struct SegmentChrome {
    label: AnyView,
    theme: Theme,
    control_size: ControlSize,
    is_selected: bool,
    is_hovered: bool,
    is_pressed: bool,
    is_focused: bool,
    is_enabled: bool,
    interaction: ButtonInteractionKey,
}

impl SegmentChrome {
    fn fill(&self, paints_selection: bool) -> Color {
        let base = if paints_selection {
            self.theme.colors.surface_raised
        } else {
            Color::CLEAR
        };
        if !self.is_enabled {
            base
        } else if self.is_pressed {
            base.composited(self.theme.colors.state_layer_pressed)
        } else if self.is_hovered {
            base.composited(self.theme.colors.state_layer_hover)
        } else {
            base
        }
    }
}

impl crate::PrimitiveView for SegmentChrome {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn update_node(&self, node: &Node) {
        // Inside a group the thumb is one surface that springs between
        // segments; a segment paints only its own hover and press.
        let slides = self.is_selected && is_in_sliding_selection(node);
        SlidingSelectionMark(slides).apply_node(node);
        let fill = self.fill(self.is_selected && !slides);
        let border = if self.is_focused {
            self.theme.colors.focus_ring
        } else {
            Color::CLEAR
        };
        let border_width = if self.is_focused { 2.0 } else { 0.0 };
        let radius = self.theme.radius.pill;
        let shadow = if self.is_selected && !slides {
            SurfaceShadow {
                color: Some(Color {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                    a: 0.12,
                }),
                offset_x: 0.0,
                offset_y: 1.0,
                blur: 2.0,
            }
        } else {
            SurfaceShadow::NONE
        };
        let animate = node
            .attachment::<ButtonInteractionKey>(SEGMENT_KEY)
            .is_some_and(|previous| previous != self.interaction);

        let apply = || {
            let owner = node.clone();
            let current = node.background_color();
            node.animatable_set_optional(
                "segment.backgroundColor",
                current,
                Some(fill),
                move |value| {
                    owner.set_background_color(value);
                },
            );
            let owner = node.clone();
            let current = node.border_color();
            node.animatable_set_optional(
                "segment.borderColor",
                current,
                Some(border),
                move |value| {
                    owner.set_border_color(value);
                },
            );
            let owner = node.clone();
            let current = node.border_width();
            node.animatable_set_with("segment.borderWidth", current, border_width, move |value| {
                owner.set_border_width(value);
            });
            let owner = node.clone();
            let current = node.corner_radius();
            node.animatable_set_with("segment.cornerRadius", current, radius, move |value| {
                owner.set_corner_radius(value);
            });
            node.set_shadow(shadow);
        };

        if animate {
            with_animation(
                Some(Animation::new(
                    self.theme.motion.fast,
                    self.theme.motion.standard_easing.as_animation_curve(),
                )),
                apply,
            );
        } else {
            apply();
        }
        node.set_attachment(SEGMENT_KEY, self.interaction);
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        Some(LayoutNode::new())
    }

    fn update_layout(&self, layout: &LayoutNode) {
        layout.set_flex_direction(FlexDirection::Row);
        layout.set_align_items(Align::Center);
        layout.set_padding(self.control_size.horizontal_padding(), Edge::Start);
        layout.set_padding(self.control_size.horizontal_padding(), Edge::End);
        layout.set_padding(4.0, Edge::Top);
        layout.set_padding(4.0, Edge::Bottom);
        layout.set_min_height(Some(20.0f32.max(self.control_size.button_height() - 6.0)));
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        let foreground = if !self.is_enabled {
            SemanticColorRef::on_surface_disabled()
        } else if self.is_selected {
            SemanticColorRef::on_surface()
        } else {
            SemanticColorRef::on_surface_variant()
        };
        let label = AnyView::new(self.label.clone())
            .semantic_font(SemanticFontRef::label())
            .semantic_foreground(foreground);
        vec![AnyView::new(label)]
    }
}

impl_primitive_view!(SegmentChrome);
