//! Translation of Theme/Styles/Buttons/TabButtonStyle.swift.
use cove_core::Color;
use cove_core::scene::{
    Align, Animation, Edge, FlexDirection, Justify, LayoutNode, Node, with_animation,
};

use crate::animation::geometry::{SlidingSelectionMark, is_in_sliding_selection};
use crate::theme::styles::components::selection_appearance::{SelectionAppearance, SelectionRole};
use crate::theme::{SemanticColorRef, SemanticFontRef, SemanticStyleViewExt, Theme};
use crate::{AnyView, ViewModifier, impl_primitive_view};

use super::button_style::{ButtonInteractionKey, ButtonStyle, ButtonStyleConfiguration};

/// IDE tab with the shared selection pill and full keyboard/hover focus states.
/// The same style can be used for document tabs and tool-panel tabs.
///
/// The tab takes its fill, radius and insets from `SelectionAppearance` rather
/// than composing its own, so a tab strip, a list row and a sidebar destination
/// are the same selection three times over. A hard-edged full-bleed band read
/// as a box sitting *in* the header instead of a tab that belongs to it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TabButtonStyle {
    pub height: f32,
}
impl TabButtonStyle {
    pub fn new(height: f32) -> Self {
        Self { height }
    }
}
impl Default for TabButtonStyle {
    fn default() -> Self {
        Self::new(28.0)
    }
}

impl ButtonStyle for TabButtonStyle {
    fn make_body(&self, configuration: ButtonStyleConfiguration) -> AnyView {
        let theme = configuration.theme.clone();
        let appearance = SelectionAppearance::resolve(SelectionRole::Navigation, &theme);
        // The indicator rides the bottom of the strip, so the pill keeps a
        // little more room below than above.
        let top = appearance.vertical_inset;
        let bottom = appearance.vertical_inset + 2.0;
        AnyView::new(TabChrome {
            label: configuration.label.clone(),
            theme,
            height: self.height,
            appearance,
            top,
            bottom,
            is_selected: configuration.is_selected,
            is_hovered: configuration.is_hovered,
            is_pressed: configuration.is_pressed,
            is_focused: configuration.is_focused,
            is_enabled: configuration.is_enabled,
            interaction: configuration.interaction_key(),
        })
    }

    fn style_identity(&self) -> u64 {
        self.height.to_bits() as u64
    }
}

const TAB_KEY: &str = "tab.interaction";

struct TabChrome {
    label: AnyView,
    theme: Theme,
    height: f32,
    appearance: SelectionAppearance,
    top: f32,
    bottom: f32,
    is_selected: bool,
    is_hovered: bool,
    is_pressed: bool,
    is_focused: bool,
    is_enabled: bool,
    interaction: ButtonInteractionKey,
}

impl crate::PrimitiveView for TabChrome {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn update_node(&self, node: &Node) {
        // Inside a strip the resting selection is the strip's sliding pill;
        // the tab paints only its hover and press.
        let slides = self.is_selected && is_in_sliding_selection(node);
        SlidingSelectionMark(slides).apply_node(node);
        let fill = self.appearance.fill(
            self.is_selected && !slides,
            self.is_hovered,
            self.is_pressed,
        );
        let border = if self.is_focused {
            self.theme.colors.focus_ring
        } else {
            Color::CLEAR
        };
        let border_width = if self.is_focused { 1.0 } else { 0.0 };
        let animate = node
            .attachment::<ButtonInteractionKey>(TAB_KEY)
            .is_some_and(|previous| previous != self.interaction);

        let apply = || {
            let owner = node.clone();
            let current = node.background_color();
            node.animatable_set_optional(
                "tab.backgroundColor",
                current,
                Some(fill),
                move |value| {
                    owner.set_background_color(value);
                },
            );
            let owner = node.clone();
            let current = node.border_color();
            node.animatable_set_optional("tab.borderColor", current, Some(border), move |value| {
                owner.set_border_color(value);
            });
            let owner = node.clone();
            let current = node.border_width();
            node.animatable_set_with("tab.borderWidth", current, border_width, move |value| {
                owner.set_border_width(value);
            });
            node.set_corner_radius(self.appearance.corner_radius);
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
        node.set_attachment(TAB_KEY, self.interaction);
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        Some(LayoutNode::new())
    }

    fn update_layout(&self, layout: &LayoutNode) {
        layout.set_flex_direction(FlexDirection::Column);
        layout.set_align_items(Align::Stretch);
        layout.set_min_width(Some(0.0));
        layout.set_flex_shrink(1.0);
        layout.set_padding(self.appearance.horizontal_inset, Edge::Start);
        layout.set_padding(self.appearance.horizontal_inset, Edge::End);
        // `.padding(horizontal:, vertical: top)` followed by
        // `.padding(horizontal: 0, vertical: bottom - top)`.
        let total_vertical = self.top + (self.bottom - self.top);
        layout.set_padding(total_vertical, Edge::Top);
        layout.set_padding(total_vertical, Edge::Bottom);
    }

    fn children(&self, node: &Node) -> Vec<AnyView> {
        // Inside a sliding strip the thumb already says which tab is chosen.
        let slides = is_in_sliding_selection(node);
        let foreground = if !self.is_enabled {
            SemanticColorRef::on_surface_disabled()
        } else if self.is_selected {
            SemanticColorRef::on_surface()
        } else {
            SemanticColorRef::on_surface_variant()
        };
        vec![
            AnyView::new(TabLabelRow {
                label: self.label.clone(),
                foreground,
                min_height: (self.height - self.top - self.bottom).max(0.0),
            }),
            AnyView::new(TabIndicator {
                color: if self.is_selected && !slides {
                    self.theme.colors.accent
                } else {
                    Color::CLEAR
                },
            }),
        ]
    }
}

impl_primitive_view!(TabChrome);

struct TabLabelRow {
    label: AnyView,
    foreground: SemanticColorRef,
    min_height: f32,
}

impl crate::PrimitiveView for TabLabelRow {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn update_node(&self, _node: &Node) {}

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
        layout.set_min_height(Some(self.min_height));
        // A long title truncates inside a clamped tab instead of forcing the
        // tab wider than the strip.
        layout.set_min_width(Some(0.0));
        layout.set_flex_shrink(1.0);
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        let label = AnyView::new(self.label.clone())
            .semantic_font(SemanticFontRef::label())
            .semantic_foreground(self.foreground.clone());
        vec![AnyView::new(label)]
    }
}

impl_primitive_view!(TabLabelRow);

struct TabIndicator {
    color: Color,
}

impl crate::PrimitiveView for TabIndicator {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node.set_view_tag(Some("tab-selection-indicator".to_string()));
        node
    }

    fn update_node(&self, node: &Node) {
        node.set_background_color(Some(self.color));
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        Some(LayoutNode::new())
    }

    fn update_layout(&self, layout: &LayoutNode) {
        layout.set_height(Some(2.0));
    }
}

impl_primitive_view!(TabIndicator);
