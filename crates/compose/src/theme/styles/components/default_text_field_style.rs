//! Translation of Theme/Styles/Components/DefaultTextFieldStyle.swift.
//!
//! Recessed field chrome for dense desktop forms. The resting fill is a sunken
//! surface so inspector rows and toolbar fields read as inputs rather than as
//! generic cards.
use crate::modifiers::animation_value::AnimationValueExt;
use crate::modifiers::style::AppearanceExt;
use crate::theme::SemanticMotionRef;
use crate::{AnyView, ViewExt};
use cove_core::Color;
use cove_core::scene::Node;

use super::text_field_style::{TextFieldStyle, TextFieldStyleConfiguration};

#[derive(Clone, Copy, Debug, Default)]
pub struct DefaultTextFieldStyle;

impl TextFieldStyle for DefaultTextFieldStyle {
    fn make_body(&self, configuration: TextFieldStyleConfiguration) -> AnyView {
        let theme = configuration.theme.clone();
        let inputs = theme.inputs;
        let state = super::input_appearance::InputChromeState::from(&configuration);
        let background = inputs.background_for(state);
        let border = inputs.border_for(state);
        let border_width = inputs.border_width_for(state);
        let alpha = if configuration.is_enabled || theme.text_emphasis.disabled.is_some() {
            1.0
        } else {
            0.55
        };
        // Swift keys the implicit fast transition on this visual state, so a
        // focus or hover change interpolates the border instead of snapping.
        let visual_key = (background, border, border_width, alpha);
        AnyView::new(
            configuration
                .content
                .clone()
                .modifier(TextFieldChrome {
                    background,
                    border,
                    border_width,
                    radius: inputs.radius,
                })
                .opacity(alpha)
                .animation_motion(SemanticMotionRef::fast(), visual_key),
        )
    }
}

/// Animated field chrome driven by the shared semantic fast motion, mirroring
/// `.animation(.semantic(.fast, in: theme), value: visualKey)`.
struct TextFieldChrome {
    background: Color,
    border: Color,
    border_width: f32,
    radius: f32,
}

impl cove_composition::ViewModifier for TextFieldChrome {
    fn apply_node(&self, node: &Node) {
        let weak = node.downgrade();
        let background = self.background;
        node.animatable_set_optional(
            "textField.background",
            node.background_color(),
            Some(background),
            move |value| {
                if let Some(owner) = Node::upgrade(&weak) {
                    owner.set_background_color(value);
                }
            },
        );
        let weak = node.downgrade();
        let border = self.border;
        node.animatable_set_optional(
            "textField.borderColor",
            node.border_color(),
            Some(border),
            move |value| {
                if let Some(owner) = Node::upgrade(&weak) {
                    owner.set_border_color(value);
                }
            },
        );
        let weak = node.downgrade();
        let border_width = self.border_width;
        node.animatable_set_with(
            "textField.borderWidth",
            node.border_width(),
            border_width,
            move |value| {
                if let Some(owner) = Node::upgrade(&weak) {
                    owner.set_border_width(value);
                }
            },
        );
        let weak = node.downgrade();
        let radius = self.radius;
        node.animatable_set_with(
            "textField.cornerRadius",
            node.corner_radius(),
            radius,
            move |value| {
                if let Some(owner) = Node::upgrade(&weak) {
                    owner.set_corner_radius(value);
                }
            },
        );
    }
}
