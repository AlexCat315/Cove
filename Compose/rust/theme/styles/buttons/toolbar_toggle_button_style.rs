//! Translation of Theme/Styles/Buttons/ToolbarToggleButtonStyle.swift.
use cove_core::scene::{with_animation, Align, Animation, Edge, FlexDirection, Justify, LayoutNode, Node};
use cove_core::Color;

use crate::theme::{SemanticColorRef, SemanticFontRef, SemanticStyleViewExt, Theme};
use crate::{impl_primitive_view, AnyView};

use super::button_style::{ButtonInteractionKey, ButtonStyle, ButtonStyleConfiguration};

/// Quiet selection for dense toolbars; primary actions retain solid fills.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ToolbarToggleButtonStyle {
    pub height: f32,
    pub min_width: f32,
}
impl ToolbarToggleButtonStyle {
    pub fn new() -> Self {
        Self { height: 28.0, min_width: 28.0 }
    }
    /// Swift's `init(_ configure: (inout Self) -> Void)`.
    pub fn configured(configure: impl FnOnce(&mut Self)) -> Self {
        let mut style = Self::new();
        configure(&mut style);
        style
    }
}
impl Default for ToolbarToggleButtonStyle {
    fn default() -> Self {
        Self::new()
    }
}

impl ButtonStyle for ToolbarToggleButtonStyle {
    fn make_body(&self, configuration: ButtonStyleConfiguration) -> AnyView {
        AnyView::new(ToolbarToggleChrome {
            label: configuration.label.clone(),
            theme: configuration.theme.clone(),
            height: self.height,
            min_width: self.min_width,
            is_pressed: configuration.is_pressed,
            is_hovered: configuration.is_hovered,
            is_focused: configuration.is_focused,
            is_selected: configuration.is_selected,
            is_enabled: configuration.is_enabled,
            interaction: configuration.interaction_key(),
        })
    }

    fn style_identity(&self) -> u64 {
        let mut hash = 0xcbf2_9ce4_8422_2325u64;
        for bits in [self.height.to_bits(), self.min_width.to_bits()] {
            hash ^= bits as u64;
            hash = hash.wrapping_mul(0x0000_0100_0000_01B3);
        }
        hash
    }
}

const TOOLBAR_TOGGLE_KEY: &str = "toolbarToggle.interaction";

struct ToolbarToggleChrome {
    label: AnyView,
    theme: Theme,
    height: f32,
    min_width: f32,
    is_pressed: bool,
    is_hovered: bool,
    is_focused: bool,
    is_selected: bool,
    is_enabled: bool,
    interaction: ButtonInteractionKey,
}

impl crate::PrimitiveView for ToolbarToggleChrome {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn update_node(&self, node: &Node) {
        let colors = &self.theme.colors;
        let fill = if self.is_pressed {
            colors.state_layer_pressed
        } else if self.is_hovered {
            colors.state_layer_hover
        } else if self.is_selected {
            colors.surface_variant
        } else {
            Color::CLEAR
        };
        let border = if self.is_focused { colors.focus_ring } else { Color::CLEAR };
        let border_width = if self.is_focused { 1.0 } else { 0.0 };
        let animate = node
            .attachment::<ButtonInteractionKey>(TOOLBAR_TOGGLE_KEY)
            .is_some_and(|previous| previous != self.interaction);

        let apply = || {
            let owner = node.clone();
            let current = node.background_color();
            node.animatable_set_optional("toolbarToggle.backgroundColor", current, Some(fill), move |value| {
                owner.set_background_color(value);
            });
            let owner = node.clone();
            let current = node.border_color();
            node.animatable_set_optional("toolbarToggle.borderColor", current, Some(border), move |value| {
                owner.set_border_color(value);
            });
            let owner = node.clone();
            let current = node.border_width();
            node.animatable_set_with("toolbarToggle.borderWidth", current, border_width, move |value| {
                owner.set_border_width(value);
            });
            node.set_corner_radius(self.theme.radius.sm);
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
        node.set_attachment(TOOLBAR_TOGGLE_KEY, self.interaction);
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        Some(LayoutNode::new())
    }

    fn update_layout(&self, layout: &LayoutNode) {
        layout.set_flex_direction(FlexDirection::Row);
        layout.set_align_items(Align::Center);
        layout.set_justify_content(Justify::Center);
        layout.set_padding(6.0, Edge::Start);
        layout.set_padding(6.0, Edge::End);
        layout.set_padding(4.0, Edge::Top);
        layout.set_padding(4.0, Edge::Bottom);
        layout.set_min_width(Some(self.min_width));
        layout.set_min_height(Some(self.height));
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        let foreground = if !self.is_enabled {
            SemanticColorRef::on_surface_disabled()
        } else if self.is_selected {
            SemanticColorRef::accent()
        } else {
            SemanticColorRef::on_surface_variant()
        };
        let label = AnyView::new(self.label.clone())
            .semantic_font(SemanticFontRef::label())
            .semantic_foreground(foreground);
        vec![AnyView::new(label)]
    }
}

impl_primitive_view!(ToolbarToggleChrome);
