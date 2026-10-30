//! Translation of `Sources/CoveUIWorkspace/Theme/TabStyle.swift`.
//!
//! Panel tabs are selection blocks inside the pane's tab bar: no rules above or
//! between tabs, so the rounded pane stays one continuous surface. Unlike the
//! shared `TabButtonStyle` this style paints no bottom indicator and no
//! selection pill — the workspace tab is a rounded fill flush inside the bar,
//! `height - 8` tall, with 9pt horizontal padding.
//!
//! Behaviour checklist:
//! - the tab is `height - 8` tall with 9pt leading/trailing padding
//! - fill is `surfaceVariant` when selected, `stateLayerHover` when merely
//!   hovered, and clear otherwise
//! - a 6pt corner radius and a 1pt `focusRing` border apply, the border only on
//!   focus
//! - the interaction key drives the fast motion animation, so a changed
//!   selection or hover animates rather than snapping
//! - no rules are drawn above or between tabs

use cove_core::scene::{
    with_animation, Align, Animation, Edge, FlexDirection, LayoutNode, Node,
};
use cove_core::Color;

use cove_ui_compose::theme::styles::buttons::button_style::{
    ButtonInteractionKey, ButtonStyle, ButtonStyleConfiguration,
};
use cove_ui_compose::theme::ColorScheme;
use cove_composition::{impl_primitive_view, AnyView, PrimitiveView};

/// Panel tab chrome for the workspace tab bar.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorkspaceTabStyle {
    /// Full tab-bar height; the tab itself is drawn `height - 8`.
    pub height: f32,
}

impl WorkspaceTabStyle {
    pub fn new(height: f32) -> Self {
        Self { height }
    }
}

impl Default for WorkspaceTabStyle {
    fn default() -> Self {
        Self::new(34.0)
    }
}

impl ButtonStyle for WorkspaceTabStyle {
    fn make_body(&self, configuration: ButtonStyleConfiguration) -> AnyView {
        // Snapshot the theme before moving the label out of the configuration.
        let colors = configuration.theme.colors;
        let motion = configuration.theme.motion;
        let interaction = configuration.interaction_key();
        AnyView::new(WorkspaceTabChrome {
            label: configuration.label,
            colors,
            motion,
            height: self.height,
            is_selected: configuration.is_selected,
            is_hovered: configuration.is_hovered,
            is_focused: configuration.is_focused,
            is_enabled: configuration.is_enabled,
            interaction,
        })
    }

    fn style_identity(&self) -> u64 {
        self.height.to_bits() as u64
    }
}

const WORKSPACE_TAB_KEY: &str = "cove.workspace.tab.interaction";

/// Corner radius of the panel tab block.
const CORNER_RADIUS: f32 = 6.0;
/// Horizontal padding around the label.
const LABEL_PADDING: f32 = 9.0;
/// The tab is inset this much inside the bar's full height.
const TAB_HEIGHT_INSET: f32 = 8.0;

struct WorkspaceTabChrome {
    label: AnyView,
    colors: ColorScheme,
    motion: cove_ui_compose::theme::MotionScale,
    height: f32,
    is_selected: bool,
    is_hovered: bool,
    is_focused: bool,
    is_enabled: bool,
    interaction: ButtonInteractionKey,
}

impl PrimitiveView for WorkspaceTabChrome {
    fn make_node(&self) -> Node {
        let node = Node::new();
        // The enclosing tab bar owns hit testing for the strip; the tab block
        // itself only paints, so a click anywhere in the bar still reaches the
        // row beneath it rather than being eaten by the fill.
        node.set_is_hit_testable(false);
        node
    }

    fn update_node(&self, node: &Node) {
        // Selection wins over hover, hover over the resting clear fill. Swift
        // expresses this as a nested ternary in the same order.
        let fill = if self.is_selected {
            self.colors.surface_variant
        } else if self.is_hovered {
            self.colors.state_layer_hover
        } else {
            Color::CLEAR
        };
        let border = if self.is_focused { self.colors.focus_ring } else { Color::CLEAR };
        let border_width = if self.is_focused { 1.0 } else { 0.0 };

        let animate = node
            .attachment::<ButtonInteractionKey>(WORKSPACE_TAB_KEY)
            .is_some_and(|previous| previous != self.interaction);
        node.set_attachment(WORKSPACE_TAB_KEY, self.interaction);

        let apply = || {
            node.set_background_color(Some(fill));
            node.set_corner_radius(CORNER_RADIUS);
            node.set_border_color(Some(border));
            node.set_border_width(border_width);
            // A disabled tab keeps its block but reads as unavailable.
            node.set_opacity(if self.is_enabled { 1.0 } else { 0.5 });
        };
        if animate {
            with_animation(
                Some(Animation::new(
                    self.motion.fast,
                    self.motion.standard_easing.as_animation_curve(),
                )),
                apply,
            );
        } else {
            apply();
        }
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        Some(LayoutNode::new())
    }

    fn update_layout(&self, layout: &LayoutNode) {
        layout.set_flex_direction(FlexDirection::Row);
        layout.set_align_items(Align::Center);
        layout.set_justify_content(cove_core::scene::Justify::Center);
        layout.set_padding(LABEL_PADDING, Edge::Start);
        layout.set_padding(LABEL_PADDING, Edge::End);
        layout.set_height(Some(self.height - TAB_HEIGHT_INSET));
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        // `.makeBody` centres `configuration.label` in a row box with no rules
        // of its own; the enclosing tab bar supplies the shared surface.
        vec![self.label.clone()]
    }
}

impl_primitive_view!(WorkspaceTabChrome);
