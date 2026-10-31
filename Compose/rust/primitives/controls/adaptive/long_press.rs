//! Translation of Primitives/Controls/Adaptive/LongPress.swift.
use cove_core::Point;
use cove_core::scene::{
    AnimatorScheduler, AnyAnimationController, Node, NodeResource, PointerCaptureHolder,
};
use std::any::Any;
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

/// A hold observes the tap until it wins. Motion/wheel input cancels before
/// an ancestor scroll takes over; only recognition cancels the child press.
pub struct TouchLongPress {
    node: RefCell<Option<Node>>,
    origin: Cell<Option<Point>>,
    elapsed: Cell<f64>,
    is_finished: Cell<bool>,
    self_reference: RefCell<Weak<TouchLongPress>>,
    pub on_recognized: RefCell<Rc<dyn Fn(Point)>>,
}

impl TouchLongPress {
    pub fn new() -> Rc<Self> {
        Rc::new_cyclic(|self_reference| Self {
            node: RefCell::new(None),
            origin: Cell::new(None),
            elapsed: Cell::new(0.0),
            is_finished: Cell::new(true),
            self_reference: RefCell::new(self_reference.clone()),
            on_recognized: RefCell::new(Rc::new(|_| {})),
        })
    }

    pub fn begin(&self, point: Point) {
        self.cancel();
        self.origin.set(Some(point));
        self.elapsed.set(0.0);
        self.is_finished.set(false);
        if let Some(controller) = self.self_reference.borrow().upgrade() {
            let controller: Rc<dyn AnyAnimationController> = controller;
            AnimatorScheduler::current().register(controller);
        }
    }

    pub fn move_to(&self, point: Point) {
        let Some(origin) = self.origin.get() else {
            return;
        };
        if ((point.x - origin.x).powi(2) + (point.y - origin.y).powi(2)).sqrt() > 10.0 {
            self.cancel();
        }
    }

    pub fn cancel(&self) {
        self.is_finished.set(true);
        self.origin.set(None);
    }

    fn tick_long_press(&self, delta_time: f64) {
        if self.is_finished.get() {
            return;
        }
        let (Some(node), Some(origin)) = (self.node.borrow().clone(), self.origin.get()) else {
            return;
        };
        if !node.accepts_subtree_input() {
            self.cancel();
            return;
        }
        if let Some(captured) = PointerCaptureHolder::current().and_then(|capture| capture.target())
        {
            let mut candidate = Some(captured);
            while let Some(current) = candidate.clone() {
                if current.id() == node.id() {
                    break;
                }
                candidate = current.parent();
            }
            if !candidate.is_some_and(|current| current.id() == node.id()) {
                self.cancel();
                return;
            }
        }
        self.elapsed.set(self.elapsed.get() + delta_time.max(0.0));
        if self.elapsed.get() < 0.5 {
            return;
        }
        self.is_finished.set(true);
        self.origin.set(None);
        if let Some(capture) = PointerCaptureHolder::current() {
            capture.release();
        }
        let callback = self.on_recognized.borrow().clone();
        callback(origin);
    }
}

impl NodeResource for TouchLongPress {
    fn mount(&self, node: &Node) {
        *self.node.borrow_mut() = Some(node.clone());
    }
    fn unmount(&self, _node: &Node) {
        self.cancel();
        *self.node.borrow_mut() = None;
    }
    fn as_any_rc(self: Rc<Self>) -> Rc<dyn Any> {
        self
    }
}

impl AnyAnimationController for TouchLongPress {
    fn is_finished(&self) -> bool {
        self.is_finished.get()
    }
    fn tick(&self, delta_time: f64) {
        self.tick_long_press(delta_time);
    }
    fn finish_immediately(&self) {
        self.cancel();
    }
    fn cancel(&self) {
        TouchLongPress::cancel(self);
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}
