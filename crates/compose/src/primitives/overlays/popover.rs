//! Translation of Primitives/Overlays/Popover.swift.
//!
//! Overlay opened by a labelled trigger button. Touch surfaces use a bottom
//! sheet; pointer surfaces anchor the overlay to its trigger. The popup is
//! portal-rendered, so it escapes any clipping or stacking context the trigger
//! sits in, and it is dismissed by Escape, by losing the pointer, or by the
//! trigger being pressed again.
use crate::Scope;
use std::rc::Rc;

use crate::adaptive::SurfaceReader;
use crate::modifiers::animation_value::AnimationValueExt;
use crate::modifiers::interaction::AccessibilityExt;
use crate::modifiers::layout::{EdgeInsets, FlexExt, FrameDimension, LayoutFrameExt, PaddingExt};
use crate::modifiers::style::AppearanceExt;
use crate::primitives::controls::button::{ActionControlAccessibility, Button, ButtonRole};
use crate::primitives::layout::direction::ResolvedLayoutDirection;
use crate::primitives::layout::{ScrollAxis, ScrollView, ScrollbarGutter};
use crate::primitives::overlays::anchored::{
    AnchoredOverlayHost, MenuKeyHost, PopoverOverflow, PopoverPlacement, PopoverSurface,
};
use crate::primitives::overlays::modal::{
    Modal, ModalBackdrop, ModalOptions, ModalPlacement, SheetDragHandle, TouchPresentation,
    uses_sheet,
};
use crate::theme::SemanticMotionRef;
use crate::theme::styles::buttons::{
    AnyButtonStyle, ButtonStyle, ButtonStyleConfiguration, ButtonStyleViewExt,
};
use crate::{AnyView, BoxView, LayoutDebugExt, LayoutDirectionExt, View, ViewExt};
use cove_core::scene::{Align, EventPhase, EventResult, FlexDirection, Justify, Wrap};
use cove_core::{Binding, Environment, WidthClass};

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
    pub on_key: Option<KeyHandler>,
    pub sizing: PopoverSizing,
    pub trigger_description: Option<PopoverTriggerDescription>,
    pub trigger_style: Option<AnyButtonStyle>,
    label: AnyView,
    content: AnyView,
}

type KeyHandler = Rc<dyn Fn(&cove_platform::KeyEvent, EventPhase) -> EventResult>;

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
            trigger_style: None,
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

    /// Replaces the focus-ring-only chrome, so a styled trigger is the
    /// popover's own button rather than a second button nested inside it.
    pub fn trigger_style(mut self, style: impl ButtonStyle + 'static) -> Self {
        self.trigger_style = Some(AnyButtonStyle::new(style));
        self
    }

    pub fn on_key(
        mut self,
        handler: impl Fn(&cove_platform::KeyEvent, EventPhase) -> EventResult + 'static,
    ) -> Self {
        self.on_key = Some(Rc::new(handler));
        self
    }

    fn trigger(&self) -> AnyView {
        let presented = self.is_presented.get() && self.is_enabled;
        let toggle = self.is_presented.clone();
        let button = Button::with_options(
            self.label.clone(),
            move || toggle.set(!toggle.get()),
            ButtonRole::Normal,
            self.is_enabled,
            false,
            presented,
            self.trigger_description
                .as_ref()
                .and_then(|description| description.help.clone()),
        );
        let styled = match &self.trigger_style {
            Some(style) => button.button_style_any(style.clone()),
            None => button.button_style(PopoverTriggerStyle),
        };
        // A closed popover reports `Some(false)` rather than no state.
        let trigger =
            styled.composition_local(ActionControlAccessibility::expanded(), Some(presented));
        match &self.trigger_description {
            Some(description) => AnyView::new(
                trigger
                    .composition_local(
                        ActionControlAccessibility::label(),
                        Some(description.label.clone()),
                    )
                    .composition_local(
                        ActionControlAccessibility::value(),
                        description.value.clone(),
                    ),
            ),
            None => AnyView::new(trigger),
        }
    }
}

impl View for Popover {
    fn body(&self, _cx: &Scope) -> AnyView {
        // A popover disabled while open must close, not linger over a dead
        // trigger.
        if !self.is_enabled && self.is_presented.get() {
            self.is_presented.set(false);
        }
        let overlay = AdaptiveOverlayHost {
            is_presented: self.is_presented.clone(),
            is_enabled: self.is_enabled,
            width: self.width,
            placement: self.placement,
            sizing: self.sizing,
            surface: self.surface,
            overflow: self.overflow,
            on_key: self.on_key.clone(),
            content: self.content.clone(),
        };
        let raised = self.is_presented.get();
        AnyView::new(
            BoxView::new(
                FlexDirection::Column,
                if self.sizing == PopoverSizing::Anchor {
                    Align::Stretch
                } else {
                    Align::FlexStart
                },
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                AnyView::new(crate::TupleView::new(vec![
                    self.trigger(),
                    AnyView::new(overlay),
                ])),
            )
            .z_index(if raised { 10_000.0 } else { 0.0 }),
        )
    }
}

/// Presents overlay content as a bottom sheet on touch surfaces and as an
/// anchored panel for pointer input.
pub(crate) struct AdaptiveOverlayHost {
    pub is_presented: Binding<bool>,
    pub is_enabled: bool,
    pub width: Option<f32>,
    pub placement: PopoverPlacement,
    pub sizing: PopoverSizing,
    pub surface: PopoverSurface,
    pub overflow: PopoverOverflow,
    pub on_key: Option<KeyHandler>,
    pub content: AnyView,
}

impl View for AdaptiveOverlayHost {
    fn body(&self, _cx: &Scope) -> AnyView {
        let is_presented = self.is_presented.clone();
        let is_enabled = self.is_enabled;
        let width = self.width;
        let placement = self.placement;
        let sizing = self.sizing;
        let surface = self.surface;
        let overflow = self.overflow;
        let on_key = self.on_key.clone();
        let content = self.content.clone();
        AnyView::new(SurfaceReader::new(move |environment: &Environment| {
            if uses_sheet(environment) {
                return AnyView::new(TouchOverlay {
                    is_presented: is_presented.clone(),
                    content: content.clone(),
                    on_key: on_key.clone(),
                });
            }
            let dismiss = is_presented.clone();
            let mut host =
                AnchoredOverlayHost::new(is_enabled && is_presented.get(), content.clone())
                    .width_opt(width)
                    .placement(placement)
                    .sizing_anchor(sizing == PopoverSizing::Anchor)
                    .surface(surface)
                    .overflow(overflow)
                    .on_dismiss(move || dismiss.set(false));
            host.key_handler = on_key.clone();
            AnyView::new(host)
        }))
    }
}

/// Touch presentation: a dimmed bottom sheet with a drag handle and scrolling
/// content, padded clear of the safe area and the software keyboard.
pub(crate) struct TouchOverlay {
    pub is_presented: Binding<bool>,
    pub content: AnyView,
    pub on_key: Option<KeyHandler>,
}

impl View for TouchOverlay {
    fn body(&self, _cx: &Scope) -> AnyView {
        let is_presented = self.is_presented.clone();
        let content = self.content.clone();
        let on_key = self.on_key.clone();
        AnyView::new(SurfaceReader::new(move |surface: &Environment| {
            let safe = surface.safe_area.normalized();
            let bottom = safe.bottom.max(surface.keyboard.height.max(0.0));
            let available = (surface.viewport.height - safe.top - bottom - 12.0).max(0.0);
            let mut options = ModalOptions::default();
            options.geometry.placement = ModalPlacement::Bottom;
            options.geometry.inset = 0.0;
            options.geometry.width = if surface.width_class() == WidthClass::Compact {
                f32::MAX
            } else {
                560.0
            };
            options.geometry.height = (available + bottom).max(64.0);
            options.geometry.corner_radius = 18.0;
            options.fits_content_height = true;
            options.backdrop = ModalBackdrop::Dimmed;

            let is_presented = is_presented.clone();
            let content = content.clone();
            let on_key = on_key.clone();
            AnyView::new(ResolvedLayoutDirection::new(move |direction| {
                let dismiss = is_presented.clone();
                let handle = SheetDragHandle::new(
                    |_| {},
                    move |distance, velocity| {
                        if distance > 80.0 || (distance > 0.0 && velocity > 900.0) {
                            dismiss.set(false);
                        }
                    },
                )
                .accessibility_hidden(true);
                let scroller = ScrollView::new(
                    ScrollAxis::Vertical,
                    content
                        .clone()
                        .frame(
                            Some(FrameDimension::Percent(100.0)),
                            None,
                            Some(0.0),
                            None,
                            None,
                            None,
                        )
                        .composition_local(TouchPresentation::menu(), true),
                )
                .scrollbar_gutter(ScrollbarGutter::Overlay)
                .frame(
                    None,
                    None,
                    None,
                    None,
                    None,
                    Some((available - 56.0).max(0.0)),
                );
                let column = BoxView::new(
                    FlexDirection::Column,
                    Align::Stretch,
                    Justify::FlexStart,
                    Wrap::NoWrap,
                    0.0,
                    AnyView::new(crate::TupleView::new(vec![
                        AnyView::new(handle),
                        AnyView::new(scroller),
                    ])),
                )
                .padding_insets(EdgeInsets::new(
                    0.0,
                    safe.leading.max(12.0),
                    bottom + 12.0,
                    safe.trailing.max(12.0),
                ))
                .layout_direction(direction)
                .debug_name("touch-overlay-sheet");
                let key = on_key.clone();
                Modal::new(
                    is_presented.clone(),
                    MenuKeyHost::new(
                        move |event| {
                            key.as_ref().is_some_and(|key| {
                                key(event, EventPhase::Target) == EventResult::Handled
                            })
                        },
                        AnyView::new(column),
                    ),
                )
                .options(options.clone())
            }))
        }))
    }
}

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
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                AnyView::new(configuration.label.clone()),
            )
            .flex_with(1.0, 1.0, None)
            .frame_min_width(0.0)
            .corner_radius(theme.inputs.radius)
            .border_color(
                if configuration.is_focused {
                    theme.colors.focus_ring
                } else {
                    cove_core::Color::CLEAR
                },
                if configuration.is_focused { 2.0 } else { 0.0 },
            )
            .animation_motion(SemanticMotionRef::snappy(), configuration.interaction_key()),
        )
    }
}

impl AnchoredOverlayHost {
    fn width_opt(mut self, width: Option<f32>) -> Self {
        self.width = width;
        self
    }
}
