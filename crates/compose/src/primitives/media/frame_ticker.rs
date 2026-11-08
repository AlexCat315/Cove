//! A node-owned per-frame callback, driven by the shared animation scheduler.
//!
//! It runs only while active and mounted, so an idle or off-screen player
//! costs no frames.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use cove_core::scene::{
    Animation, AnimationController, AnimatorScheduler, AnyAnimationController, Node, NodeResource,
};

pub type FrameCallback = Rc<dyn Fn(&Node)>;

pub struct FrameTicker {
    node: RefCell<cove_core::scene::NodeWeak>,
    scheduler: RefCell<Option<AnimatorScheduler>>,
    controller: RefCell<Option<Rc<dyn AnyAnimationController>>>,
    callback: RefCell<Option<FrameCallback>>,
    active: Cell<bool>,
}

impl FrameTicker {
    pub fn new() -> Rc<Self> {
        Rc::new(Self {
            node: RefCell::new(Weak::new()),
            scheduler: RefCell::new(None),
            controller: RefCell::new(None),
            callback: RefCell::new(None),
            active: Cell::new(false),
        })
    }

    /// Replaces the callback and starts or stops the ticks.
    pub fn configure(&self, active: bool, callback: FrameCallback) {
        *self.callback.borrow_mut() = Some(callback);
        if active == self.active.get()
            && (!active
                || self
                    .controller
                    .borrow()
                    .as_ref()
                    .is_some_and(|controller| !controller.is_finished()))
        {
            return;
        }
        self.active.set(active);
        self.restart();
    }

    fn restart(&self) {
        self.stop();
        if !self.active.get() {
            return;
        }
        let Some(node) = Node::upgrade(&self.node.borrow()) else {
            return;
        };
        let Some(scheduler) = self.scheduler.borrow().clone() else {
            return;
        };
        let weak = node.downgrade();
        let animation =
            Animation::new(1.0, cove_core::scene::AnimationCurve::Linear).repeat_forever(false);
        let controller = AnimationController::new(0.0_f64, 1.0, animation, 0.0, move |_| {
            let Some(node) = Node::upgrade(&weak) else {
                return;
            };
            let callback = node
                .first_resource::<FrameTicker>()
                .and_then(|ticker| ticker.callback.borrow().clone());
            if let Some(callback) = callback {
                callback(&node);
            }
        });
        let any: Rc<dyn AnyAnimationController> = controller;
        scheduler.register(any.clone());
        *self.controller.borrow_mut() = Some(any);
    }

    fn stop(&self) {
        if let Some(controller) = self.controller.borrow_mut().take() {
            controller.cancel();
            if let Some(scheduler) = self.scheduler.borrow().as_ref() {
                scheduler.unregister(&controller);
            }
        }
    }
}

impl NodeResource for FrameTicker {
    fn mount(&self, node: &Node) {
        *self.node.borrow_mut() = node.downgrade();
        *self.scheduler.borrow_mut() = Some(AnimatorScheduler::current());
        self.restart();
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

impl Drop for FrameTicker {
    fn drop(&mut self) {
        self.stop();
    }
}
