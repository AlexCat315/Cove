//! Translation of Primitives/Controls/Fields/Numeric/PrefixStyle.swift.
//!
//! Field chrome for a numeric field with a prefix chip: one rounded input whose
//! leading addon slab is the scrub chip, separated by a hairline. The border,
//! hover and focus ring wrap chip and text together.
use crate::modifiers::animation_value::AnimationValueExt;
use crate::modifiers::layout::{FlexExt, LayoutFrameExt};
use crate::modifiers::style::AppearanceExt;
use crate::theme::SemanticMotionRef;
use crate::theme::styles::components::{TextFieldStyle, TextFieldStyleConfiguration};
use crate::{AnyView, BoxView, PrimitiveView, TupleView, View};
use cove_core::Color;
use cove_core::scene::{Align, FlexDirection, Justify, Node, Wrap};

/// Numeric field with a leading chip slab.
#[derive(Clone)]
pub struct NumericPrefixFieldStyle {
    pub chip: AnyView,
}

impl NumericPrefixFieldStyle {
    pub fn new(chip: impl View + 'static) -> Self {
        Self {
            chip: AnyView::new(chip),
        }
    }

    /// Border tone for the current interaction state.
    pub fn border_color(configuration: &TextFieldStyleConfiguration) -> Color {
        let inputs = &configuration.theme.inputs;
        if !configuration.is_enabled {
            inputs.border_disabled
        } else if configuration.is_error {
            inputs.border_error
        } else if configuration.is_focused {
            inputs.border_focused
        } else if configuration.is_hovered {
            inputs.border_hover
        } else {
            inputs.border_color
        }
    }
}

impl TextFieldStyle for NumericPrefixFieldStyle {
    fn make_body(&self, configuration: TextFieldStyleConfiguration) -> AnyView {
        let theme = configuration.theme.clone();
        let border = Self::border_color(&configuration);
        let width = if configuration.is_focused {
            theme.inputs.focus_ring_width
        } else {
            theme.inputs.border_width
        };
        let alpha = if configuration.is_enabled || theme.text_emphasis.disabled.is_some() {
            1.0
        } else {
            0.55
        };
        // Swift keys the implicit fast transition on the whole interaction
        // snapshot, including isEditing from IME composition.
        let key = configuration.interaction_key();
        AnyView::new(
            AnyView::new(NumericPrefixChrome {
                chip: self.chip.clone(),
                content: configuration.content.clone(),
                border,
                border_width: width,
                background: if configuration.is_enabled {
                    theme.inputs.background
                } else {
                    theme.inputs.background_disabled
                },
                divider: theme.inputs.divider_color,
                radius: theme.inputs.radius,
                alpha,
            })
            .animation_motion(SemanticMotionRef::fast(), key),
        )
    }
}

/// Chip slab, hairline and text laid out as one bordered input.
struct NumericPrefixChrome {
    chip: AnyView,
    content: AnyView,
    border: Color,
    border_width: f32,
    background: Color,
    divider: Color,
    radius: f32,
    alpha: f32,
}

impl PrimitiveView for NumericPrefixChrome {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node.set_clips_to_bounds(true);
        node
    }

    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        let layout = cove_core::scene::LayoutNode::new();
        layout.set_flex_direction(FlexDirection::Column);
        layout.set_align_items(Align::Stretch);
        Some(layout)
    }

    fn update_node(&self, node: &Node) {
        let weak = node.downgrade();
        let background = self.background;
        node.animatable_set_optional(
            "numericPrefix.background",
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
            "numericPrefix.borderColor",
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
            "numericPrefix.borderWidth",
            node.border_width(),
            border_width,
            move |value| {
                if let Some(owner) = Node::upgrade(&weak) {
                    owner.set_border_width(value);
                }
            },
        );
        node.set_corner_radius(self.radius);
        node.set_opacity(self.alpha);
        node.set_clips_to_bounds(true);
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![AnyView::new(
            BoxView::new(
                FlexDirection::Row,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                AnyView::new(TupleView::new(vec![
                    AnyView::new(self.chip.clone().flex_with(0.0, 0.0, None)),
                    AnyView::new(
                        BoxView::new(
                            FlexDirection::Column,
                            Align::Stretch,
                            Justify::FlexStart,
                            Wrap::NoWrap,
                            0.0,
                            AnyView::new(crate::EmptyView),
                        )
                        .frame_width(1.0)
                        .background_color(self.divider)
                        .flex_with(0.0, 0.0, None),
                    ),
                    AnyView::new(
                        self.content
                            .clone()
                            .flex_with(1.0, 1.0, Some(0.0))
                            .frame_min_width(0.0),
                    ),
                ])),
            )
            .flex_with(1.0, 1.0, None),
        )]
    }
}
crate::impl_primitive_view!(NumericPrefixChrome);
