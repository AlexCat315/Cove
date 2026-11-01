//! Translation of Animation/AnimationValueModifier.swift.
use crate::theme::{SemanticMotionRef, theme_for_node};
use crate::{ModifiedContent, View, ViewExt, ViewModifier};
use cove_core::scene::{ActiveAnimationContext, Animation, Node};

const ANIMATION_VALUE_CACHE_KEY: &str = "cove.ui.animation.lastValue";

enum AnimationSource {
    Fixed(Option<Animation>),
    Motion(SemanticMotionRef),
}

/// Activates an implicit animation when the keyed value changes between
/// materialisation passes. The initial pass intentionally uses no animation.
pub struct AnimationValueModifier<Value: Clone + PartialEq + 'static> {
    animation: AnimationSource,
    value: Value,
}

impl<Value: Clone + PartialEq + 'static> AnimationValueModifier<Value> {
    pub fn new(animation: Option<Animation>, value: Value) -> Self {
        Self {
            animation: AnimationSource::Fixed(animation),
            value,
        }
    }

    pub fn with_motion(motion: SemanticMotionRef, value: Value) -> Self {
        Self {
            animation: AnimationSource::Motion(motion),
            value,
        }
    }

    fn resolved_animation(&self, node: &Node) -> Option<Animation> {
        match &self.animation {
            AnimationSource::Fixed(animation) => animation.clone(),
            AnimationSource::Motion(motion) => Some(motion.resolve(&theme_for_node(node))),
        }
    }
}

impl<Value: Clone + PartialEq + 'static> ViewModifier for AnimationValueModifier<Value> {
    fn around_apply(&self, node: &Node, perform: &mut dyn FnMut()) -> bool {
        let previous = node.attachment::<Value>(ANIMATION_VALUE_CACHE_KEY);
        let changed = previous
            .as_ref()
            .is_some_and(|previous| previous != &self.value);
        node.set_attachment(ANIMATION_VALUE_CACHE_KEY, self.value.clone());
        if changed {
            let animation = self.resolved_animation(node);
            let disables = animation.is_none();
            ActiveAnimationContext::with(animation, None, Some(disables), perform);
        } else {
            perform();
        }
        true
    }

    fn is_around_applying(&self) -> bool {
        true
    }
}

pub trait AnimationValueExt: View + Sized + 'static {
    fn animation_value<Value: Clone + PartialEq + 'static>(
        self,
        animation: Option<Animation>,
        value: Value,
    ) -> ModifiedContent {
        self.modifier(AnimationValueModifier::new(animation, value))
    }

    fn animation_motion<Value: Clone + PartialEq + 'static>(
        self,
        motion: SemanticMotionRef,
        value: Value,
    ) -> ModifiedContent {
        self.modifier(AnimationValueModifier::with_motion(motion, value))
    }
}
impl<T: View + Sized + 'static> AnimationValueExt for T {}
