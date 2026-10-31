//! Translation of Compose/Sources/CoveUICompose/Primitives/Media/LoadingAnimationClock.swift.
//!
//! Node-owned adapter to the shared repeating animation controller.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use cove_core::scene::{
    Animation, AnimationController, AnimatorScheduler, AnyAnimationController, InvalidationSource,
    Node, NodeResource,
};

/// Repeating 0...1 phase used by spinners, skeletons and indeterminate bars.
///
/// The controller is registered with the owning node and cancelled when it
/// leaves the tree, so a loader in a conditionally-rendered branch never keeps
/// a clock running in the background.
pub struct LoadingAnimationClock {
    node: RefCell<cove_core::scene::NodeWeak>,
    scheduler: RefCell<Option<AnimatorScheduler>>,
    duration: Cell<f64>,
    controller: RefCell<Option<Rc<dyn AnyAnimationController>>>,
    phase: Cell<f64>,
}

impl LoadingAnimationClock {
    pub fn new() -> Rc<Self> {
        Rc::new(Self {
            node: RefCell::new(Weak::new()),
            scheduler: RefCell::new(None),
            duration: Cell::new(2.0),
            controller: RefCell::new(None),
            phase: Cell::new(0.0),
        })
    }

    pub fn phase(&self) -> f64 {
        self.phase.get()
    }

    /// Durations outside `0.25...60` are clamped; non-finite values fall back
    /// to two seconds. Inactive clocks stop and reset the phase.
    pub fn configure(&self, duration: f64, is_active: bool) {
        let duration = if duration.is_finite() {
            duration.clamp(0.25, 60.0)
        } else {
            2.0
        };
        if !is_active {
            self.stop();
            return;
        }
        if self.duration.get() == duration
            && self
                .controller
                .borrow()
                .as_ref()
                .is_some_and(|controller| !controller.is_finished())
        {
            return;
        }
        self.stop();
        self.duration.set(duration);
        let Some(node) = Node::upgrade(&self.node.borrow()) else {
            return;
        };
        let weak_node = node.downgrade();
        let animation = Animation::new(duration, cove_core::scene::AnimationCurve::Linear)
            .repeat_forever(false);
        let controller = AnimationController::new(0.0_f64, 1.0, animation, 0.0, move |value| {
            let Some(node) = Node::upgrade(&weak_node) else {
                return;
            };
            if let Some(clock) = node.first_resource::<LoadingAnimationClock>() {
                clock.phase.set(value);
            }
            node.mark_render_dirty(InvalidationSource::style_set("loading.phase"));
        });
        let any: Rc<dyn AnyAnimationController> = controller;
        if let Some(scheduler) = self.scheduler.borrow().as_ref() {
            scheduler.register(any.clone());
        }
        *self.controller.borrow_mut() = Some(any);
    }

    fn stop(&self) {
        let controller = self.controller.borrow_mut().take();
        if controller.is_none() && self.phase.get() == 0.0 {
            return;
        }
        if let Some(controller) = controller {
            controller.cancel();
            if let Some(scheduler) = self.scheduler.borrow().as_ref() {
                scheduler.unregister(&controller);
            }
        }
        self.phase.set(0.0);
        if let Some(node) = Node::upgrade(&self.node.borrow()) {
            node.mark_render_dirty(InvalidationSource::style_set("loading.phase"));
        }
    }
}

impl Default for LoadingAnimationClock {
    fn default() -> Self {
        Self {
            node: RefCell::new(Weak::new()),
            scheduler: RefCell::new(None),
            duration: Cell::new(2.0),
            controller: RefCell::new(None),
            phase: Cell::new(0.0),
        }
    }
}

impl NodeResource for LoadingAnimationClock {
    fn mount(&self, node: &Node) {
        *self.node.borrow_mut() = node.downgrade();
        *self.scheduler.borrow_mut() = Some(AnimatorScheduler::current());
    }

    fn unmount(&self, _node: &Node) {
        self.stop();
        *self.node.borrow_mut() = Weak::new();
        *self.scheduler.borrow_mut() = None;
    }

    fn as_any_rc(self: Rc<Self>) -> Rc<dyn std::any::Any> {
        self
    }
}

impl Drop for LoadingAnimationClock {
    fn drop(&mut self) {
        self.stop();
    }
}
