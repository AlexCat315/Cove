//! Translation of Animation/Interaction/SpringDrag.swift.
//!
//! Direct manipulation followed by a velocity-preserving spring release. The
//! offset is in the parent's logical points; input remains at the drawn frame.
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use cove_core::scene::{
    ActiveAnimationContext, AnimatorScheduler, AnyAnimationController, EventResult, InputHandlerRole,
    InputHandlerRoute, InputRoutingPriority, InteractionRegistryHolder, LayoutNode, LayoutUpdateHook,
    Node, NodeResource, PointerCaptureHolder,
};
use cove_core::scene::scrolling::ScrollVelocityTracker;
use cove_core::{Binding, Point};
use cove_platform::MouseButton;
use crate::{impl_primitive_view, AnyView, PrimitiveView, View};

/// Velocity samples older than this are ignored on release.
const STALE_SAMPLE_WINDOW: f64 = 0.12;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpringDragOptions {
    pub target: Point,
    pub spring: cove_core::scene::SpringSolution,
    pub is_enabled: bool,
}

/// Drag state, velocity tracking and spring release owned by the mounted
/// surface.
pub struct SpringDragSession {
    node: RefCell<Option<Node>>,
    scheduler: RefCell<Option<AnimatorScheduler>>,
    binding: RefCell<Option<Binding<Point>>>,
    spring: Cell<cove_core::scene::SpringSolution>,
    target: Cell<Point>,
    current: Cell<Point>,
    origin: Cell<Point>,
    press: Cell<Point>,
    start: Cell<Point>,
    release_point: Cell<Point>,
    velocity: Cell<Point>,
    tracker_x: RefCell<ScrollVelocityTracker>,
    tracker_y: RefCell<ScrollVelocityTracker>,
    elapsed: Cell<f64>,
    reduced: Cell<bool>,
    is_dragging: Cell<bool>,
    is_finished: Cell<bool>,
    controller: RefCell<Option<Rc<dyn AnyAnimationController>>>,
    self_ref: RefCell<std::rc::Weak<Self>>,
}

impl SpringDragSession {
    pub fn new() -> Rc<Self> {
        let session = Rc::new(Self {
            node: RefCell::new(None),
            scheduler: RefCell::new(None),
            binding: RefCell::new(None),
            spring: Cell::new(cove_core::scene::SpringSolution::new(0.4, 0.82)),
            target: Cell::new(Point::default()),
            current: Cell::new(Point::default()),
            origin: Cell::new(Point::default()),
            press: Cell::new(Point::default()),
            start: Cell::new(Point::default()),
            release_point: Cell::new(Point::default()),
            velocity: Cell::new(Point::default()),
            tracker_x: RefCell::new(ScrollVelocityTracker::new(STALE_SAMPLE_WINDOW)),
            tracker_y: RefCell::new(ScrollVelocityTracker::new(STALE_SAMPLE_WINDOW)),
            elapsed: Cell::new(0.0),
            reduced: Cell::new(false),
            is_dragging: Cell::new(false),
            is_finished: Cell::new(true),
            controller: RefCell::new(None),
            self_ref: RefCell::new(std::rc::Weak::new()),
        });
        *session.self_ref.borrow_mut() = Rc::downgrade(&session);
        session
    }

    pub fn is_dragging(&self) -> bool {
        self.is_dragging.get()
    }

    pub fn is_finished(&self) -> bool {
        self.is_finished.get()
    }

    pub fn configure(&self, offset: Binding<Point>, options: SpringDragOptions, reduced: bool) {
        *self.binding.borrow_mut() = Some(offset.clone());
        self.target.set(options.target);
        self.spring.set(options.spring);
        self.reduced.set(reduced);
        if self.is_finished.get() && !self.is_dragging.get() {
            self.current.set(offset.get());
        }
        if !options.is_enabled || (reduced && !self.is_finished.get()) {
            self.finish_immediately();
        }
        self.place();
    }

    fn local(&self, point: Point) -> Point {
        let node = self.node.borrow().clone();
        match node.and_then(|node| node.parent()) {
            Some(parent) => parent.convert_from_window(point),
            None => point,
        }
    }

    pub fn begin(&self, point: Point) {
        self.cancel();
        self.is_dragging.set(true);
        let local = self.local(point);
        self.press.set(local);
        self.start.set(self.current.get());
        self.tracker_x.borrow_mut().reset();
        self.tracker_y.borrow_mut().reset();
        self.sample();
        if let Some(node) = self.node.borrow().as_ref()
            && let Some(capture) = PointerCaptureHolder::current()
        {
            capture.acquire(node, true);
        }
    }

    pub fn move_to(&self, point: Point) {
        if !self.is_dragging.get() {
            return;
        }
        let local = self.local(point);
        let start = self.start.get();
        let press = self.press.get();
        self.current.set(Point::new(start.x + local.x - press.x, start.y + local.y - press.y));
        self.sample();
        self.publish();
    }

    fn sample(&self) {
        let time = now();
        let current = self.current.get();
        self.tracker_x.borrow_mut().add(time, current.x);
        self.tracker_y.borrow_mut().add(time, current.y);
    }

    pub fn release(&self) {
        if !self.is_dragging.get() {
            return;
        }
        self.is_dragging.set(false);
        if let Some(capture) = PointerCaptureHolder::current() {
            capture.release();
        }
        let time = now();
        let vx = self.tracker_x.borrow_mut().velocity(Some(time), Some(STALE_SAMPLE_WINDOW));
        let vy = self.tracker_y.borrow_mut().velocity(Some(time), Some(STALE_SAMPLE_WINDOW));
        self.velocity.set(Point::new(vx, vy));
        self.release_point.set(self.current.get());
        self.elapsed.set(0.0);
        if self.reduced.get() {
            self.finish_immediately();
        } else {
            self.is_finished.set(false);
            let controller: Rc<dyn AnyAnimationController> =
                Rc::new(SpringDragController { session: self.session_weak() });
            *self.controller.borrow_mut() = Some(controller.clone());
            if let Some(scheduler) = self.scheduler.borrow().as_ref() {
                scheduler.register(controller);
            }
        }
    }

    fn tick(&self, delta_time: f64) {
        if self.is_finished.get() || !delta_time.is_finite() || delta_time < 0.0 {
            return;
        }
        self.elapsed.set(self.elapsed.get() + delta_time);
        let spring = self.spring.get();
        if self.elapsed.get() >= spring.settling_duration() {
            self.finish_immediately();
            return;
        }
        let t = self.elapsed.get() as f32;
        let release = self.release_point.get();
        let velocity = self.velocity.get();
        let target = self.target.get();
        self.current.set(Point::new(
            spring.position(t, release.x, velocity.x, target.x),
            spring.position(t, release.y, velocity.y, target.y),
        ));
        self.publish();
    }

    pub fn finish_immediately(&self) {
        let target = self.target.get();
        let changed = self.current.get() != target
            || self.binding.borrow().as_ref().is_some_and(|binding| binding.get() != target);
        self.current.set(target);
        self.cancel();
        if changed {
            self.publish();
        } else {
            self.place();
        }
    }

    pub fn cancel(&self) {
        self.is_finished.set(true);
        if let (Some(scheduler), Some(controller)) =
            (self.scheduler.borrow().as_ref(), self.controller.borrow().as_ref())
        {
            scheduler.unregister(controller);
        }
        self.controller.borrow_mut().take();
        if self.is_dragging.get() {
            self.is_dragging.set(false);
            if let Some(capture) = PointerCaptureHolder::current() {
                capture.release();
            }
        }
    }

    fn publish(&self) {
        if let Some(binding) = self.binding.borrow().as_ref() {
            binding.set(self.current.get());
        }
        self.place();
    }

    fn place(&self) {
        let Some(node) = self.node.borrow().clone() else { return };
        let origin = self.origin.get();
        let current = self.current.get();
        node.set_frame(cove_core::Rect::new(
            origin.x + current.x,
            origin.y + current.y,
            node.frame().width,
            node.frame().height,
        ));
    }

    fn session_weak(&self) -> std::rc::Weak<Self> {
        self.self_ref.borrow().clone()
    }
}

fn now() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs_f64())
        .unwrap_or(0.0)
}

struct SpringDragController {
    session: std::rc::Weak<SpringDragSession>,
}

impl AnyAnimationController for SpringDragController {
    fn is_finished(&self) -> bool {
        self.session.upgrade().is_none_or(|session| session.is_finished())
    }

    fn tick(&self, delta_time: f64) {
        if let Some(session) = self.session.upgrade() {
            session.tick(delta_time);
        }
    }

    fn finish_immediately(&self) {
        if let Some(session) = self.session.upgrade() {
            session.finish_immediately();
        }
    }

    fn cancel(&self) {
        if let Some(session) = self.session.upgrade() {
            session.cancel();
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl NodeResource for SpringDragSession {
    fn mount(&self, node: &Node) {
        *self.node.borrow_mut() = Some(node.clone());
        *self.scheduler.borrow_mut() = Some(AnimatorScheduler::current());
        let weak_node = node.downgrade();
        let weak_session = self.session_weak();
        node.set_layout_did_update(Some(Rc::new(move |owner: &Node| {
            let (Some(_owner), Some(session)) = (Node::upgrade(&weak_node), weak_session.upgrade()) else {
                return;
            };
            session.origin.set(cove_core::Point::new(owner.frame().x, owner.frame().y));
            session.place();
        }) as LayoutUpdateHook));
    }

    fn unmount(&self, _node: &Node) {
        self.cancel();
        self.binding.borrow_mut().take();
        *self.node.borrow_mut() = None;
    }

    fn as_any_rc(self: Rc<Self>) -> Rc<dyn std::any::Any> {
        self
    }
}

/// Drag-to-offset surface with a velocity-preserving spring release.
pub struct SpringDrag {
    pub offset: Binding<Point>,
    pub target: Point,
    pub spring: cove_core::scene::SpringSolution,
    pub is_enabled: bool,
    content: AnyView,
}

impl SpringDrag {
    pub fn new(offset: Binding<Point>, content: impl View + 'static) -> Self {
        Self {
            offset,
            target: Point::default(),
            spring: cove_core::scene::SpringSolution::new(0.4, 0.82),
            is_enabled: true,
            content: AnyView::new(content),
        }
    }

    pub fn resting_at(mut self, target: Point) -> Self {
        self.target = target;
        self
    }

    pub fn spring(mut self, spring: cove_core::scene::SpringSolution) -> Self {
        self.spring = spring;
        self
    }

    pub fn enabled(mut self, is_enabled: bool) -> Self {
        self.is_enabled = is_enabled;
        self
    }
}

impl PrimitiveView for SpringDrag {
    fn make_node(&self) -> Node {
        let node = Node::new();
        let session: Rc<dyn NodeResource> = SpringDragSession::new();
        node.add_resource(session);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        Some(LayoutNode::new())
    }

    fn update_layout(&self, layout: &LayoutNode) {
        layout.set_flex_direction(cove_core::scene::FlexDirection::Column);
        layout.set_align_items(cove_core::scene::Align::Stretch);
    }

    fn update_node(&self, node: &Node) {
        let Some(session) = node.first_resource::<SpringDragSession>() else { return };
        let options = SpringDragOptions { target: self.target, spring: self.spring, is_enabled: self.is_enabled };
        session.configure(self.offset.clone(), options, node.prefers_reduced_motion());
        let _ = ActiveAnimationContext::current();
        let Some(registry) = InteractionRegistryHolder::current() else { return };
        let route = InputHandlerRoute::new(
            InputHandlerRole::Drag, InputRoutingPriority::CHROME, "spring-drag",
        );
        let is_enabled = self.is_enabled;
        let pointer_session = session.clone();
        registry.set_pointer(node, route.clone(), Rc::new(move |event, phase, _event_phase| {
            if !is_enabled || event.button != MouseButton::Left {
                return EventResult::Ignored;
            }
            match phase {
                cove_core::scene::PointerPhase::Down => {
                    pointer_session.begin(Point::new(event.x, event.y));
                }
                cove_core::scene::PointerPhase::Up => pointer_session.release(),
            }
            EventResult::Handled
        }));
        let motion_session = session;
        registry.set_motion(node, route, Rc::new(move |event, _phase| {
            if !motion_session.is_dragging() {
                return EventResult::Ignored;
            }
            motion_session.move_to(Point::new(event.x, event.y));
            EventResult::Handled
        }));
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![self.content.clone()]
    }
}
impl_primitive_view!(SpringDrag);
