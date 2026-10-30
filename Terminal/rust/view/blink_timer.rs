//! Drives a blink phase: the cursor while focused (DECSCUSR / mode 12, on by
//! default) and SGR 5/6 blinking text while any is on screen.
//!
//! A coarse phase toggles visibility, so an idle terminal costs two repaints a
//! second instead of a frame-rate animation. Input and output restart the cursor
//! phase so it stays solid while typing, as in other terminals.

use cove_core::scene::{Animation, AnimationController, AnimatorScheduler, AnyAnimationController};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

/// Half a blink period: the visible and hidden phases are each this long.
pub const INTERVAL_SECONDS: f64 = 0.6;

#[derive(Default)]
pub struct BlinkTimer {
    is_visible: Cell<bool>,
    on_toggle: RefCell<Option<Rc<dyn Fn()>>>,
    controller: RefCell<Option<Rc<dyn AnyAnimationController>>>,
    scheduler: RefCell<Option<AnimatorScheduler>>,
    active: Cell<bool>,
}

impl BlinkTimer {
    pub fn new() -> Rc<Self> {
        let timer = Rc::new(Self::default());
        timer.is_visible.set(true);
        timer
    }

    pub fn set_on_toggle(&self, handler: Option<Rc<dyn Fn()>>) {
        *self.on_toggle.borrow_mut() = handler;
    }

    pub fn is_visible(&self) -> bool {
        self.is_visible.get()
    }

    /// Called each paint with whether blinking applies right now.
    pub fn update(&self, active: bool) {
        if active == self.active.get() {
            return;
        }
        self.active.set(active);
        if active {
            self.start();
        } else {
            self.stop();
        }
    }

    /// Shows the cursor and restarts the period.
    pub fn restart(&self) {
        self.is_visible.set(true);
        if self.active.get() {
            self.start();
        }
    }

    pub fn stop(&self) {
        if let Some(controller) = self.controller.borrow_mut().take()
            && let Some(scheduler) = self.scheduler.borrow().as_ref()
        {
            controller.cancel();
            scheduler.unregister(&controller);
        }
        self.is_visible.set(true);
    }

    /// The scheduler that drives the phase; set when the timer is mounted on a
    /// node so it shares the scene's clock.
    pub fn attach(&self, scheduler: AnimatorScheduler) {
        *self.scheduler.borrow_mut() = Some(scheduler);
        if self.active.get() {
            self.start();
        }
    }

    pub fn detach(&self) {
        self.stop();
        *self.scheduler.borrow_mut() = None;
    }

    fn start(&self) {
        self.stop_controller_only();
        self.is_visible.set(true);
        let timer = self.clone_handle();
        // Two half-periods make one full cycle, so a linear 1.2 s repeat gives a
        // 600 ms on / 600 ms off phase.
        let animation = Animation::new(INTERVAL_SECONDS * 2.0, cove_core::scene::AnimationCurve::Linear)
            .repeat_forever(false);
        let visible = self.is_visible.clone();
        let toggle = self.on_toggle.clone();
        let controller = AnimationController::new(0.0_f64, 1.0, animation, 0.0, move |progress| {
            let visible_now = progress < 0.5;
            if visible.get() == visible_now {
                return;
            }
            visible.set(visible_now);
            if let Some(handler) = toggle.borrow().as_ref() {
                handler();
            }
            let _ = &timer;
        });
        let controller: Rc<dyn AnyAnimationController> = controller;
        if let Some(scheduler) = self.scheduler.borrow().as_ref() {
            scheduler.register(controller.clone());
        }
        *self.controller.borrow_mut() = Some(controller);
    }

    fn stop_controller_only(&self) {
        if let Some(controller) = self.controller.borrow_mut().take()
            && let Some(scheduler) = self.scheduler.borrow().as_ref()
        {
            controller.cancel();
            scheduler.unregister(&controller);
        }
    }

    /// A handle the phase callback can hold without keeping the timer alive.
    fn clone_handle(self: &Rc<Self>) -> BlinkHandle {
        BlinkHandle(Rc::downgrade(self))
    }
}

/// Weak handle used inside the phase callback so the animation never owns the
/// timer.
pub struct BlinkHandle(std::rc::Weak<BlinkTimer>);

impl BlinkHandle {
    pub fn upgrade(&self) -> Option<Rc<BlinkTimer>> {
        self.0.upgrade()
    }
}

impl Drop for BlinkTimer {
    fn drop(&mut self) {
        self.stop_controller_only();
    }
}
