//! Translation of Animation/Scheduler.swift; callbacks are invoked outside registry borrows.
use super::{AnimationCompletionGroup, AnyAnimationController};
use std::{cell::RefCell, collections::HashSet, rc::Rc};
#[derive(Default)]
struct Storage { active: RefCell<Vec<Rc<dyn AnyAnimationController>>>, registered: RefCell<HashSet<usize>>, completions: RefCell<Vec<Rc<AnimationCompletionGroup>>> }
#[derive(Clone, Default)]
pub struct AnimatorScheduler(Rc<Storage>);
thread_local! { static CURRENT: RefCell<AnimatorScheduler> = RefCell::new(AnimatorScheduler::default()); }
fn identity(controller: &Rc<dyn AnyAnimationController>) -> usize { Rc::as_ptr(controller) as *const () as usize }
impl AnimatorScheduler {
    pub fn current() -> Self { CURRENT.with(|current| current.borrow().clone()) }
    pub fn with_current<R>(&self, body: impl FnOnce() -> R) -> R {
        let previous = CURRENT.with(|current| current.replace(self.clone())); struct Restore(Option<AnimatorScheduler>);
        impl Drop for Restore { fn drop(&mut self) { CURRENT.with(|current| { current.replace(self.0.take().unwrap()); }); } }
        let _restore = Restore(Some(previous)); body()
    }
    pub fn active_count(&self) -> usize { self.0.active.borrow().len() }
    pub fn has_active_animations(&self) -> bool { !self.0.active.borrow().is_empty() }
    pub fn register(&self, controller: Rc<dyn AnyAnimationController>) {
        if controller.is_finished() || !self.0.registered.borrow_mut().insert(identity(&controller)) { return; } self.0.active.borrow_mut().push(controller);
    }
    pub fn unregister(&self, controller: &Rc<dyn AnyAnimationController>) {
        let id = identity(controller); if !self.0.registered.borrow_mut().remove(&id) { return; } self.0.active.borrow_mut().retain(|controller| identity(controller) != id);
    }
    pub fn track(&self, group: Rc<AnimationCompletionGroup>) {
        let storage = Rc::downgrade(&self.0); let weak = Rc::downgrade(&group);
        *group.on_settlement.borrow_mut() = Some(Rc::new(move || { if let (Some(storage), Some(group)) = (storage.upgrade(), weak.upgrade()) { storage.completions.borrow_mut().retain(|candidate| !Rc::ptr_eq(candidate, &group)); } }));
        self.0.completions.borrow_mut().push(group);
    }
    pub fn tick(&self, delta_time: f64) {
        if !delta_time.is_finite() || delta_time < 0.0 { return; }
        let active = self.0.active.borrow().clone(); for controller in active { if self.0.registered.borrow().contains(&identity(&controller)) { controller.tick(delta_time); } }
        let finished: HashSet<_> = self.0.active.borrow().iter().filter(|c| c.is_finished()).map(identity).collect();
        self.0.active.borrow_mut().retain(|c| !finished.contains(&identity(c))); self.0.registered.borrow_mut().retain(|id| !finished.contains(id));
        let groups = self.0.completions.borrow().clone(); for group in groups { group.settle_if_idle(); } self.0.completions.borrow_mut().retain(|g| !g.has_settled());
    }
    pub fn reset(&self) {
        let active = self.0.active.borrow().clone(); for controller in active { controller.cancel(); }
        self.0.active.borrow_mut().clear(); self.0.registered.borrow_mut().clear(); self.0.completions.borrow_mut().clear();
    }
}
