//! Translation of Primitives/Overlays/Popover.swift.
//!
//! Overlay opened by a labelled trigger button. Touch surfaces use a bottom
//! sheet; pointer surfaces anchor the overlay to its trigger. The popup is
//! portal-rendered, so it escapes any clipping or stacking context the trigger
//! sits in, and it is dismissed by Escape, by losing the pointer, or by the
//! trigger being pressed again.
use std::cell::Cell;
use std::rc::Rc;

use cove_core::scene::{Align, EventResult, FlexDirection, Justify, LayoutNode, Node, Wrap};
use cove_core::Binding;
use crate::modifiers::layout::{FlexExt, LayoutFrameExt};
use crate::modifiers::style::AppearanceExt;
use crate::primitives::controls::button::{Button, ButtonRole};
use crate::primitives::overlays::anchored::{
    AnchoredOverlayHost, PopoverOverflow, PopoverPlacement, PopoverSurface,
};
use crate::theme::styles::buttons::{ButtonStyle, ButtonStyleConfiguration};
use crate::{impl_primitive_view, AnyView, BoxView, PrimitiveView, View};

/// How the popup sizes itself.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum PopoverSizing {
    #[default]
    Content,
    Anchor,
}

/// Accessible description attached to the trigger.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PopoverTriggerDescription {
    pub label: String,
    pub value: String,
    pub help: Option<String>,
}

/// Overlay opened by a labelled trigger button.
pub struct Popover {
    pub is_presented: Binding<bool>,
    pub is_enabled: bool,
    pub width: Option<f32>,
    pub placement: PopoverPlacement,
    pub surface: PopoverSurface,
    pub overflow: PopoverOverflow,
    pub on_key: Option<Rc<dyn Fn(&cove_platform::KeyEvent, cove_core::scene::EventPhase) -> EventResult>>,
    pub sizing: PopoverSizing,
    pub trigger_description: Option<PopoverTriggerDescription>,
    label: AnyView,
    content: AnyView,
}

impl Popover {
    pub fn new(label: impl View + 'static, content: impl View + 'static) -> Self {
        Self {
            is_presented: Binding::constant(false),
            is_enabled: true,
            width: None,
            placement: PopoverPlacement::BottomLeading,
            surface: PopoverSurface::Content,
            overflow: PopoverOverflow::Flip,
            on_key: None,
            sizing: PopoverSizing::Content,
            trigger_description: None,
            label: AnyView::new(label),
            content: AnyView::new(content),
        }
    }

    pub fn is_presented(mut self, binding: Binding<bool>) -> Self {
        self.is_presented = binding;
        self
    }

    pub fn enabled(mut self, is_enabled: bool) -> Self {
        self.is_enabled = is_enabled;
        self
    }

    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    pub fn placement(mut self, placement: PopoverPlacement) -> Self {
        self.placement = placement;
        self
    }

    pub fn surface(mut self, surface: PopoverSurface) -> Self {
        self.surface = surface;
        self
    }

    pub fn overflow(mut self, overflow: PopoverOverflow) -> Self {
        self.overflow = overflow;
        self
    }

    pub fn sizing(mut self, sizing: PopoverSizing) -> Self {
        self.sizing = sizing;
        self
    }

    pub fn trigger_description(mut self, description: PopoverTriggerDescription) -> Self {
        self.trigger_description = Some(description);
        self
    }
}

impl View for Popover {
    fn body(&self) -> AnyView {
        // A popover disabled while open must close, not linger over a dead
        // trigger.
        if !self.is_enabled && self.is_presented.get() {
            self.is_presented.set(false);
        }
        let is_presented = self.is_presented.clone();
        let is_enabled = self.is_enabled;
        let trigger = AnyView::new(
            Button::with_options(
                PopoverTriggerLabel {
                    label: self.label.clone(),
                    description: self.trigger_description.clone(),
                },
                move || is_presented.set(!is_presented.get()),
                ButtonRole::Normal,
                is_enabled,
                false,
                self.is_presented.get() && is_enabled,
                self.trigger_description.as_ref().and_then(|description| description.help.clone()),
            )
            .button_style(PopoverTriggerStyle),
        );
        let overlay = AnyView::new(AnchoredOverlayHost::new(self.is_presented.get() && is_enabled, self.content.clone())
            .width_opt(self.width)
            .placement(self.placement)
            .sizing_anchor(self.sizing == PopoverSizing::Anchor)
            .surface(self.surface)
            .overflow(self.overflow)
            .on_dismiss({
                let binding = self.is_presented.clone();
                move || binding.set(false)
            }));
        AnyView::new(
            BoxView::new(
                FlexDirection::Column,
                if self.sizing == PopoverSizing::Anchor { Align::Stretch } else { Align::FlexStart },
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                AnyView::new(crate::TupleView::new(vec![trigger, overlay])),
            ),
        )
    }
}

/// Trigger content with the accessibility description applied.
struct PopoverTriggerLabel {
    label: AnyView,
    description: Option<PopoverTriggerDescription>,
}

impl PrimitiveView for PopoverTriggerLabel {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        None
    }

    fn update_node(&self, _node: &Node) {}

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        let label = self
            .description
            .as_ref()
            .map(|description| description.label.clone())
            .unwrap_or_default();
        let value = self.description.as_ref().map(|description| description.value.clone()).unwrap_or_default();
        vec![AnyView::new(PopoverLabelSemantics { content: self.label.clone(), label, value })]
    }
}
impl_primitive_view!(PopoverTriggerLabel);

/// Applies the trigger's label and value to the control it wraps.
struct PopoverLabelSemantics {
    content: AnyView,
    label: String,
    value: String,
}

impl PrimitiveView for PopoverLabelSemantics {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        None
    }

    fn update_node(&self, node: &Node) {
        node.set_attachment("__popover.label", self.label.clone());
        node.set_attachment("__popover.value", self.value.clone());
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![self.content.clone()]
    }
}
impl_primitive_view!(PopoverLabelSemantics);

/// Trigger chrome: contributes only the keyboard focus ring, outside the
/// trigger's own box. Pointer focus is invisible, which is why an open popover
/// never shows a ring around the control that opened it.
struct PopoverTriggerStyle;

impl ButtonStyle for PopoverTriggerStyle {
    fn make_body(&self, configuration: ButtonStyleConfiguration) -> AnyView {
        let theme = configuration.theme.clone();
        // The ring lives on its own wrapper node: modifiers on an `AnyView` land
        // on the label's node, so a clear ring would erase a label's border.
        // It fills the button's row, so an anchor-width trigger spans its frame
        // while a hugging trigger has no space to fill.
        AnyView::new(
            BoxView::new(
                FlexDirection::Column, Align::Stretch, Justify::FlexStart, Wrap::NoWrap, 0.0,
                AnyView::new(configuration.label.clone()),
            )
            .flex_with(1.0, 1.0, None)
            .frame_min_width(0.0)
            .corner_radius(theme.inputs.radius)
            .border_color(
                if configuration.is_focused { theme.colors.focus_ring } else { cove_core::Color::CLEAR },
                if configuration.is_focused { 2.0 } else { 0.0 },
            ),
        )
    }
}

impl AnchoredOverlayHost {
    fn width_opt(mut self, width: Option<f32>) -> Self {
        self.width = width;
        self
    }
}
