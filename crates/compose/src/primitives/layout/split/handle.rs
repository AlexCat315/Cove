//! Divider that converts pointer travel into a fraction of its container.
//!
//! The container extent is sampled from the parent node at drag start rather
//! than from the handle, so a drag that leaves the handle still tracks — the
//! handle captures the pointer for the whole gesture.
use std::cell::RefCell;
use std::rc::Rc;

use cove_core::scene::{
    AccessibilityActions, AccessibilityRole, AccessibilitySemantics, EventPhase, EventResult,
    FocusChainHolder, HoverPhase, InputHandlerRoute, InteractionRegistryHolder, LayoutNode, Node,
    NodeResource, PointerCaptureAttachmentKey, PointerCaptureHolder, PointerPhase,
};
use cove_core::{Binding, Color};
use cove_platform::MouseButton;

use crate::theme::theme_for_node;
use crate::{AnyView, PrimitiveView, impl_primitive_view};

use super::SplitAxis;
use super::constraints::SplitConstraints;
use super::retention::CONTAINER_MARKER;

const START: &str = "splitview.dragStart";
const HOVER: &str = "splitview.hover";

#[derive(Clone, Copy, Debug, PartialEq)]
struct DragStart {
    fraction: f32,
    position: f32,
    extent: f32,
}

/// Divider that writes clamped fractions of its container back through a binding.
pub struct SplitHandle {
    pub axis: SplitAxis,
    pub thickness: f32,
    /// The grab extent this mount resolved from the surface. `update_layout`
    /// runs after `update_node`, so the touch width has to survive there.
    pub resolved_thickness: std::cell::Cell<Option<f32>>,
    pub color: Option<Color>,
    pub fraction: Binding<f32>,
    pub constraints: SplitConstraints,
    pub spacing: f32,
}

impl SplitHandle {
    fn container_extent(&self, node: &Node) -> f32 {
        let mut owner = node.parent();
        while let Some(candidate) = &owner {
            if candidate.attachment::<bool>(CONTAINER_MARKER) == Some(true) {
                break;
            }
            owner = candidate.parent();
        }
        let frame = owner.as_ref().map_or_else(|| node.frame(), Node::frame);
        let extent = match self.axis {
            SplitAxis::Horizontal => frame.width,
            SplitAxis::Vertical => frame.height,
        };
        (extent - self.thickness - 2.0 * self.spacing).max(0.0)
    }

    fn current(&self, node: &Node) -> f32 {
        let proposed = self.fraction.get();
        self.constraints
            .clamp(proposed, self.container_extent(node))
    }

    fn write(&self, node: &Node, value: f32) {
        if !value.is_finite() || !node.accepts_subtree_input() {
            return;
        }
        let clamped = self.constraints.clamp(value, self.container_extent(node));
        if self.fraction.get() != clamped {
            self.fraction.set(clamped);
        }
    }

    fn adjust(&self, node: &Node, delta: f32) {
        self.write(node, self.current(node) + delta);
    }

    fn release(&self, node: &Node) {
        node.remove_attachment(START);
        if let Some(capture) = PointerCaptureHolder::current()
            && capture.target().is_some_and(|target| target == *node)
        {
            capture.release();
        }
    }
}

impl PrimitiveView for SplitHandle {
    fn make_node(&self) -> Node {
        let node = Node::new();
        let resource: Rc<dyn NodeResource> = Rc::new(SplitHandleLifetime::new());
        node.add_resource(resource);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        Some(LayoutNode::new())
    }

    fn update_layout(&self, layout: &LayoutNode) {
        layout.set_flex_shrink(0.0);
        layout.set_align_self(cove_core::scene::Align::Stretch);
        let thickness = self.resolved_thickness.get().unwrap_or(self.thickness);
        match self.axis {
            SplitAxis::Horizontal => {
                layout.set_width(Some(thickness));
                layout.set_height(None);
            }
            SplitAxis::Vertical => {
                layout.set_height(Some(thickness));
                layout.set_width(None);
            }
        }
    }

    fn update_node(&self, node: &Node) {
        let thickness = self.resolve_thickness(node);
        self.resolved_thickness.set(Some(thickness));
        if let Some(layout) = node.layout_node() {
            match self.axis {
                SplitAxis::Horizontal => layout.set_width(Some(thickness)),
                SplitAxis::Vertical => layout.set_height(Some(thickness)),
            }
        }
        node.set_is_hit_testable(true);
        node.set_is_focusable(true);
        node.set_is_tab_stop(true);
        node.set_cursor(Some(self.axis.resize_cursor()));

        let current = self.current(node);
        node.set_accessibility(Some(AccessibilitySemantics {
            role: AccessibilityRole::Slider,
            label: crate::i18n::tr("Resize split"),
            value: format!("{}%", (current * 100.0).round() as i64),
            ..Default::default()
        }));

        let increment_node = node.downgrade();
        let increment = SplitHandle::clone_for_actions(self);
        node.set_accessibility_actions(AccessibilityActions {
            increment: Some(Rc::new(move || {
                if let Some(owner) = Node::upgrade(&increment_node) {
                    increment.adjust(&owner, 0.02);
                }
            })),
            decrement: {
                let decrement_node = node.downgrade();
                let decrement = SplitHandle::clone_for_actions(self);
                Some(Rc::new(move || {
                    if let Some(owner) = Node::upgrade(&decrement_node) {
                        decrement.adjust(&owner, -0.02);
                    }
                }))
            },
            ..Default::default()
        });

        let loss_node = node.downgrade();
        node.set_attachment(
            PointerCaptureAttachmentKey::DID_LOSE_CAPTURE,
            Rc::new(move || {
                if let Some(owner) = Node::upgrade(&loss_node) {
                    owner.remove_attachment(START);
                    owner.mark_render_dirty(cove_core::scene::InvalidationSource::style_set(
                        "splitview",
                    ));
                }
            }) as Rc<dyn Fn()>,
        );

        let draw_color = self.color;
        let axis = self.axis;
        let thickness = self.thickness;
        let draw_node = node.downgrade();
        node.update_draw(
            (draw_color.map(|color| color.rgba8()), thickness.to_bits()),
            move |list, origin| {
                let Some(owner) = Node::upgrade(&draw_node) else {
                    return;
                };
                let frame = owner.frame();
                let theme = theme_for_node(&owner);
                let resolved = draw_color.unwrap_or(theme.colors.divider);
                // Grab area painted inset from the divider line, so the visual
                // weight stays a hairline while the hit target stays usable.
                let rect = match axis {
                    SplitAxis::Horizontal => cove_core::Rect::new(
                        origin.x + (frame.width - 1.0) / 2.0,
                        origin.y,
                        1.0,
                        frame.height,
                    ),
                    SplitAxis::Vertical => cove_core::Rect::new(
                        origin.x,
                        origin.y + (frame.height - 1.0) / 2.0,
                        frame.width,
                        1.0,
                    ),
                };
                if rect.width <= 0.0 || rect.height <= 0.0 {
                    return;
                }
                let _ = list.add_rect(rect, resolved.multiplied_alpha(owner.opacity()));
            },
        );

        let Some(registry) = InteractionRegistryHolder::current() else {
            return;
        };

        let hover_node = node.downgrade();
        registry.set_hover(
            node,
            Rc::new(move |phase| {
                let Some(owner) = Node::upgrade(&hover_node) else {
                    return;
                };
                owner.set_attachment(HOVER, phase == HoverPhase::Enter);
                owner.mark_render_dirty(cove_core::scene::InvalidationSource::style_set(
                    "splitview",
                ));
            }),
        );

        let pointer_node = node.downgrade();
        let pointer_handle = SplitHandle::clone_for_actions(self);
        registry.set_pointer(
            node,
            InputHandlerRoute::control(),
            Rc::new(move |event, phase, _| {
                if event.button != MouseButton::Left {
                    return EventResult::Ignored;
                }
                let Some(owner) = Node::upgrade(&pointer_node) else {
                    return EventResult::Ignored;
                };
                match phase {
                    PointerPhase::Down => {
                        let extent = pointer_handle.container_extent(&owner);
                        let start = DragStart {
                            fraction: pointer_handle
                                .constraints
                                .clamp(pointer_handle.fraction.get(), extent),
                            position: if pointer_handle.axis == SplitAxis::Horizontal {
                                event.x
                            } else {
                                event.y
                            },
                            extent,
                        };
                        owner.set_attachment(START, start);
                        if let Some(capture) = PointerCaptureHolder::current() {
                            capture.acquire(&owner, true);
                        }
                    }
                    PointerPhase::Up => pointer_handle.release(&owner),
                }
                EventResult::Handled
            }),
        );

        let motion_node = node.downgrade();
        let motion_handle = SplitHandle::clone_for_actions(self);
        registry.set_motion(
            node,
            InputHandlerRoute::control(),
            Rc::new(move |event, _| {
                let Some(owner) = Node::upgrade(&motion_node) else {
                    return EventResult::Ignored;
                };
                if !PointerCaptureHolder::current()
                    .and_then(|capture| capture.target())
                    .is_some_and(|target| target == owner)
                {
                    return EventResult::Ignored;
                }
                let Some(start) = owner.attachment::<DragStart>(START) else {
                    return EventResult::Ignored;
                };
                if start.extent <= 1.0 {
                    return EventResult::Ignored;
                }
                let position = if motion_handle.axis == SplitAxis::Horizontal {
                    event.x
                } else {
                    event.y
                };
                motion_handle.write(
                    &owner,
                    start.fraction + (position - start.position) / start.extent,
                );
                EventResult::Handled
            }),
        );

        let key_node = node.downgrade();
        let key_handle = SplitHandle::clone_for_actions(self);
        registry.set_key(
            node,
            InputHandlerRoute::control(),
            Rc::new(move |event, phase| {
                if phase != EventPhase::Target {
                    return EventResult::Ignored;
                }
                if event.modifiers.has_ctrl()
                    || event.modifiers.has_gui()
                    || event.modifiers.has_alt()
                {
                    return EventResult::Ignored;
                }
                let Some(owner) = Node::upgrade(&key_node) else {
                    return EventResult::Ignored;
                };
                let step = 0.02;
                let delta = match key_handle.axis {
                    SplitAxis::Horizontal => match event.scancode {
                        crate::Scancode::ARROW_LEFT => -step,
                        crate::Scancode::ARROW_RIGHT => step,
                        _ => return EventResult::Ignored,
                    },
                    SplitAxis::Vertical => match event.scancode {
                        crate::Scancode::ARROW_UP => -step,
                        crate::Scancode::ARROW_DOWN => step,
                        _ => return EventResult::Ignored,
                    },
                };
                key_handle.adjust(&owner, delta);
                if let Some(chain) = FocusChainHolder::current() {
                    chain.focus(Some(&owner), true);
                }
                EventResult::Handled
            }),
        );
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        Vec::new()
    }
}
impl_primitive_view!(SplitHandle);

impl SplitHandle {
    /// A touch surface needs the full 44pt grab extent; the painted line stays a
    /// hairline. Swift reaches the same extent through `minimumControlExtent`.
    fn resolve_thickness(&self, node: &Node) -> f32 {
        let environment = node.composition_value(&crate::adaptive::SurfaceEnvironment::key());
        if environment
            .input
            .contains(cove_core::InputCapabilities::TOUCH)
        {
            self.thickness.max(environment.minimum_control_extent())
        } else {
            self.thickness
        }
    }

    fn clone_for_actions(&self) -> SplitHandle {
        SplitHandle {
            resolved_thickness: std::cell::Cell::new(self.resolved_thickness.get()),
            axis: self.axis,
            thickness: self.thickness,
            color: self.color,
            fraction: self.fraction.clone(),
            constraints: self.constraints,
            spacing: self.spacing,
        }
    }
}

/// Releases capture and clears focus when the divider leaves the tree.
struct SplitHandleLifetime {
    capture: RefCell<Option<Rc<cove_core::scene::PointerCapture>>>,
    chain: RefCell<Option<Rc<cove_core::scene::FocusChain>>>,
}

impl SplitHandleLifetime {
    fn new() -> Self {
        Self {
            capture: RefCell::new(None),
            chain: RefCell::new(None),
        }
    }
}

impl NodeResource for SplitHandleLifetime {
    fn mount(&self, _node: &Node) {
        *self.capture.borrow_mut() = PointerCaptureHolder::current();
        *self.chain.borrow_mut() = FocusChainHolder::current();
    }

    fn unmount(&self, node: &Node) {
        if let Some(capture) = self.capture.borrow().as_ref()
            && capture.target().is_some_and(|target| target == *node)
        {
            capture.release();
        }
        if let Some(chain) = self.chain.borrow().as_ref()
            && chain.focused().is_some_and(|focused| focused == *node)
        {
            chain.clear();
        }
    }

    fn as_any_rc(self: Rc<Self>) -> Rc<dyn std::any::Any> {
        self
    }
}
