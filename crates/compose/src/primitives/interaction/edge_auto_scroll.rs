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

use crate::primitives::layout::scroll_view::{
    cached_scrollable_content_size, report_scroll_change, visible_viewport_rect,
};
use crate::primitives::layout::ScrollAxis;

/// Points per second at the very edge.
pub const MAXIMUM_SPEED: f32 = 1200.0;

/// The component of a value that lies along the scrolling axis.
fn main_of(point: Point, axis: ScrollAxis) -> f32 {
    match axis {
        ScrollAxis::Horizontal => point.x,
        _ => point.y,
    }
}

/// Signed speed for a pointer held at `pointer`: zero outside the edge zones
/// of `axis`, rising quadratically to the maximum at (or beyond) the edge.
pub fn velocity(pointer: Point, viewport: Rect, axis: ScrollAxis) -> f32 {
    let (main, start, extent) = match axis {
        ScrollAxis::Horizontal => (pointer.x, viewport.x, viewport.width),
        _ => (pointer.y, viewport.y, viewport.height),
    };
    let zone = 48.0_f32.min(extent / 4.0);
    if zone <= 0.0 {
        return 0.0;
    }
    let leading = main - start;
    let trailing = start + extent - main;
    if leading < zone {
        let depth = (1.0_f32).min((zone - leading) / zone);
        return -MAXIMUM_SPEED * depth * depth;
    }
    if trailing < zone {
        let depth = (1.0_f32).min((zone - trailing) / zone);
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
    pointer: Cell<Point>,
    axis: Cell<ScrollAxis>,
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
                pointer: Cell::new(Point::default()),
                axis: Cell::new(ScrollAxis::Vertical),
                start_offset: Cell::new(0.0),
                on_scroll: RefCell::new(None),
                is_finished: Cell::new(true),
            }),
            scheduler: RefCell::new(None),
            controller: RefCell::new(None),
        })
    }

    /// Distance scrolled along the gesture axis since `begin`; drags add it to
    /// the pointer delta so positions stay in content coordinates.
    pub fn scrolled_distance(&self) -> f32 {
        let Some(scroll) = self.state.scroll.borrow().clone() else {
            return 0.0;
        };
        main_of(scroll.content_offset(), self.state.axis.get()) - self.state.start_offset.get()
    }

    pub fn is_finished(&self) -> bool {
        self.state.is_finished.get()
    }

    /// Starts tracking; `on_scroll` runs after every automatic scroll step.
    pub fn begin(self: &Rc<Self>, pointer: Point, axis: ScrollAxis, on_scroll: Rc<dyn Fn()>) {
        let Some(owner) = self.state.owner.borrow().clone() else {
            return;
        };
        let Some(scroll) = enclosing_scroll(&owner) else {
            return;
        };
        self.state.axis.set(axis);
        self.state
            .start_offset
            .set(main_of(scroll.content_offset(), axis));
        self.state.pointer.set(pointer);
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

    pub fn update(&self, pointer: Point) {
        self.state.pointer.set(pointer);
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
    let axis = state.axis.get();
    // Pointer events are in window coordinates while the viewport rect is
    // local to the scroll node; compare both in the scroll's space.
    let pointer = scroll.convert_from_window(state.pointer.get());
    let viewport = visible_viewport_rect(&scroll);
    let speed = velocity(pointer, viewport, axis);
    if speed == 0.0 {
        return;
    }
    let content = cached_scrollable_content_size(&scroll);
    let (content_main, viewport_main) = match axis {
        ScrollAxis::Horizontal => (content.width, viewport.width),
        _ => (content.height, viewport.height),
    };
    let maximum = (content_main - viewport_main).max(0.0);
    let offset = scroll.content_offset();
    let current = main_of(offset, axis);
    let next = (current + speed * delta_time as f32).clamp(0.0, maximum);
    if next == current {
        return;
    }
    let scrolled = match axis {
        ScrollAxis::Horizontal => Point::new(next, offset.y),
        _ => Point::new(offset.x, next),
    };
    scroll.set_content_offset(scrolled);
    report_scroll_change(&scroll);
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
