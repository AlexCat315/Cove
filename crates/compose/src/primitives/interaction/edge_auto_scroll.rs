//! Translation of Primitives/Interaction/EdgeAutoScroll.swift.
//!
//! Scrolls the enclosing scroll view while a drag holds the pointer near its
//! top or bottom edge: faster the deeper into the edge zone, and still moving
//! when the pointer rests there. Attached to the drag source's node, it ticks
//! only between `begin` and `end`.
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use cove_core::Point;
use cove_core::scene::{AnimatorScheduler, AnyAnimationController, Node, NodeResource, Rect};

/// Points per second at the very edge.
pub const MAXIMUM_SPEED: f32 = 1200.0;

/// Signed speed for a pointer at `pointer_y`: zero outside the edge zones,
/// rising quadratically to the maximum at (or beyond) the edge.
pub fn velocity(pointer_y: f32, viewport: Rect) -> f32 {
    let zone = 48.0_f32.min(viewport.height / 4.0);
    if zone <= 0.0 {
        return 0.0;
    }
    let top = pointer_y - viewport.y;
    let bottom = viewport.y + viewport.height - pointer_y;
    if top < zone {
        let depth = (1.0_f32).min((zone - top) / zone);
        return -MAXIMUM_SPEED * depth * depth;
    }
    if bottom < zone {
        let depth = (1.0_f32).min((zone - bottom) / zone);
        return MAXIMUM_SPEED * depth * depth;
    }
    0.0
}

/// Nearest scrollable ancestor, matching `EnclosingScroll.container(of:)`.
fn enclosing_scroll(node: &Node) -> Option<Node> {
    let mut cursor = node.parent();
    while let Some(current) = cursor {
        if current.is_scroll_viewport() {
            return Some(current);
        }
        cursor = current.parent();
    }
    None
}

struct EdgeAutoScrollState {
    owner: RefCell<Option<Node>>,
    scroll: RefCell<Option<Node>>,
    pointer_y: Cell<f32>,
    start_offset: Cell<f32>,
    on_scroll: RefCell<Option<Rc<dyn Fn()>>>,
    is_finished: Cell<bool>,
}

/// Scroll-while-dragging driver for the enclosing scroll view.
pub struct EdgeAutoScroll {
    state: Rc<EdgeAutoScrollState>,
    scheduler: RefCell<Option<AnimatorScheduler>>,
    controller: RefCell<Option<Rc<dyn AnyAnimationController>>>,
}

impl EdgeAutoScroll {
    pub fn new() -> Rc<Self> {
        Rc::new(Self {
            state: Rc::new(EdgeAutoScrollState {
                owner: RefCell::new(None),
                scroll: RefCell::new(None),
                pointer_y: Cell::new(0.0),
                start_offset: Cell::new(0.0),
                on_scroll: RefCell::new(None),
                is_finished: Cell::new(true),
            }),
            scheduler: RefCell::new(None),
            controller: RefCell::new(None),
        })
    }

    /// Distance scrolled since `begin`; drags add it to the pointer delta so
    /// positions stay in content coordinates.
    pub fn scrolled_distance(&self) -> f32 {
        match self.state.scroll.borrow().as_ref() {
            Some(scroll) => scroll.content_offset().y - self.state.start_offset.get(),
            None => 0.0,
        }
    }

    pub fn is_finished(&self) -> bool {
        self.state.is_finished.get()
    }

    /// Starts tracking; `on_scroll` runs after every automatic scroll step.
    pub fn begin(self: &Rc<Self>, pointer_y: f32, on_scroll: Rc<dyn Fn()>) {
        let Some(owner) = self.state.owner.borrow().clone() else {
            return;
        };
        let Some(scroll) = enclosing_scroll(&owner) else {
            return;
        };
        self.state.start_offset.set(scroll.content_offset().y);
        self.state.pointer_y.set(pointer_y);
        *self.state.scroll.borrow_mut() = Some(scroll);
        *self.state.on_scroll.borrow_mut() = Some(on_scroll);
        if self.state.is_finished.get() {
            self.state.is_finished.set(false);
            let controller: Rc<dyn AnyAnimationController> = self.clone();
            *self.controller.borrow_mut() = Some(controller.clone());
            if let Some(scheduler) = self.scheduler.borrow().as_ref() {
                scheduler.register(controller);
            }
        }
    }

    pub fn update(&self, pointer_y: f32) {
        self.state.pointer_y.set(pointer_y);
    }

    pub fn end(&self) {
        *self.state.scroll.borrow_mut() = None;
        *self.state.on_scroll.borrow_mut() = None;
        if let (Some(scheduler), Some(controller)) = (
            self.scheduler.borrow().as_ref(),
            self.controller.borrow().as_ref(),
        ) {
            scheduler.unregister(controller);
        }
        self.controller.borrow_mut().take();
        self.state.is_finished.set(true);
    }
}

fn tick(state: &EdgeAutoScrollState, delta_time: f64) {
    if state.is_finished.get() || !delta_time.is_finite() || delta_time < 0.0 {
        return;
    }
    let Some(scroll) = state.scroll.borrow().clone() else {
        return;
    };
    let viewport = crate::primitives::layout::scroll_view::visible_viewport_rect(&scroll);
    let speed = velocity(state.pointer_y.get(), viewport);
    if speed == 0.0 {
        return;
    }
    let delta = speed * delta_time as f32;
    let offset = scroll.content_offset();
    let target = Point::new(offset.x, offset.y + delta);
    if target.y == offset.y {
        return;
    }
    scroll.set_content_offset(target);
    if let Some(on_scroll) = state.on_scroll.borrow().clone() {
        on_scroll();
    }
}

impl AnyAnimationController for EdgeAutoScroll {
    fn is_finished(&self) -> bool {
        self.state.is_finished.get()
    }

    fn tick(&self, delta_time: f64) {
        tick(&self.state, delta_time);
    }

    fn finish_immediately(&self) {
        self.end();
    }

    fn cancel(&self) {
        self.end();
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl NodeResource for EdgeAutoScroll {
    fn mount(&self, node: &Node) {
        *self.state.owner.borrow_mut() = Some(node.clone());
        *self.scheduler.borrow_mut() = Some(AnimatorScheduler::current());
    }

    fn unmount(&self, _node: &Node) {
        self.end();
        *self.state.owner.borrow_mut() = None;
        *self.scheduler.borrow_mut() = None;
    }

    fn as_any_rc(self: Rc<Self>) -> Rc<dyn std::any::Any> {
        self
    }
}
