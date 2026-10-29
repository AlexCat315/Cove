//! Translation of CoveUIScene/Animation/AnimatableProperty.swift.
//!
//! Per-node registry for active per-property animation controllers.
//!
//! `Node::animatable_set` uses this to cancel a previous controller when the
//! same property is retargeted before completion, matching the Phase 8 rule
//! that new animations start from the current instantaneous value and older
//! ones are discarded.
//!
//! Swift keys this table with `AnyHashable`; Rust has no key paths, so the
//! ported modifiers name their property with a `&'static str`. The semantics —
//! one live controller per node per property — are unchanged.
use super::{
    ActiveAnimationContext, AnimatorScheduler, AnyAnimationController, AnimationController,
    Interpolatable, Node, NodeResource,
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::{Rc, Weak};

/// Attachment key holding the registry on the owning node.
pub const ANIMATABLE_PROPERTY_REGISTRY: &str = "__cove_animatable_property_registry";

struct WeakAnimationControllerBox {
    controller: Weak<dyn AnyAnimationController>,
    scheduler: AnimatorScheduler,
}

#[derive(Default)]
pub struct AnimatablePropertyRegistry {
    controllers: RefCell<HashMap<&'static str, WeakAnimationControllerBox>>,
}

impl AnimatablePropertyRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn count(&self) -> usize {
        self.controllers.borrow().len()
    }

    pub fn replace_controller(&self, key: &'static str, controller: Option<Rc<dyn AnyAnimationController>>) {
        let mut controllers = self.controllers.borrow_mut();
        if let Some(existing) = controllers.get(key).and_then(|entry| entry.controller.upgrade()) {
            existing.cancel();
            if let Some(entry) = controllers.get(key) {
                entry.scheduler.unregister(&existing);
            }
        }
        match controller {
            Some(controller) => {
                controllers.insert(
                    key,
                    WeakAnimationControllerBox {
                        controller: Rc::downgrade(&controller),
                        scheduler: AnimatorScheduler::current(),
                    },
                );
            }
            None => {
                controllers.remove(key);
            }
        }
    }

    /// The controller currently driving `key`, if it is still alive and
    /// unfinished. Peeked rather than taken: the successor reads the outgoing
    /// controller's motion *before* replacing it, so the rate is still there.
    pub fn live_controller(&self, key: &'static str) -> Option<Rc<dyn AnyAnimationController>> {
        let live = self
            .controllers
            .borrow()
            .get(key)
            .and_then(|entry| entry.controller.upgrade())
            .filter(|controller| !controller.is_finished());
        if live.is_none() {
            self.controllers.borrow_mut().remove(key);
        }
        live
    }

    pub fn has_active_controller(&self, key: &'static str) -> bool {
        self.live_controller(key).is_some()
    }
}

impl NodeResource for AnimatablePropertyRegistry {
    fn mount(&self, _node: &Node) {}
    fn unmount(&self, _node: &Node) {
        let entries: Vec<WeakAnimationControllerBox> = self.controllers.borrow_mut().drain().map(|(_, entry)| entry).collect();
        for entry in entries {
            if let Some(controller) = entry.controller.upgrade() {
                controller.cancel();
                entry.scheduler.unregister(&controller);
            }
        }
    }
    fn as_any_rc(self: Rc<Self>) -> Rc<dyn std::any::Any> {
        self
    }
}

impl Node {
    fn existing_animatable_property_registry(&self) -> Option<Rc<AnimatablePropertyRegistry>> {
        self.attachment::<Rc<AnimatablePropertyRegistry>>(ANIMATABLE_PROPERTY_REGISTRY)
    }

    fn animatable_property_registry(&self) -> Rc<AnimatablePropertyRegistry> {
        if let Some(existing) = self.existing_animatable_property_registry() {
            return existing;
        }
        let created = Rc::new(AnimatablePropertyRegistry::new());
        self.set_attachment(ANIMATABLE_PROPERTY_REGISTRY, created.clone());
        self.add_resource(created.clone());
        created
    }

    pub fn replace_animation_controller(&self, key: &'static str, controller: Option<Rc<dyn AnyAnimationController>>) {
        if controller.is_none() && self.existing_animatable_property_registry().is_none() {
            return;
        }
        self.animatable_property_registry().replace_controller(key, controller);
    }

    pub fn has_animation_controller(&self, key: &'static str) -> bool {
        self.existing_animatable_property_registry().is_some_and(|registry| registry.has_active_controller(key))
    }

    pub fn inherit_motion<T: Interpolatable + 'static>(&self, key: &'static str, controller: &Rc<AnimationController<T>>) {
        let Some(registry) = self.existing_animatable_property_registry() else { return };
        let Some(previous) = registry.live_controller(key) else { return };
        if let Some(previous) = previous.as_any().downcast_ref::<AnimationController<T>>() {
            controller.inherit_motion(previous);
        }
    }

    pub fn continues_animation<T: Interpolatable + PartialEq + 'static>(&self, key: &'static str, target: &T) -> bool {
        if self.prefers_reduced_motion() || ActiveAnimationContext::current().disables_animations {
            return false;
        }
        let Some(registry) = self.existing_animatable_property_registry() else { return false };
        let Some(controller) = registry.live_controller(key) else { return false };
        controller.as_any().downcast_ref::<AnimationController<T>>().is_some_and(|controller| &controller.to == target)
    }

    pub fn finishes_reduced_animation<T: Interpolatable + PartialEq + 'static>(&self, key: &'static str, target: &T) -> bool {
        if !self.prefers_reduced_motion() {
            return false;
        }
        let Some(registry) = self.existing_animatable_property_registry() else { return false };
        let Some(controller) = registry.live_controller(key) else { return false };
        let matches = controller.as_any().downcast_ref::<AnimationController<T>>().is_some_and(|controller| &controller.to == target);
        if !matches {
            return false;
        }
        controller.finish_immediately();
        self.replace_animation_controller(key, None);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::{Animation, AnimationCurve};

    #[test]
    fn retargeting_cancels_the_previous_controller() {
        let node = Node::new();
        let animation = Animation::new(0.3, AnimationCurve::EaseOut);
        let first = AnimationController::new(0.0_f32, 1.0, animation.clone(), 0.0, |_| {});
        let first_any: Rc<dyn AnyAnimationController> = first.clone();
        node.replace_animation_controller("opacity", Some(first_any));
        assert!(node.has_animation_controller("opacity"));

        let second = AnimationController::new(0.5_f32, 1.0, animation, 0.0, |_| {});
        node.replace_animation_controller("opacity", Some(second));
        assert!(first.is_finished(), "the outgoing controller must be cancelled");
        assert!(node.has_animation_controller("opacity"));
    }

    #[test]
    fn clearing_without_a_registry_is_a_no_op() {
        let node = Node::new();
        node.replace_animation_controller("opacity", None);
        assert!(node.existing_animatable_property_registry().is_none());
    }

    #[test]
    fn finished_controllers_are_pruned_on_lookup() {
        let node = Node::new();
        let controller = AnimationController::new(0.0_f32, 1.0, Animation::new(0.0, AnimationCurve::Linear), 0.0, |_| {});
        node.replace_animation_controller("opacity", Some(controller));
        assert!(!node.has_animation_controller("opacity"));
    }
}
