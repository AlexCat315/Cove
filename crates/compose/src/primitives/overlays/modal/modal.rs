//! Translation of Primitives/Overlays/Modal/Modal.swift.
//!
//! Presented modals are portal entries covering the window: a backdrop that owns
//! dismissal plus one animated panel. The backing node stays mounted while the
//! exit transition runs, so the panel animates out instead of vanishing.
use crate::Scope;
use std::rc::Rc;

use crate::animation::content::AnimatedVisibility;
use crate::animation::transition::Transition;
use crate::modifiers::interaction::AccessibilityExt;
use crate::modifiers::layout::LayoutFrameExt;
use crate::modifiers::style::{AppearanceExt, SemanticStyleViewExt};
use crate::primitives::overlays::focus_scope::FocusScope;
use crate::primitives::overlays::portal::portal_registry::PortalResource;
use crate::primitives::overlays::portal::{
    PortalStoreEnvironment, PortalStoreHolder, portal_window_bounds,
};
use crate::theme::theme_for_node;
use crate::{AnyView, PrimitiveView, View, ViewExt, impl_primitive_view};
use cove_core::scene::{
    AccessibilityRole, Align, EventResult, FlexDirection, InputHandlerRole, InputHandlerRoute,
    InputRoutingPriority, InteractionRegistryHolder, Justify, LayoutNode, Node, PointerPhase,
    PositionType,
};
use cove_core::{Binding, Color, Edge, Point, Rect, Signal, Size};
use cove_platform::MouseButton;

use super::modal_options::{ModalBackdrop, ModalGeometry, ModalOptions, ModalPlacement};

const WINDOW_BOUNDS: &str = "modal.windowBounds";

/// A modal dialog presented over the window.
pub struct Modal {
    pub is_presented: Binding<bool>,
    pub options: ModalOptions,
    fitting_geometry: Option<Rc<dyn Fn(&mut ModalGeometry, Size)>>,
    content: AnyView,
}

impl Modal {
    pub fn new(is_presented: Binding<bool>, content: impl View + 'static) -> Self {
        Self {
            is_presented,
            options: ModalOptions::default(),
            fitting_geometry: None,
            content: AnyView::new(content),
        }
    }

    pub fn options(mut self, options: ModalOptions) -> Self {
        self.options = options.validated();
        self
    }

    pub fn fitting_geometry(mut self, fit: impl Fn(&mut ModalGeometry, Size) + 'static) -> Self {
        self.fitting_geometry = Some(Rc::new(fit));
        self
    }
}

impl View for Modal {
    /// `retained` keeps the panel mounted and painting while it animates out,
    /// so the exit transition plays instead of the portal vanishing.
    fn body(&self, cx: &Scope) -> AnyView {
        AnyView::new(ModalPresenter {
            is_presented: self.is_presented.clone(),
            retained: cx.state(|| false),
            options: self.options.clone(),
            fitting_geometry: self.fitting_geometry.clone(),
            content: self.content.clone(),
        })
    }
}

struct ModalPresenter {
    is_presented: Binding<bool>,
    retained: Signal<bool>,
    options: ModalOptions,
    fitting_geometry: Option<Rc<dyn Fn(&mut ModalGeometry, Size)>>,
    content: AnyView,
}

impl PrimitiveView for ModalPresenter {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        let resource: Rc<dyn cove_core::scene::NodeResource> = PortalResource::new();
        node.add_resource(resource);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        None
    }

    fn update_node(&self, node: &Node) {
        present(
            node,
            self.is_presented.get(),
            &self.retained,
            &self.options,
            &self.fitting_geometry,
            &self.content,
            &self.is_presented,
        );
        let presented = self.is_presented.clone();
        let retained = self.retained.clone();
        let options = self.options.clone();
        let fitting = self.fitting_geometry.clone();
        let content = self.content.clone();
        let weak = node.downgrade();
        node.set_layout_did_update(Some(Rc::new(move |owner: &Node| {
            let Some(node) = Node::upgrade(&weak) else {
                return;
            };
            if node.attachment::<Rect>(WINDOW_BOUNDS) != Some(portal_window_bounds(&node)) {
                present(
                    &node,
                    presented.get(),
                    &retained,
                    &options,
                    &fitting,
                    &content,
                    &presented,
                );
            }
            let _ = owner;
        }) as cove_core::scene::LayoutUpdateHook));
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        Vec::new()
    }
}
impl_primitive_view!(ModalPresenter);

fn present(
    node: &Node,
    is_presented: bool,
    retained: &Signal<bool>,
    options: &ModalOptions,
    fitting: &Option<Rc<dyn Fn(&mut ModalGeometry, Size)>>,
    content: &AnyView,
    is_presented_binding: &Binding<bool>,
) {
    let bounds = portal_window_bounds(node);
    node.set_attachment(WINDOW_BOUNDS, bounds);
    let resource = node.first_resource::<PortalResource>();
    if !(is_presented || retained.get()) {
        if let Some(resource) = resource.as_ref() {
            cove_core::scene::NodeResource::unmount(&**resource, node);
        }
        return;
    }
    if is_presented && !retained.get() {
        retained.set(true);
    }
    let store = Some(
        node.composition_value(&PortalStoreEnvironment::key())
            .unwrap_or_else(PortalStoreHolder::current),
    );
    let mut fitted = options.clone();
    if let Some(fit) = fitting.as_ref() {
        fit(
            &mut fitted.geometry,
            Size {
                width: bounds.width,
                height: bounds.height,
            },
        );
    }
    fitted.geometry.fit_compact_window(Size {
        width: bounds.width,
        height: bounds.height,
    });
    let retained_state = retained.clone();
    let panel = AnyView::new(AnimatedVisibility::new(
        is_presented,
        Transition::opacity(),
        None,
        true,
        Some(Rc::new(move |visible| {
            if !visible {
                retained_state.set(false);
            }
        })),
        AnyView::new(FocusScope::new(AnyView::new(ModalBackdropView {
            is_visible: is_presented,
            options: fitted.clone(),
            content: content.clone(),
            dismiss: Rc::new({
                let binding = is_presented_binding.clone();
                move || binding.set(false)
            }),
        }))),
    ));
    if let Some(resource) = resource.as_ref() {
        resource.present(
            store,
            Point::default(),
            None,
            AnyView::new(panel.frame(
                Some(crate::modifiers::layout::FrameDimension::Percent(100.0)),
                Some(crate::modifiers::layout::FrameDimension::Percent(100.0)),
                None,
                None,
                None,
                None,
            )),
            true,
            crate::primitives::overlays::portal::PortalPlacementMode::Fixed,
            None,
            None,
        );
    }
}

/// Backdrop that owns dismissal plus one animated panel.
struct ModalBackdropView {
    is_visible: bool,
    options: ModalOptions,
    dismiss: Rc<dyn Fn()>,
    content: AnyView,
}

impl PrimitiveView for ModalBackdropView {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(true);
        node.set_clips_to_bounds(true);
        node
    }

    fn update_node(&self, node: &Node) {
        let theme = theme_for_node(node);
        let fill = match self.options.backdrop {
            ModalBackdrop::Standard => theme.colors.overlay,
            ModalBackdrop::Dimmed => Color::BLACK.multiplied_alpha(0.2),
        };
        node.set_background_color(Some(fill));
        let Some(registry) = InteractionRegistryHolder::current() else {
            return;
        };
        let closes_on_backdrop = self.options.dismissal.closes_on_backdrop;
        let closes_on_escape = self.options.dismissal.closes_on_escape;
        let dismiss = self.dismiss.clone();
        registry.set_pointer(
            node,
            InputHandlerRoute::new(
                InputHandlerRole::Control,
                InputRoutingPriority::MODAL,
                "modal.backdrop",
            ),
            Rc::new(move |event, phase, event_phase| {
                if event_phase != cove_core::scene::EventPhase::Target {
                    return EventResult::Ignored;
                }
                if event.button != MouseButton::Left {
                    return EventResult::Handled;
                }
                if closes_on_backdrop && phase == PointerPhase::Down {
                    dismiss();
                }
                EventResult::Handled
            }),
        );
        let dismiss = self.dismiss.clone();
        registry.set_key(
            node,
            InputHandlerRoute::overlay(),
            Rc::new(move |event, _phase| {
                if event.scancode == crate::Scancode::ESCAPE {
                    if closes_on_escape {
                        dismiss();
                    }
                    return EventResult::Handled;
                }
                EventResult::Ignored
            }),
        );
        registry.set_wheel(
            node,
            InputHandlerRoute::overlay(),
            Rc::new(move |_event, phase| {
                if phase == cove_core::scene::EventPhase::Capture {
                    EventResult::Ignored
                } else {
                    EventResult::Handled
                }
            }),
        );
        let _ = self.is_visible;
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        layout.set_position_type(PositionType::Absolute);
        layout.set_position(0.0, Edge::Start);
        layout.set_position(0.0, Edge::Top);
        layout.set_position(0.0, Edge::End);
        layout.set_position(0.0, Edge::Bottom);
        // `inset` is the documented margin from the window edge, not only a cap
        // on the dialog size. Without padding, top/bottom placement put a
        // rounded panel flush against the window and clipped its corners.
        if self.options.geometry.inset > 0.0 {
            layout.set_padding(self.options.geometry.inset, Edge::All);
        }
        layout.set_flex_direction(FlexDirection::Column);
        layout.set_align_items(match self.options.geometry.placement {
            ModalPlacement::Leading => Align::FlexStart,
            ModalPlacement::Trailing => Align::FlexEnd,
            _ => Align::Center,
        });
        layout.set_justify_content(match self.options.geometry.placement {
            ModalPlacement::Bottom => Justify::FlexEnd,
            ModalPlacement::Top => Justify::FlexStart,
            _ => Justify::Center,
        });
        Some(layout)
    }

    fn children(&self, node: &Node) -> Vec<AnyView> {
        let bounds = portal_window_bounds(node);
        let inset = self.options.geometry.inset;
        let size = Size {
            width: self
                .options
                .geometry
                .width
                .min((bounds.width - inset * 2.0).max(0.0)),
            height: self
                .options
                .geometry
                .height
                .min((bounds.height - inset * 2.0).max(0.0)),
        };
        let panel = AnyView::new(AnimatedVisibility::new(
            self.is_visible,
            self.options.geometry.transition(size),
            None,
            true,
            None,
            AnyView::new(
                self.content
                    .clone()
                    .frame(
                        Some(crate::modifiers::layout::FrameDimension::Percent(100.0)),
                        if self.options.fits_content_height {
                            None
                        } else {
                            Some(crate::modifiers::layout::FrameDimension::Percent(100.0))
                        },
                        None,
                        None,
                        None,
                        None,
                    )
                    .accessibility({
                        let label = self.options.label.clone();
                        move |semantics| {
                            semantics.role = AccessibilityRole::Dialog;
                            semantics.label = label.clone();
                        }
                    })
                    .semantic_background(self.options.surface.clone())
                    .corner_radius(self.options.geometry.corner_radius)
                    // The modal lift: a wide, deep shadow.
                    .shadow(Color::from_rgba8(0, 0, 0, 77), 0.0, 10.0, 30.0),
            ),
        ));
        // Fixed dialogs need a definite animation-host height for flexible
        // content. Action sheets instead measure their bounded intrinsic height.
        let sized = AnyView::new(panel.frame(
            Some(crate::modifiers::layout::FrameDimension::Points(size.width)),
            if self.options.fits_content_height {
                None
            } else {
                Some(crate::modifiers::layout::FrameDimension::Points(
                    size.height,
                ))
            },
            None,
            None,
            None,
            Some(size.height),
        ));
        vec![AnyView::new(sized.modifier(TranslationY {
            value: self.options.translation_y,
        }))]
    }
}
impl_primitive_view!(ModalBackdropView);

/// Vertical offset applied after layout so the panel's own height is unaffected.
struct TranslationY {
    value: f32,
}

impl cove_composition::ViewModifier for TranslationY {
    fn apply_node(&self, node: &Node) {
        let value = self.value;
        let weak = node.downgrade();
        node.animatable_set_with(
            "modal.translationY",
            node.content_offset().y,
            value,
            move |offset| {
                if let Some(owner) = Node::upgrade(&weak) {
                    let current = owner.content_offset();
                    owner.set_content_offset(Point::new(current.x, offset));
                }
            },
        );
    }
}
