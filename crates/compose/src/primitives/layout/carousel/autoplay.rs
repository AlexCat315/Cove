//! Translation of Primitives/Layout/Carousel/Autoplay.swift.
//!
//! Automatic advance for a carousel. Rotation pauses while the pointer is over
//! the carousel or keyboard focus is inside it, restarts its interval after any
//! manual page change, and always offers a pause control. With reduced motion
//! it starts paused.
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use cove_composition::{AnyView, PrimitiveView};
use cove_core::scene::{
    AnimatorScheduler, AnyAnimationController, FocusChainHolder, InvalidationSource, LayoutNode,
    Node, NodeResource,
};

use crate::impl_primitive_view;

/// Automatic advance for a `Carousel`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CarouselAutoplay {
    /// Seconds each page stays before advancing, 2–60.
    pub interval: f64,
}

impl CarouselAutoplay {
    pub fn new(interval: f64) -> Self {
        Self {
            interval: if interval.is_finite() {
                interval.clamp(2.0, 60.0)
            } else {
                5.0
            },
        }
    }
}

impl Default for CarouselAutoplay {
    fn default() -> Self {
        Self::new(5.0)
    }
}

/// Transient rotation state, owned by the mounted carousel and never part of
/// its configuration.
#[derive(Default)]
pub struct CarouselAutoplaySession {
    user_paused: Cell<Option<bool>>,
    elapsed: Cell<f64>,
    is_hovered: Cell<bool>,
    page_index: Cell<Option<Int>>,
    progress_node: RefCell<Option<Node>>,
}

type Int = i64;

impl CarouselAutoplaySession {
    pub fn new() -> Self {
        Self::default()
    }

    /// Before the viewer chooses, reduced motion means paused.
    pub fn is_paused(&self, reduced_motion: bool) -> bool {
        self.user_paused.get().unwrap_or(reduced_motion)
    }

    pub fn set_paused(&self, paused: bool) {
        self.user_paused.set(Some(paused));
        self.elapsed.set(0.0);
        if let Some(node) = self.progress_node.borrow().clone() {
            node.mark_render_dirty(InvalidationSource::style_set("carouselProgress"));
        }
    }

    pub fn progress(&self, interval: f64) -> f32 {
        (self.elapsed.get() / interval).clamp(0.0, 1.0) as f32
    }

    /// A page change from any source restarts the interval.
    pub fn observe(&self, page: Int) {
        if self.page_index.get() == Some(page) {
            return;
        }
        if self.page_index.get().is_some() {
            self.elapsed.set(0.0);
        }
        self.page_index.set(Some(page));
        if let Some(node) = self.progress_node.borrow().clone() {
            node.mark_render_dirty(InvalidationSource::style_set("carouselProgress"));
        }
    }

    pub fn set_progress_node(&self, node: Option<Node>) {
        *self.progress_node.borrow_mut() = node;
    }

    pub fn set_hovered(&self, hovered: bool) {
        self.is_hovered.set(hovered);
    }

    fn invalidate_progress(&self) {
        if let Some(node) = self.progress_node.borrow().clone() {
            node.mark_render_dirty(InvalidationSource::style_set("carouselProgress"));
        }
    }
}

/// Hosts the clock for one carousel; adds no layout of its own.
pub(crate) struct CarouselAutoplayHost {
    session: Rc<CarouselAutoplaySession>,
    /// `None` stops the clock: no autoplay, paused, disabled or nothing to
    /// advance.
    interval: Option<f64>,
    advance: Rc<dyn Fn()>,
    /// Reads the live page, so a change from any source restarts the interval
    /// even when it does not recompose the carousel.
    page: Rc<dyn Fn() -> Int>,
    content: AnyView,
}

impl CarouselAutoplayHost {
    pub fn new(
        session: Rc<CarouselAutoplaySession>,
        interval: Option<f64>,
        advance: impl Fn() + 'static,
        page: impl Fn() -> Int + 'static,
        content: AnyView,
    ) -> Self {
        Self {
            session,
            interval,
            advance: Rc::new(advance),
            page: Rc::new(page),
            content,
        }
    }
}

impl PrimitiveView for CarouselAutoplayHost {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        let clock = Rc::new(CarouselClock::new());
        node.add_resource(clock);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        None
    }

    fn update_node(&self, node: &Node) {
        if let Some(clock) = node.first_resource::<CarouselClock>() {
            clock.configure(
                self.session.clone(),
                self.interval,
                self.advance.clone(),
                self.page.clone(),
            );
        }
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![self.content.clone()]
    }
}
impl_primitive_view!(CarouselAutoplayHost);

struct CarouselClock {
    node: RefCell<Option<Node>>,
    session: RefCell<Option<Rc<CarouselAutoplaySession>>>,
    interval: Cell<f64>,
    advance: RefCell<Rc<dyn Fn()>>,
    page: RefCell<Rc<dyn Fn() -> Int>>,
    is_finished: Cell<bool>,
}

impl CarouselClock {
    fn new() -> Self {
        Self {
            node: RefCell::new(None),
            session: RefCell::new(None),
            interval: Cell::new(5.0),
            advance: RefCell::new(Rc::new(|| {})),
            page: RefCell::new(Rc::new(|| 0)),
            is_finished: Cell::new(true),
        }
    }

    fn configure(
        self: &Rc<Self>,
        session: Rc<CarouselAutoplaySession>,
        interval: Option<f64>,
        advance: Rc<dyn Fn()>,
        page: Rc<dyn Fn() -> Int>,
    ) {
        *self.session.borrow_mut() = Some(session);
        *self.advance.borrow_mut() = advance;
        *self.page.borrow_mut() = page;
        let Some(interval) = interval else {
            Self::stop(self);
            return;
        };
        self.interval.set(interval);
        if self.is_finished.get() {
            self.is_finished.set(false);
            AnimatorScheduler::current().register(self.clone());
        }
    }

    fn stop(&self) {
        if self.is_finished.get() {
            return;
        }
        self.is_finished.set(true);
        // The owning node holds the canonical Rc; recover it so the scheduler
        // matches it by pointer identity.
        if let Some(node) = self.node.borrow().clone()
            && let Some(clock) = node.first_resource::<CarouselClock>()
        {
            let controller: Rc<dyn AnyAnimationController> = clock;
            AnimatorScheduler::current().unregister(&controller);
        }
    }

    fn contains_focus(node: &Node) -> bool {
        let mut current = FocusChainHolder::current_focused();
        while let Some(candidate) = current {
            if &candidate == node {
                return true;
            }
            current = candidate.parent();
        }
        false
    }
}

impl NodeResource for CarouselClock {
    fn mount(&self, node: &Node) {
        *self.node.borrow_mut() = Some(node.clone());
    }

    fn unmount(&self, _node: &Node) {
        self.stop();
        *self.node.borrow_mut() = None;
    }

    fn as_any_rc(self: Rc<Self>) -> Rc<dyn std::any::Any> {
        self
    }
}

impl AnyAnimationController for CarouselClock {
    fn is_finished(&self) -> bool {
        self.is_finished.get()
    }

    fn tick(&self, delta_time: f64) {
        if self.is_finished.get() {
            return;
        }
        let Some(node) = self.node.borrow().clone() else {
            return;
        };
        let Some(session) = self.session.borrow().clone() else {
            return;
        };
        session.observe((self.page.borrow())());
        if session.is_hovered.get() || Self::contains_focus(&node) {
            return;
        }
        let next = session.elapsed.get() + delta_time;
        let interval = self.interval.get();
        if next >= interval {
            session.elapsed.set(0.0);
            (self.advance.borrow())();
        } else {
            session.elapsed.set(next);
        }
        session.invalidate_progress();
    }

    fn finish_immediately(&self) {
        self.stop();
    }

    fn cancel(&self) {
        self.stop();
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Reads the reduced-motion preference without adding a layout node.
pub(crate) struct ReducedMotionReader {
    content: Rc<dyn Fn(bool) -> AnyView>,
}

impl ReducedMotionReader {
    pub fn new(content: impl Fn(bool) -> AnyView + 'static) -> Self {
        Self {
            content: Rc::new(content),
        }
    }
}

impl PrimitiveView for ReducedMotionReader {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        None
    }

    fn update_node(&self, _node: &Node) {}

    fn children(&self, node: &Node) -> Vec<AnyView> {
        vec![(self.content)(node.prefers_reduced_motion())]
    }
}
impl_primitive_view!(ReducedMotionReader);
