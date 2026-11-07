//! Translation of Theme/Styles/Components/DefaultTextFieldStyle.swift.
//!
//! macOS field chrome: a white well with a hairline, and a soft accent halo
//! outside the border while the field has focus.
use crate::modifiers::animation_value::AnimationValueExt;
use crate::modifiers::style::AppearanceExt;
use crate::theme::SemanticMotionRef;
use crate::{AnyView, ViewExt};
use cove_core::scene::Node;
use cove_core::{Color, SurfaceShadow};

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
        let glow = focus_halo(&theme, state.is_focused && configuration.is_enabled);
        let visual_key = (background, border, border_width, alpha, glow.blur.to_bits());
        AnyView::new(
            configuration
                .content
                .clone()
                .modifier(TextFieldChrome {
                    background,
                    border,
                    border_width,
                    radius: inputs.radius,
                    glow,
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
    glow: SurfaceShadow,
}

/// The macOS focus ring: a 4pt accent glow hugging the field's outline.
pub fn focus_halo(theme: &crate::theme::Theme, is_focused: bool) -> SurfaceShadow {
    SurfaceShadow {
        color: Some(if is_focused {
            theme.colors.focus_ring
        } else {
            theme.colors.focus_ring.multiplied_alpha(0.0)
        }),
        offset_x: 0.0,
        offset_y: 0.0,
        blur: 4.0,
    }
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
        let glow = self.glow;
        let current = node.shadow().color.unwrap_or(Color::CLEAR);
        node.animatable_set_optional(
            "textField.focusHalo",
            Some(current),
            glow.color,
            move |value| {
                if let Some(owner) = Node::upgrade(&weak) {
                    owner.set_shadow(SurfaceShadow {
                        color: value,
                        ..glow
                    });
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
