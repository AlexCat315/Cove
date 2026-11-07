//! Translation of CoveUIScene/Animation/AnimatableWrite.swift.
//!
//! Helpers for modifier `apply` paths that need to flow through the animator
//! when `withAnimation` is in scope, and write the value instantaneously
//! otherwise.
//!
//! All paths follow the same shape:
//! 1. If `ActiveAnimationContext::current().animation` is `None`, write
//!    `target` directly and return.
//! 2. Otherwise, snapshot the current value, build an `AnimationController`
//!    whose `apply` closure writes through the same accessor, and register it
//!    with `AnimatorScheduler::current()`.
//! 3. Render invalidation is emitted by the `Node` property setter that the
//!    controller writes through; `animatable_set` does not need a second,
//!    parallel dirty-tracking path.
//!
//! A node that prefers reduced motion snaps to `target` and cancels any
//! in-flight controller for that property, matching `AnimatedVisibility` and
//! the scroll physics; state feedback still reaches its final value.
//!
//! Edge cases for `Option<Value>` (e.g. `Color?`): if either endpoint is `None`
//! we cannot meaningfully interpolate, so we fall back to an instant write and
//! skip the controller. Animating in/out of `None` is the caller's
//! responsibility (e.g. wrap with an explicit transparent `Color`).
use super::{
    ActiveAnimationContext, AnimationController, AnimatorScheduler, AnyAnimationController,
    Interpolatable, Node,
};
use std::rc::Rc;

impl Node {
    /// Cancel and clear the active controller for a custom property key.
    ///
    /// Useful for mode switches where interpolation is not defined (for
    /// example points -> percent or any -> auto).
    pub fn cancel_animation(&self, key: &'static str) {
        self.replace_animation_controller(key, None);
    }

    /// Animate (or instantly assign) a custom property endpoint.
    ///
    /// Use this when the target value does not live behind a `Node` accessor
    /// (for example, layout properties on `LayoutNode`).
    pub fn animatable_set_with<T: Interpolatable + PartialEq + 'static>(
        &self,
        key: &'static str,
        current: T,
        target: T,
        apply: impl Fn(T) + 'static,
    ) {
        // A node that has left the tree cannot be seen and nothing will cancel
        // a controller started on it now: write the value and register nothing.
        if self.is_detached() {
            self.replace_animation_controller(key, None);
            apply(target);
            return;
        }
        if self.finishes_reduced_animation(key, &target) {
            return;
        }
        if self.continues_animation(key, &target) {
            return;
        }
        let context = ActiveAnimationContext::current();
        if current == target && context.animation.is_none() {
            if self.has_animation_controller(key) {
                self.replace_animation_controller(key, None);
            }
            apply(target);
            return;
        }
        let Some(animation) = context.animation else {
            self.replace_animation_controller(key, None);
            apply(target);
            return;
        };
        if self.prefers_reduced_motion() || current == target {
            self.replace_animation_controller(key, None);
            apply(target);
            return;
        }
        let controller = AnimationController::new(current, target, animation, 0.0, apply);
        self.inherit_motion(key, &controller);
        let any: Rc<dyn AnyAnimationController> = controller.clone();
        self.replace_animation_controller(key, Some(any));
        if let Some(completion) = context.completion.as_ref() {
            completion.attach(&controller);
        }
        AnimatorScheduler::current().register(controller);
    }

    /// Optional-typed variant of the custom-property API.
    ///
    /// If either endpoint is `None`, the value snaps and no controller is
    /// registered because interpolation is undefined.
    pub fn animatable_set_optional<T: Interpolatable + PartialEq + 'static>(
        &self,
        key: &'static str,
        current: Option<T>,
        target: Option<T>,
        apply: impl Fn(Option<T>) + 'static,
    ) {
        if self.is_detached() {
            self.replace_animation_controller(key, None);
            apply(target);
            return;
        }
        if let Some(target) = target.as_ref()
            && self.finishes_reduced_animation(key, target)
        {
            return;
        }
        if let Some(target) = target.as_ref()
            && self.continues_animation(key, target)
        {
            return;
        }
        let context = ActiveAnimationContext::current();
        if current == target && context.animation.is_none() {
            if self.has_animation_controller(key) {
                self.replace_animation_controller(key, None);
            }
            apply(target);
            return;
        }
        let animate = match (context.animation.clone(), current.clone(), target.clone()) {
            (Some(animation), Some(from), Some(to)) => Some((animation, from, to)),
            _ => None,
        };
        let Some((animation, from, to)) = animate else {
            self.replace_animation_controller(key, None);
            apply(target);
            return;
        };
        if self.prefers_reduced_motion() || from == to {
            self.replace_animation_controller(key, None);
            apply(target);
            return;
        }
        let apply = Rc::new(apply);
        let controller =
            AnimationController::new(from, to, animation, 0.0, move |value| apply(Some(value)));
        self.inherit_motion(key, &controller);
        let any: Rc<dyn AnyAnimationController> = controller.clone();
        self.replace_animation_controller(key, Some(any));
        if let Some(completion) = context.completion.as_ref() {
            completion.attach(&controller);
        }
        AnimatorScheduler::current().register(controller);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::{Animation, AnimationCurve};
    use std::cell::RefCell;

    #[test]
    fn writes_through_when_no_animation_is_active() {
        let node = Node::new();
        let written = Rc::new(RefCell::new(Vec::new()));
        let log = written.clone();
        node.animatable_set_with("opacity", 0.0_f32, 1.0, move |value| {
            log.borrow_mut().push(value)
        });
        assert_eq!(*written.borrow(), vec![1.0]);
        assert!(!node.has_animation_controller("opacity"));
    }

    #[test]
    fn active_animation_registers_a_controller() {
        let node = Node::new();
        let scheduler = AnimatorScheduler::current();
        scheduler.reset();
        let written = Rc::new(RefCell::new(Vec::new()));
        let log = written.clone();
        ActiveAnimationContext::with(
            Some(Animation::new(0.2, AnimationCurve::EaseOut)),
            None,
            None,
            || {
                node.animatable_set_with("opacity", 0.0_f32, 1.0, move |value| {
                    log.borrow_mut().push(value)
                });
            },
        );
        assert_eq!(scheduler.active_count(), 1);
        assert!(node.has_animation_controller("opacity"));
        assert!(
            written.borrow().is_empty(),
            "the value is written by the tick"
        );
        scheduler.reset();
    }

    #[test]
    fn optional_endpoints_snap_when_either_side_is_none() {
        let node = Node::new();
        let written = Rc::new(RefCell::new(Vec::new()));
        let log = written.clone();
        ActiveAnimationContext::with(
            Some(Animation::new(0.2, AnimationCurve::EaseOut)),
            None,
            None,
            || {
                node.animatable_set_optional::<f32>("color", None, Some(1.0), move |value| {
                    log.borrow_mut().push(value)
                });
            },
        );
        assert_eq!(*written.borrow(), vec![Some(1.0)]);
        assert!(!node.has_animation_controller("color"));
    }
}
