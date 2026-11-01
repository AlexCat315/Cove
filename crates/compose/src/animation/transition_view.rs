//! Compatibility facade around the retained AnimatedVisibility lifecycle.
use super::{AnimatedVisibility, Transition};
use crate::Scope;
use crate::theme::SemanticMotionRef;
use crate::{AnyView, View};
use std::time::Duration;

pub struct TransitionView {
    pub is_visible: bool,
    pub transition: Transition,
    pub removal: Option<Transition>,
    pub motion: SemanticMotionRef,
    pub stagger_index: usize,
    pub stagger_interval: Duration,
    content: AnyView,
}

impl TransitionView {
    pub fn new<Content: View + 'static>(
        is_visible: bool,
        transition: Transition,
        removal: Option<Transition>,
        motion: SemanticMotionRef,
        stagger_index: usize,
        stagger_interval: Duration,
        content: impl FnOnce() -> Content,
    ) -> Self {
        Self {
            is_visible,
            transition,
            removal,
            motion,
            stagger_index,
            stagger_interval,
            content: AnyView::new(content()),
        }
    }
}

impl View for TransitionView {
    fn body(&self, _cx: &Scope) -> AnyView {
        let transition =
            Transition::asymmetric(self.transition, self.removal.unwrap_or(self.transition));
        AnyView::new(
            AnimatedVisibility::with_motion(
                self.is_visible,
                transition,
                Some(self.motion.clone()),
                true,
                None,
                self.content.clone(),
            )
            .staggered(self.stagger_index, self.stagger_interval),
        )
    }
}
