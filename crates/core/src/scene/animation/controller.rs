//! Translation of Animation/AnimationController.swift.
use super::{
    Animation, AnimationCurve, AnimationRepetition, Interpolatable, MotionVector, SpringSolution,
};
use std::{
    any::Any,
    cell::{Cell, RefCell},
    rc::Rc,
};
pub type AnimationCallback = Rc<dyn Fn()>;
pub trait AnyAnimationController: Any {
    fn is_finished(&self) -> bool;
    fn tick(&self, delta_time: f64);
    fn finish_immediately(&self);
    fn cancel(&self);
    fn as_any(&self) -> &dyn Any;
}
pub struct AnimationController<T: Interpolatable> {
    pub from: T,
    pub to: T,
    pub animation: Animation,
    apply: Rc<dyn Fn(T)>,
    elapsed: Cell<f64>,
    finished: Cell<bool>,
    logical_finished: Cell<bool>,
    progress_velocity: Cell<f32>,
    motion: RefCell<Option<MotionVector<T>>>,
    spring: Option<SpringSolution>,
    release_velocity: f32,
    pub(crate) on_finish: RefCell<Option<AnimationCallback>>,
    pub(crate) on_logical_finish: RefCell<Option<AnimationCallback>>,
    pub(crate) on_cancel: RefCell<Option<AnimationCallback>>,
}
impl<T: Interpolatable> AnimationController<T> {
    pub fn new(
        from: T,
        to: T,
        animation: Animation,
        initial_velocity: f32,
        apply: impl Fn(T) + 'static,
    ) -> Rc<Self> {
        let velocity = if initial_velocity == 0.0 {
            animation.initial_velocity
        } else {
            initial_velocity
        };
        let spring = if let AnimationCurve::Spring {
            response,
            damping_fraction,
        } = &animation.curve
        {
            Some(SpringSolution::new(*response, *damping_fraction))
        } else {
            None
        };
        let immediate = animation.duration == 0.0 && animation.delay == 0.0;
        let controller = Rc::new(Self {
            from,
            to,
            animation,
            apply: Rc::new(apply),
            elapsed: Cell::new(0.0),
            finished: Cell::new(immediate),
            logical_finished: Cell::new(immediate),
            progress_velocity: Cell::new(0.0),
            motion: RefCell::new(None),
            spring,
            release_velocity: if velocity.is_finite() { velocity } else { 0.0 },
            on_finish: RefCell::new(None),
            on_logical_finish: RefCell::new(None),
            on_cancel: RefCell::new(None),
        });
        if immediate {
            (controller.apply)(controller.to.clone());
        }
        controller
    }
    pub fn elapsed(&self) -> f64 {
        self.elapsed.get()
    }
    pub fn progress_velocity(&self) -> f32 {
        self.progress_velocity.get()
    }
    pub fn is_logically_finished(&self) -> bool {
        self.logical_finished.get()
    }
    pub fn set_on_finish(&self, callback: Option<AnimationCallback>) {
        *self.on_finish.borrow_mut() = callback;
    }
    pub fn set_on_cancel(&self, callback: Option<AnimationCallback>) {
        *self.on_cancel.borrow_mut() = callback;
    }
    pub fn inherit_motion(&self, previous: &Self) {
        if self.spring.is_none()
            || !self.animation.allows_overshoot
            || self.release_velocity != 0.0
            || previous.is_finished()
        {
            return;
        }
        let active = (previous.elapsed.get() - previous.animation.delay).max(0.0);
        let lower = (active - 0.0001).max(0.0);
        let upper = active + 0.0001;
        *self.motion.borrow_mut() = Some(MotionVector {
            from: previous.value_at(lower),
            to: previous.value_at(upper),
            rate: (1.0 / (upper - lower)) as f32,
        });
    }
    fn spring_progress(&self, active: f64) -> f32 {
        self.spring.map_or(0.0, |spring| {
            spring.position(active as f32, 0.0, self.release_velocity, 1.0)
        })
    }
    fn value_at(&self, active: f64) -> T {
        if let Some(spring) = self
            .spring
            .filter(|_| self.animation.repetition == AnimationRepetition::Once)
        {
            let progress = self.spring_progress(active);
            let base = T::interpolate(
                &self.from,
                &self.to,
                if self.animation.allows_overshoot {
                    progress
                } else {
                    progress.clamp(0.0, 1.0)
                },
            );
            if self.animation.allows_overshoot
                && let Some(motion) = self.motion.borrow().as_ref()
            {
                return motion.adding(&base, spring.position(active as f32, 0.0, 1.0, 0.0));
            }
            return base;
        }
        let sample = self.animation.playback(active + self.animation.delay);
        let progress = self.animation.curve.evaluate(sample.time);
        T::interpolate(
            &self.from,
            &self.to,
            if self.animation.allows_overshoot {
                progress
            } else {
                progress.clamp(0.0, 1.0)
            },
        )
    }
    fn finish_logical(&self) {
        if self.logical_finished.replace(true) {
            return;
        }
        let callback = self.on_logical_finish.borrow().clone();
        if let Some(callback) = callback {
            callback();
        }
    }
    fn finish(&self) {
        self.finish_logical();
        self.finished.set(true);
        self.progress_velocity.set(0.0);
        let callback = self.on_finish.borrow().clone();
        if let Some(callback) = callback {
            callback();
        }
    }
}
impl<T: Interpolatable> AnyAnimationController for AnimationController<T> {
    fn is_finished(&self) -> bool {
        self.finished.get()
    }
    fn tick(&self, delta_time: f64) {
        if self.finished.get() || !delta_time.is_finite() || delta_time < 0.0 {
            return;
        }
        self.elapsed
            .set((self.elapsed.get() + delta_time).min(f64::MAX));
        let active = self.elapsed.get() - self.animation.delay;
        if active <= 0.0 {
            (self.apply)(self.from.clone());
            return;
        }
        if let Some(spring) = self.spring.filter(|_| {
            self.animation.repetition == AnimationRepetition::Once && self.animation.duration > 0.0
        }) {
            let progress = self.spring_progress(active);
            self.progress_velocity.set(spring.velocity(
                active as f32,
                0.0,
                self.release_velocity,
                1.0,
            ));
            if active >= self.animation.duration {
                self.finish_logical();
            }
            if active >= self.animation.duration.max(spring.settling_duration())
                || !progress.is_finite()
            {
                (self.apply)(self.to.clone());
                self.finish();
                return;
            }
            (self.apply)(self.value_at(active));
        } else {
            let sample = self.animation.playback(self.elapsed.get());
            if sample.finished {
                (self.apply)(if sample.time == 0.0 {
                    self.from.clone()
                } else {
                    self.to.clone()
                });
                self.finish();
                return;
            }
            (self.apply)(self.value_at(active));
        }
    }
    fn finish_immediately(&self) {
        if !self.finished.get() {
            (self.apply)(self.to.clone());
            self.finish();
        }
    }
    fn cancel(&self) {
        if self.finished.replace(true) {
            return;
        }
        self.progress_velocity.set(0.0);
        let callback = self.on_cancel.borrow().clone();
        if let Some(callback) = callback {
            callback();
        }
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}
