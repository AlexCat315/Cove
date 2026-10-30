//! Translation of Primitives/Interaction/HorizontalSwipe.swift.
//!
//! Observes the capture phase so a nested button can still receive a tap.
//! Horizontal intent may cancel a button press; sliders and text selections
//! retain their capture and are never taken over by paging.
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use cove_core::scene::{
    EventPhase, EventResult, InputHandlerRoute, InteractionRegistryHolder, LayoutNode, Node,
    NodeResource, PointerCaptureAttachmentKey, PointerCaptureHolder, PointerPhase,
};
use cove_core::Point;
use cove_platform::MouseButton;
use crate::{impl_primitive_view, AnyView, PrimitiveView, View};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HorizontalSwipeDirection {
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Gesture {
    Idle,
    Observing(Point),
    Horizontal(Point),
    Rejected,
}

/// Pointer session that turns a horizontal drag into one page turn.
pub struct HorizontalSwipeSession {
    node: RefCell<Option<Node>>,
    capture: RefCell<Option<Rc<cove_core::scene::PointerCapture>>>,
    gesture: Cell<Gesture>,
    is_enabled: Cell<bool>,
    on_swipe: RefCell<Option<Rc<dyn Fn(HorizontalSwipeDirection)>>>,
    should_begin: RefCell<Option<Rc<dyn Fn(&cove_platform::MouseButtonEvent, &Node) -> bool>>>,
}

impl HorizontalSwipeSession {
    fn new() -> Rc<Self> {
        Rc::new(Self {
            node: RefCell::new(None),
            capture: RefCell::new(None),
            gesture: Cell::new(Gesture::Idle),
            is_enabled: Cell::new(true),
            on_swipe: RefCell::new(None),
            should_begin: RefCell::new(None),
        })
    }

    pub fn configure(
        &self,
        is_enabled: bool,
        should_begin: Option<Rc<dyn Fn(&cove_platform::MouseButtonEvent, &Node) -> bool>>,
        on_swipe: Rc<dyn Fn(HorizontalSwipeDirection)>,
    ) {
        self.is_enabled.set(is_enabled);
        *self.on_swipe.borrow_mut() = Some(on_swipe);
        *self.should_begin.borrow_mut() = should_begin;
        if !is_enabled {
            self.cancel_gesture();
        }
    }

    fn pointer(&self, event: &cove_platform::MouseButtonEvent, phase: PointerPhase) -> EventResult {
        let Some(node) = self.node.borrow().clone() else { return EventResult::Ignored };
        if !self.is_enabled.get() {
            return EventResult::Ignored;
        }
        let point = Point::new(event.x, event.y);
        match phase {
            PointerPhase::Down => {
                self.cancel_gesture();
                let allow = self
                    .should_begin
                    .borrow()
                    .as_ref()
                    .is_none_or(|should_begin| should_begin(event, &node));
                if allow {
                    self.gesture.set(Gesture::Observing(point));
                }
                EventResult::Ignored
            }
            PointerPhase::Up => {
                let previous = self.gesture.get();
                let owned_capture = PointerCaptureHolder::current()
                    .and_then(|capture| capture.target())
                    .is_some_and(|target| target == node);
                self.cancel_gesture();
                let Gesture::Horizontal(start) = previous else { return EventResult::Ignored };
                if !owned_capture {
                    return EventResult::Ignored;
                }
                let distance = point.x - start.x;
                let threshold = 32.0_f32.max(node.frame().width * 0.15);
                if distance.abs() >= threshold
                    && let Some(on_swipe) = self.on_swipe.borrow().as_ref()
                {
                    on_swipe(if distance < 0.0 {
                        HorizontalSwipeDirection::Left
                    } else {
                        HorizontalSwipeDirection::Right
                    });
                }
                EventResult::Handled
            }
        }
    }

    fn motion(&self, event: &cove_platform::MouseMotionEvent) -> EventResult {
        let Some(node) = self.node.borrow().clone() else { return EventResult::Ignored };
        if !self.is_enabled.get() {
            return EventResult::Ignored;
        }
        match self.gesture.get() {
            Gesture::Observing(start) => {
                let dx = event.x - start.x;
                let dy = event.y - start.y;
                if dx.abs().max(dy.abs()) < 8.0 {
                    return EventResult::Ignored;
                }
                if dx.abs() <= dy.abs() {
                    self.gesture.set(Gesture::Rejected);
                    return EventResult::Ignored;
                }
                if let Some(captured) = PointerCaptureHolder::current().and_then(|capture| capture.target())
                    && captured != node
                {
                    let mut is_ancestor = false;
                    let mut cursor = Some(captured.clone());
                    while let Some(current) = cursor {
                        if current == node {
                            is_ancestor = true;
                            break;
                        }
                        cursor = current.parent();
                    }
                    let did_lose = captured
                        .attachment::<Rc<dyn Fn()>>(PointerCaptureAttachmentKey::DID_LOSE_CAPTURE)
                        .is_some();
                    if !is_ancestor || !did_lose {
                        self.gesture.set(Gesture::Rejected);
                        return EventResult::Ignored;
                    }
                }
                self.gesture.set(Gesture::Horizontal(start));
                if let Some(capture) = PointerCaptureHolder::current() {
                    capture.acquire(&node, true);
                }
                EventResult::Handled
            }
            Gesture::Horizontal(_) => {
                if !PointerCaptureHolder::current()
                    .and_then(|capture| capture.target())
                    .is_some_and(|target| target == node)
                {
                    self.gesture.set(Gesture::Rejected);
                    return EventResult::Ignored;
                }
                EventResult::Handled
            }
            Gesture::Idle | Gesture::Rejected => EventResult::Ignored,
        }
    }

    pub fn pointer_left(&self) {
        if matches!(self.gesture.get(), Gesture::Observing(_)) {
            self.cancel_gesture();
        }
    }

    /// Cancels the in-flight gesture; returns whether a horizontal drag owned it.
    pub fn cancel_gesture(&self) -> bool {
        let was_dragging = matches!(self.gesture.get(), Gesture::Horizontal(_));
        self.gesture.set(Gesture::Idle);
        if let Some(node) = self.node.borrow().clone()
            && PointerCaptureHolder::current()
                .and_then(|capture| capture.target())
                .is_some_and(|target| target == node)
            && let Some(capture) = PointerCaptureHolder::current()
        {
            capture.release();
        }
        was_dragging
    }
}

impl NodeResource for HorizontalSwipeSession {
    fn mount(&self, node: &Node) {
        *self.node.borrow_mut() = Some(node.clone());
        *self.capture.borrow_mut() = PointerCaptureHolder::current();
    }

    fn unmount(&self, _node: &Node) {
        self.cancel_gesture();
        *self.node.borrow_mut() = None;
        *self.on_swipe.borrow_mut() = None;
        *self.should_begin.borrow_mut() = None;
        *self.capture.borrow_mut() = None;
    }

    fn as_any_rc(self: Rc<Self>) -> Rc<dyn std::any::Any> {
        self
    }
}

/// Host that observes horizontal page swipes over its content.
pub struct HorizontalSwipe {
    pub on_swipe: Rc<dyn Fn(HorizontalSwipeDirection)>,
    pub is_enabled: bool,
    pub should_begin: Option<Rc<dyn Fn(&cove_platform::MouseButtonEvent, &Node) -> bool>>,
    content: AnyView,
}

impl HorizontalSwipe {
    pub fn new(on_swipe: impl Fn(HorizontalSwipeDirection) + 'static, content: impl View + 'static) -> Self {
        Self {
            on_swipe: Rc::new(on_swipe),
            is_enabled: true,
            should_begin: None,
            content: AnyView::new(content),
        }
    }

    pub fn enabled(mut self, is_enabled: bool) -> Self {
        self.is_enabled = is_enabled;
        self
    }

    pub fn should_begin(
        mut self,
        should_begin: impl Fn(&cove_platform::MouseButtonEvent, &Node) -> bool + 'static,
    ) -> Self {
        self.should_begin = Some(Rc::new(should_begin));
        self
    }
}

impl PrimitiveView for HorizontalSwipe {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_clips_to_bounds(true);
        node.set_attachment("horizontal-swipe-host", true);
        let session: Rc<dyn NodeResource> = HorizontalSwipeSession::new();
        node.add_resource(session);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        Some(LayoutNode::new())
    }

    fn update_node(&self, node: &Node) {
        let Some(session) = node.first_resource::<HorizontalSwipeSession>() else { return };
        session.configure(self.is_enabled, self.should_begin.clone(), self.on_swipe.clone());
        let Some(registry) = InteractionRegistryHolder::current() else { return };
        // Paging is a scroll gesture, not a control: a control route here would
        // make every label inside the page read as "inside a control" and stop
        // being selectable.
        let route = InputHandlerRoute::scroll();
        let pointer_session = session.clone();
        registry.set_pointer(node, route.clone(), Rc::new(move |event, phase, event_phase| {
            if event_phase == EventPhase::Bubble || event.button != MouseButton::Left {
                return EventResult::Ignored;
            }
            pointer_session.pointer(event, phase)
        }));
        let motion_session = session.clone();
        registry.set_motion(node, route, Rc::new(move |event, phase| {
            if phase == EventPhase::Bubble {
                return EventResult::Ignored;
            }
            motion_session.motion(event)
        }));
        let hover_session = session.clone();
        registry.set_hover(node, Rc::new(move |phase| {
            if phase == cove_core::scene::HoverPhase::Leave {
                hover_session.pointer_left();
            }
        }));
        let key_session = session;
        registry.set_key(node, InputHandlerRoute::scroll(), Rc::new(move |event, phase| {
            if phase == EventPhase::Capture || event.scancode != crate::Scancode::ESCAPE {
                return EventResult::Ignored;
            }
            if key_session.cancel_gesture() { EventResult::Handled } else { EventResult::Ignored }
        }));
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![self.content.clone()]
    }
}
impl_primitive_view!(HorizontalSwipe);
