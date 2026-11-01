//! Shared animated value state for determinate progress indicators.
use crate::theme::{SemanticMotionRef, theme_for_node};
use cove_core::scene::{ActiveAnimationContext, Animation, Node, NodeResource};
use std::cell::Cell;
use std::rc::{Rc, Weak};

/// Node-owned progress state. Value changes invalidate drawing without
/// rebuilding the indicator's layout or view subtree.
pub struct ProgressValue {
    value: Cell<f64>,
    is_determinate: Cell<bool>,
}

impl ProgressValue {
    pub fn new() -> Rc<Self> {
        Rc::new(Self {
            value: Cell::new(0.0),
            is_determinate: Cell::new(false),
        })
    }

    pub fn value(&self) -> f64 {
        self.value.get()
    }

    pub fn update(self: &Rc<Self>, node: &Node, target: f64, determinate: bool) {
        let was_determinate = self.is_determinate.replace(determinate);
        if !was_determinate || !determinate {
            node.cancel_animation("progress.value");
            self.value.set(target);
            return;
        }

        let current = ActiveAnimationContext::current();
        let write = || {
            let weak_self: Weak<Self> = Rc::downgrade(self);
            let weak_node = node.downgrade();
            node.animatable_set_with("progress.value", self.value.get(), target, move |value| {
                let Some(session) = weak_self.upgrade() else {
                    return;
                };
                if session.value.replace(value) == value {
                    return;
                }
                if let Some(node) = Node::upgrade(&weak_node) {
                    node.mark_render_dirty(cove_core::scene::InvalidationSource::style_set(
                        "progress.value",
                    ));
                }
            });
        };
        if current.animation.is_some() || current.disables_animations {
            write();
        } else {
            let animation: Animation = SemanticMotionRef::medium().resolve(&theme_for_node(node));
            ActiveAnimationContext::with(Some(animation), current.completion, Some(false), write);
        }
    }
}

impl Default for ProgressValue {
    fn default() -> Self {
        Self {
            value: Cell::new(0.0),
            is_determinate: Cell::new(false),
        }
    }
}

impl NodeResource for ProgressValue {
    fn mount(&self, _node: &Node) {}

    fn unmount(&self, node: &Node) {
        node.cancel_animation("progress.value");
    }

    fn as_any_rc(self: Rc<Self>) -> Rc<dyn std::any::Any> {
        self
    }
}
