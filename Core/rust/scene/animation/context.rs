//! Translation of Animation/ActiveAnimationContext.swift.
use super::{
    Animation, AnimationCallback, AnimationController, AnimatorScheduler, AnyAnimationController,
    Interpolatable,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnimationCompletionCriteria {
    LogicallyComplete,
    Removed,
}
pub struct AnimationCompletionGroup {
    criteria: AnimationCompletionCriteria,
    handler: AnimationCallback,
    outstanding: Cell<usize>,
    settled: Cell<bool>,
    cancelled: Cell<bool>,
    pub(crate) on_settlement: RefCell<Option<AnimationCallback>>,
}
impl AnimationCompletionGroup {
    pub fn new(criteria: AnimationCompletionCriteria, handler: impl Fn() + 'static) -> Rc<Self> {
        Rc::new(Self {
            criteria,
            handler: Rc::new(handler),
            outstanding: Cell::new(0),
            settled: Cell::new(false),
            cancelled: Cell::new(false),
            on_settlement: RefCell::new(None),
        })
    }
    pub fn has_settled(&self) -> bool {
        self.settled.get()
    }
    pub fn attach<T: Interpolatable>(self: &Rc<Self>, controller: &Rc<AnimationController<T>>) {
        if self.settled.get() {
            return;
        }
        self.outstanding.set(self.outstanding.get() + 1);
        if controller.is_finished()
            || (self.criteria == AnimationCompletionCriteria::LogicallyComplete
                && controller.is_logically_finished())
        {
            self.outstanding.set(self.outstanding.get() - 1);
            return;
        }
        let weak = Rc::downgrade(self);
        let completed: AnimationCallback = Rc::new(move || {
            if let Some(group) = weak.upgrade() {
                group
                    .outstanding
                    .set(group.outstanding.get().saturating_sub(1));
            }
        });
        if self.criteria == AnimationCompletionCriteria::LogicallyComplete {
            *controller.on_logical_finish.borrow_mut() = Some(completed);
        } else {
            let previous = controller.on_finish.borrow().clone();
            *controller.on_finish.borrow_mut() = Some(Rc::new(move || {
                if let Some(previous) = &previous {
                    previous();
                }
                completed();
            }));
        }
        let weak = Rc::downgrade(self);
        *controller.on_cancel.borrow_mut() = Some(Rc::new(move || {
            if let Some(group) = weak.upgrade() {
                group.cancelled.set(true);
                group.settled.set(true);
                let callback = group.on_settlement.borrow().clone();
                if let Some(callback) = callback {
                    callback();
                }
            }
        }));
    }
    pub fn settle_if_idle(&self) {
        if self.settled.get() || self.outstanding.get() != 0 {
            return;
        }
        self.settled.set(true);
        let callback = self.on_settlement.borrow().clone();
        if let Some(callback) = callback {
            callback();
        }
        if !self.cancelled.get() {
            (self.handler)();
        }
    }
}
#[derive(Clone, Default)]
pub struct ActiveAnimationContext {
    pub animation: Option<Animation>,
    pub completion: Option<Rc<AnimationCompletionGroup>>,
    pub disables_animations: bool,
}
thread_local! { static ACTIVE: RefCell<ActiveAnimationContext> = RefCell::new(ActiveAnimationContext::default()); }
impl ActiveAnimationContext {
    pub fn current() -> Self {
        ACTIVE.with(|active| active.borrow().clone())
    }
    pub fn with<R>(
        animation: Option<Animation>,
        completion: Option<Rc<AnimationCompletionGroup>>,
        disables: Option<bool>,
        body: impl FnOnce() -> R,
    ) -> R {
        let disabled = disables.unwrap_or(animation.is_none());
        let previous = ACTIVE.with(|active| {
            active.replace(Self {
                animation,
                completion,
                disables_animations: disabled,
            })
        });
        struct Restore(Option<ActiveAnimationContext>);
        impl Drop for Restore {
            fn drop(&mut self) {
                ACTIVE.with(|active| {
                    active.replace(self.0.take().unwrap());
                });
            }
        }
        let _restore = Restore(Some(previous));
        body()
    }
}
pub fn with_animation<R>(animation: Option<Animation>, body: impl FnOnce() -> R) -> R {
    ActiveAnimationContext::with(animation, None, None, body)
}
pub fn with_animation_completion<R>(
    animation: Option<Animation>,
    criteria: AnimationCompletionCriteria,
    body: impl FnOnce() -> R,
    completion: impl Fn() + 'static,
) -> R {
    let group = AnimationCompletionGroup::new(criteria, completion);
    AnimatorScheduler::current().track(group.clone());
    ActiveAnimationContext::with(animation, Some(group), None, body)
}
