//! Translation of Primitives/Controls/Fields/Numeric/Scrub.swift.
//!
//! The prefix chip of a numeric field. Dragging it horizontally scrubs the value
//! live; a double click resets to the field's reset value. The text field stays
//! the keyboard and assistive path, so the chip adds no focus stop.
use std::cell::RefCell;
use std::rc::Rc;

use crate::{AnyView, PrimitiveView, View, impl_primitive_view};
use cove_core::scene::{
    EventResult, InputHandlerRoute, InteractionRegistryHolder, LayoutNode, Node, NodeResource,
    PointerCapture, PointerCaptureHolder, PointerPhase,
};
use cove_platform::{KeyModifiers, MouseButton, SystemCursor};

/// Drag state attachment key.
pub const SCRUB_SESSION: &str = "numeric.scrub";

/// How a numeric prefix chip turns horizontal drag into value changes.
#[derive(Clone)]
pub struct NumericScrub {
    /// Value change per logical point of drag, before modifiers.
    pub per_point: f32,
    pub clamp: Rc<dyn Fn(f32) -> f32>,
    pub reset: Option<f32>,
}

impl NumericScrub {
    pub fn new(per_point: f32, reset: Option<f32>, clamp: impl Fn(f32) -> f32 + 'static) -> Self {
        Self {
            per_point: if per_point.is_finite() {
                per_point
            } else {
                0.0
            },
            clamp: Rc::new(clamp),
            reset,
        }
    }

    /// Shift is coarse (x10), Option/Alt is fine (x0.1). The rate is latched at
    /// drag start so a modifier pressed mid-drag cannot change the scale under
    /// the pointer.
    pub fn value(&self, start: f32, dragged: f32, modifiers: KeyModifiers) -> f32 {
        let mut rate = self.per_point;
        if modifiers.has_shift() {
            rate *= 10.0;
        }
        if modifiers.has_alt() {
            rate *= 0.1;
        }
        (self.clamp)(start + dragged * rate)
    }
}

/// Modifiers and position captured when the drag started.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrubStart {
    pub x: f32,
    pub value: f32,
    pub modifiers: KeyModifiers,
}

/// Drag state that survives recomposition while the value changes underneath it.
#[derive(Default)]
pub struct NumericScrubSession {
    pub start: RefCell<Option<ScrubStart>>,
    capture: RefCell<Option<Rc<PointerCapture>>>,
}

impl NumericScrubSession {
    pub fn new() -> Rc<Self> {
        Rc::new(Self::default())
    }

    pub fn begin(&self, at: f32, value: f32, modifiers: KeyModifiers, node: &Node) {
        *self.start.borrow_mut() = Some(ScrubStart {
            x: at,
            value,
            modifiers,
        });
        if let Some(capture) = self.capture.borrow().as_ref() {
            capture.acquire(node, true);
        }
    }

    /// Ends the drag; with a restore callback, writes the starting value back.
    pub fn cancel(&self, node: &Node, restoring: Option<&dyn Fn(f32)>) {
        let start = self.start.borrow_mut().take();
        if let (Some(restore), Some(start)) = (restoring, start) {
            restore(start.value);
        }
        if let Some(capture) = self.capture.borrow().as_ref()
            && capture.target().is_some_and(|target| target == *node)
        {
            capture.release();
        }
    }
}

impl NodeResource for NumericScrubSession {
    fn mount(&self, _node: &Node) {
        *self.capture.borrow_mut() = PointerCaptureHolder::current();
    }

    fn unmount(&self, node: &Node) {
        self.cancel(node, None);
    }

    fn as_any_rc(self: Rc<Self>) -> Rc<dyn std::any::Any> {
        self
    }
}

/// The prefix chip: a horizontal drag scrubber with a double-click reset.
///
/// It takes a plain value plus a write callback rather than a `Binding`: the
/// enclosing field carries theme-resolved color slots, and `Binding` requires
/// `Send + Sync`, which those closures are not.
pub struct NumericScrubHandle {
    pub value: f32,
    pub on_change: Rc<dyn Fn(f32)>,
    pub scrub: NumericScrub,
    pub is_enabled: bool,
    label: AnyView,
}

impl NumericScrubHandle {
    pub fn new(
        value: f32,
        on_change: impl Fn(f32) + 'static,
        scrub: NumericScrub,
        is_enabled: bool,
        label: impl View + 'static,
    ) -> Self {
        Self {
            value,
            on_change: Rc::new(on_change),
            scrub,
            is_enabled,
            label: AnyView::new(label),
        }
    }
}

impl PrimitiveView for NumericScrubHandle {
    fn make_node(&self) -> Node {
        let node = Node::new();
        let session: Rc<dyn NodeResource> = NumericScrubSession::new();
        node.add_resource(session.clone());
        if let Ok(session) = session
            .clone()
            .as_any_rc()
            .downcast::<NumericScrubSession>()
        {
            node.set_attachment(SCRUB_SESSION, session);
        }
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        // The chip is stretched by the field row; its label stays centered.
        let layout = LayoutNode::new();
        layout.set_flex_direction(cove_core::scene::FlexDirection::Row);
        layout.set_align_items(cove_core::scene::Align::Center);
        layout.set_justify_content(cove_core::scene::Justify::Center);
        // Tooltip and menu wrappers are columns; grow to their full height.
        layout.set_flex_grow(1.0);
        Some(layout)
    }

    fn update_node(&self, node: &Node) {
        let session = node.attachment::<Rc<NumericScrubSession>>(SCRUB_SESSION);
        node.set_is_hit_testable(self.is_enabled);
        node.set_cursor(Some(if self.is_enabled {
            SystemCursor::ResizeHorizontal
        } else {
            SystemCursor::Arrow
        }));
        let Some(registry) = InteractionRegistryHolder::current() else {
            return;
        };
        if !self.is_enabled {
            if let Some(session) = session.as_ref() {
                session.cancel(node, None);
            }
            registry.remove(node);
            return;
        }

        let value = self.value;
        let on_change = self.on_change.clone();
        let scrub = self.scrub.clone();
        let pointer_node = node.downgrade();
        let pointer_session = session.clone();
        registry.set_pointer(
            node,
            InputHandlerRoute::control(),
            Rc::new(move |event, phase, event_phase| {
                if event.button != MouseButton::Left
                    || event_phase != cove_core::scene::EventPhase::Target
                {
                    return EventResult::Ignored;
                }
                let Some(owner) = Node::upgrade(&pointer_node) else {
                    return EventResult::Ignored;
                };
                match phase {
                    PointerPhase::Down => {
                        // A double click resets; the click count arrives with the event.
                        if event.clicks >= 2
                            && let Some(reset) = scrub.reset
                        {
                            if let Some(session) = pointer_session.as_ref() {
                                session.cancel(&owner, None);
                            }
                            if value != reset {
                                on_change(reset);
                            }
                            return EventResult::Handled;
                        }
                        if let Some(session) = pointer_session.as_ref() {
                            session.begin(event.x, value, event.modifiers, &owner);
                        }
                    }
                    PointerPhase::Up => {
                        if let Some(session) = pointer_session.as_ref() {
                            session.cancel(&owner, None);
                        }
                    }
                }
                EventResult::Handled
            }),
        );

        let motion_on_change = self.on_change.clone();
        let motion_value = self.value;
        let motion_scrub = self.scrub.clone();
        let motion_session = session;
        let motion_node = node.downgrade();
        registry.set_motion(
            node,
            InputHandlerRoute::control(),
            Rc::new(move |event, phase| {
                if phase != cove_core::scene::EventPhase::Target {
                    return EventResult::Ignored;
                }
                let Some(owner) = Node::upgrade(&motion_node) else {
                    return EventResult::Ignored;
                };
                let Some(session) = motion_session.as_ref() else {
                    return EventResult::Ignored;
                };
                let start = *session.start.borrow();
                let Some(start) = start else {
                    return EventResult::Ignored;
                };
                let next = motion_scrub.value(start.value, event.x - start.x, start.modifiers);
                if motion_value != next {
                    motion_on_change(next);
                }
                let _ = owner;
                EventResult::Handled
            }),
        );
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![self.label.clone()]
    }
}
impl_primitive_view!(NumericScrubHandle);
