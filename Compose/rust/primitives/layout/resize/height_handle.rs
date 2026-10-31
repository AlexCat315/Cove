//! Translation of Primitives/Layout/Resize/HeightHandle.swift.
//!
//! A dedicated lane keeps resizing out of the editable text's hit region.
use std::cell::RefCell;
use std::rc::Rc;

use cove_core::Binding;
use cove_core::scene::{
    AccessibilityActions, AccessibilityRole, AccessibilitySemantics, Display, EventPhase,
    EventResult, FocusChainHolder, HoverPhase, InputHandlerRoute, InteractionRegistryHolder,
    LayoutNode, Node, NodeResource, PointerCaptureAttachmentKey, PointerCaptureHolder,
    PointerPhase,
};
use cove_platform::{MouseButton, SystemCursor};

use super::height_limits::EditorHeightLimits;
use crate::modifiers::interaction::CursorExt;
use crate::primitives::layout::scroll_reveal::ScrollReveal;
use crate::theme::theme_for_node;
use crate::{PrimitiveView, View, impl_primitive_view};

const START: &str = "editor.resize.start";
const BASIS: &str = "editor.resize.basis";
const HOVER: &str = "editor.resize.hover";
const REVEAL: &str = "editor.resize.reveal";

#[derive(Clone, Copy, Debug, PartialEq)]
struct ResizeStart {
    y: f32,
    height: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct ResizeBasis {
    minimum: f32,
    maximum: f32,
    extent: f32,
}

/// Drag, keyboard and accessibility resize lane for an editor's lower edge.
pub struct EditorHeightHandle {
    pub value: Binding<f32>,
    pub limits: EditorHeightLimits,
    pub label: String,
    pub is_enabled: bool,
    pub extent: f32,
}

impl EditorHeightHandle {
    pub fn new(
        value: Binding<f32>,
        limits: EditorHeightLimits,
        label: impl Into<String>,
        is_enabled: bool,
        extent: f32,
    ) -> Self {
        Self {
            value,
            limits,
            label: label.into(),
            is_enabled,
            extent,
        }
    }

    fn current(&self) -> f32 {
        self.limits.clamp(self.value.get())
    }

    fn write(&self, node: &Node, proposed: f32, reveal: bool) {
        if !node.accepts_subtree_input() || !proposed.is_finite() {
            return;
        }
        let next = self.limits.clamp(proposed);
        if self.value.get() != next {
            if reveal {
                node.set_attachment(REVEAL, true);
            }
            self.value.set(next);
        }
    }

    fn release(&self, node: &Node) {
        node.remove_attachment(START);
        node.remove_attachment(REVEAL);
        if PointerCaptureHolder::current()
            .and_then(|capture| capture.target())
            .is_some_and(|target| target == *node)
            && let Some(capture) = PointerCaptureHolder::current()
        {
            capture.release();
        }
        node.mark_render_dirty(cove_core::scene::InvalidationSource::style_set(
            "editor.resize",
        ));
    }
}

impl PrimitiveView for EditorHeightHandle {
    fn make_node(&self) -> Node {
        let node = Node::new();
        let lifetime: Rc<dyn NodeResource> = Rc::new(EditorHeightHandleLifetime::new());
        node.add_resource(lifetime);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        Some(LayoutNode::new())
    }

    fn update_layout(&self, layout: &LayoutNode) {
        layout.set_display(if self.is_enabled {
            Display::Flex
        } else {
            Display::None
        });
        layout.set_height(Some(self.extent));
        layout.set_flex_shrink(0.0);
    }

    fn update_node(&self, node: &Node) {
        let basis = ResizeBasis {
            minimum: self.limits.minimum,
            maximum: self.limits.maximum,
            extent: self.extent,
        };
        if node.attachment::<ResizeBasis>(BASIS) != Some(basis) {
            self.release(node);
        }
        node.set_attachment(BASIS, basis);

        node.set_is_hit_testable(self.is_enabled);
        node.set_is_focusable(self.is_enabled);
        node.set_cursor(Some(if self.is_enabled {
            SystemCursor::ResizeVertical
        } else {
            SystemCursor::NotAllowed
        }));

        let current = self.current();
        node.set_accessibility(Some(AccessibilitySemantics {
            role: AccessibilityRole::Slider,
            label: self.label.clone(),
            value: format!("{} pt", current.round() as i64),
            help: "Use Up/Down to resize, Shift for larger steps, Home/End for the limits"
                .to_string(),
            ..Default::default()
        }));

        // Reveal the handle after a layout change if a write requested it.
        let reveal_node = node.downgrade();
        node.set_layout_did_update(Some(Rc::new(move |node| {
            if node.attachment::<bool>(REVEAL) == Some(true) {
                node.remove_attachment(REVEAL);
                ScrollReveal::reveal(node, false);
            }
            let _ = reveal_node;
        })));

        let clear_node = node.downgrade();
        node.set_attachment(
            PointerCaptureAttachmentKey::DID_LOSE_CAPTURE,
            Rc::new(move || {
                if let Some(owner) = Node::upgrade(&clear_node) {
                    owner.remove_attachment(START);
                    owner.mark_render_dirty(cove_core::scene::InvalidationSource::style_set(
                        "editor.resize",
                    ));
                }
            }) as Rc<dyn Fn()>,
        );

        let weak = node.downgrade();
        let is_enabled = self.is_enabled;
        node.update_draw(
            (
                basis.minimum.to_bits(),
                basis.maximum.to_bits(),
                basis.extent.to_bits(),
            ),
            move |list, origin| {
                let Some(owner) = Node::upgrade(&weak) else {
                    return;
                };
                if !owner.is_layout_visible() {
                    return;
                }
                let chain = FocusChainHolder::current();
                let focused = chain
                    .as_ref()
                    .and_then(|chain| chain.focused())
                    .is_some_and(|current| current == owner)
                    && chain.as_ref().is_some_and(|chain| chain.is_focus_visible());
                let hot = is_enabled
                    && (owner.attachment::<bool>(HOVER) == Some(true)
                        || owner.attachment::<ResizeStart>(START).is_some());
                let frame = owner.frame();
                let theme = theme_for_node(&owner);
                let opacity = owner.opacity();
                if hot {
                    let _ = list.add_rounded_rect(
                        cove_core::Rect::new(origin.x, origin.y, frame.width, frame.height),
                        4.0,
                        theme.colors.state_layer_hover.multiplied_alpha(opacity),
                    );
                }
                if focused {
                    let _ = list.add_rounded_rect_stroke(
                        cove_core::Rect::new(
                            origin.x + 2.0,
                            origin.y + 2.0,
                            (frame.width - 4.0).max(0.0),
                            (frame.height - 4.0).max(0.0),
                        ),
                        4.0,
                        1.0,
                        theme.colors.focus_ring.multiplied_alpha(opacity),
                    );
                }
                let grip_width = frame.width.min(28.0);
                let grip = cove_core::Rect::new(
                    origin.x + (frame.width - grip_width).max(0.0) / 2.0,
                    origin.y + (frame.height - 2.0) / 2.0,
                    grip_width,
                    2.0,
                );
                let grip_color = if focused || hot {
                    theme.colors.accent
                } else {
                    theme.colors.on_surface_muted
                };
                let alpha = opacity * if is_enabled { 0.55 } else { 0.25 };
                let _ = list.add_rounded_rect(grip, 1.0, grip_color.multiplied_alpha(alpha));
            },
        );

        if !self.is_enabled {
            self.release(node);
            if let Some(chain) = FocusChainHolder::current()
                && chain.focused().is_some_and(|focused| focused == *node)
            {
                chain.clear();
            }
            node.remove_attachment(HOVER);
            if let Some(registry) = InteractionRegistryHolder::current() {
                registry.remove(node);
            }
            return;
        }

        node.set_accessibility_actions(AccessibilityActions::default());

        let increment_node = node.downgrade();
        let increment_binding = self.value.clone();
        let increment_limits = self.limits;
        let decrement_node = node.downgrade();
        let decrement_binding = self.value.clone();
        let decrement_limits = self.limits;
        let set_node = node.downgrade();
        let set_binding = self.value.clone();
        let set_limits = self.limits;
        node.set_accessibility_actions(AccessibilityActions {
            increment: Some(Rc::new(move || {
                if let Some(owner) = Node::upgrade(&increment_node) {
                    let next = increment_limits.clamp(increment_binding.get() + 16.0);
                    if increment_binding.get() != next {
                        owner.set_attachment(REVEAL, true);
                        increment_binding.set(next);
                    }
                }
            })),
            decrement: Some(Rc::new(move || {
                if let Some(owner) = Node::upgrade(&decrement_node) {
                    let next = decrement_limits.clamp(decrement_binding.get() - 16.0);
                    if decrement_binding.get() != next {
                        owner.set_attachment(REVEAL, true);
                        decrement_binding.set(next);
                    }
                }
            })),
            set_value: Some(Rc::new(move |text: &str| {
                let Ok(next) = text.trim().parse::<f32>() else {
                    return;
                };
                if !next.is_finite() {
                    return;
                }
                if let Some(owner) = Node::upgrade(&set_node) {
                    let next = set_limits.clamp(next);
                    if set_binding.get() != next {
                        owner.set_attachment(REVEAL, true);
                        set_binding.set(next);
                    }
                }
            })),
            ..Default::default()
        });

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
                let hovered = phase == HoverPhase::Enter
                    && crate::primitives::controls::adaptive::supports_control_hover(&owner);
                owner.set_attachment(HOVER, hovered);
                owner.mark_render_dirty(cove_core::scene::InvalidationSource::style_set(
                    "editor.resize",
                ));
            }),
        );

        let pointer_node = node.downgrade();
        let pointer_binding = self.value.clone();
        let pointer_limits = self.limits;
        let pointer_self_is_enabled = self.is_enabled;
        registry.set_pointer(
            node,
            InputHandlerRoute::control(),
            Rc::new(move |event, phase, event_phase| {
                if event_phase != EventPhase::Target || event.button != MouseButton::Left {
                    return EventResult::Ignored;
                }
                let Some(owner) = Node::upgrade(&pointer_node) else {
                    return EventResult::Ignored;
                };
                if !owner.accepts_subtree_input() {
                    return EventResult::Ignored;
                }
                match phase {
                    PointerPhase::Down => {
                        if let Some(chain) = FocusChainHolder::current() {
                            chain.focus(Some(&owner), false);
                        }
                        owner.set_attachment(
                            START,
                            ResizeStart {
                                y: event.y,
                                height: pointer_limits.clamp(pointer_binding.get()),
                            },
                        );
                        if let Some(capture) = PointerCaptureHolder::current() {
                            capture.acquire(&owner, true);
                        }
                    }
                    PointerPhase::Up => {
                        if pointer_self_is_enabled {
                            let handle = EditorHeightHandle::new(
                                pointer_binding.clone(),
                                pointer_limits,
                                "",
                                pointer_self_is_enabled,
                                0.0,
                            );
                            handle.release(&owner);
                        }
                    }
                }
                owner.mark_render_dirty(cove_core::scene::InvalidationSource::style_set(
                    "editor.resize",
                ));
                EventResult::Handled
            }),
        );

        let motion_node = node.downgrade();
        let motion_limits = self.limits;
        let motion_binding = self.value.clone();
        let motion_is_enabled = self.is_enabled;
        registry.set_motion(
            node,
            InputHandlerRoute::control(),
            Rc::new(move |event, phase| {
                if phase != EventPhase::Target {
                    return EventResult::Ignored;
                }
                let Some(owner) = Node::upgrade(&motion_node) else {
                    return EventResult::Ignored;
                };
                if !PointerCaptureHolder::current()
                    .and_then(|capture| capture.target())
                    .is_some_and(|target| target == owner)
                {
                    return EventResult::Ignored;
                }
                let Some(start) = owner.attachment::<ResizeStart>(START) else {
                    return EventResult::Ignored;
                };
                let handle = EditorHeightHandle::new(
                    motion_binding.clone(),
                    motion_limits,
                    "",
                    motion_is_enabled,
                    0.0,
                );
                handle.write(&owner, start.height + event.y - start.y, false);
                EventResult::Handled
            }),
        );

        let key_node = node.downgrade();
        let key_binding = self.value.clone();
        let key_limits = self.limits;
        let key_is_enabled = self.is_enabled;
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
                let step = if event.modifiers.has_shift() {
                    64.0
                } else {
                    16.0
                };
                let current = key_binding.get();
                let next = match event.scancode {
                    crate::Scancode::ARROW_UP => current - step,
                    crate::Scancode::ARROW_DOWN => current + step,
                    crate::Scancode::PAGE_UP => current - 64.0,
                    crate::Scancode::PAGE_DOWN => current + 64.0,
                    crate::Scancode::HOME => key_limits.minimum,
                    crate::Scancode::END => key_limits.maximum,
                    crate::Scancode::ESCAPE => {
                        let Some(start) = owner.attachment::<ResizeStart>(START) else {
                            return EventResult::Ignored;
                        };
                        let handle = EditorHeightHandle::new(
                            key_binding.clone(),
                            key_limits,
                            "",
                            key_is_enabled,
                            0.0,
                        );
                        handle.write(&owner, start.height, true);
                        handle.release(&owner);
                        return EventResult::Handled;
                    }
                    _ => return EventResult::Ignored,
                };
                let handle = EditorHeightHandle::new(
                    key_binding.clone(),
                    key_limits,
                    "",
                    key_is_enabled,
                    0.0,
                );
                handle.write(&owner, next, true);
                if let Some(chain) = FocusChainHolder::current() {
                    chain.focus(Some(&owner), true);
                }
                EventResult::Handled
            }),
        );
    }
}
impl_primitive_view!(EditorHeightHandle);

/// Releases capture and clears focus when the lane leaves the tree.
struct EditorHeightHandleLifetime {
    capture: RefCell<Option<Rc<cove_core::scene::PointerCapture>>>,
    chain: RefCell<Option<Rc<cove_core::scene::FocusChain>>>,
}

impl EditorHeightHandleLifetime {
    fn new() -> Self {
        Self {
            capture: RefCell::new(None),
            chain: RefCell::new(None),
        }
    }
}

impl NodeResource for EditorHeightHandleLifetime {
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
