//! The `animation` page: animated visibility plus the curve and system examples.
//!
//! Swift source: the `AnimationStory` struct in
//! `GalleryContent/Stories/Layout/LayoutStories.swift`. The story composes the
//! three `Foundation` animation stories, so this module owns the shared state and
//! delegates the host-supplied sections.

use cove_core::State;
use cove_ui_compose::animation::content::animated_visibility::AnimatedVisibility;
use cove_ui_compose::animation::transition::{Transition, TransitionEdge};

use crate::gallery_content::catalog::GalleryOptions;
use crate::gallery_content::stories::animation_curves::AnimationCurveExamples;
use crate::gallery_content::stories::animation_system::AnimationSystemExamples;

/// The animated-visibility demo state.
pub struct AnimationStory {
    pub options: GalleryOptions,
    pub visible: State<bool>,
}

impl AnimationStory {
    pub fn new(options: GalleryOptions) -> Self {
        Self {
            options,
            visible: State::new(true),
        }
    }

    /// Toggles the retained subtree, matching the story's button.
    pub fn toggle(&self) {
        self.visible.update(|visible| *visible = !*visible);
    }

    /// The button label reflects the current visibility.
    pub fn toggle_label(&self) -> &'static str {
        if self.visible.get() {
            "Hide content"
        } else {
            "Show content"
        }
    }

    /// `.opacity.combined(with: .move(edge: .top, distance: 12))`
    pub fn transition() -> Transition {
        Transition::opacity().combined(Transition::move_edge(TransitionEdge::Top, 12.0))
    }

    /// The visibility section's content, retained through the exit animation.
    pub fn content(&self) -> AnimatedVisibility {
        AnimatedVisibility::new(
            self.visible.get(),
            Self::transition(),
            None,
            false,
            None,
            AnimationDemoNote,
        )
    }

    pub fn curve_examples(&self) -> AnimationCurveExamples {
        AnimationCurveExamples::new(self.options.is_enabled)
    }

    pub fn system_examples(&self) -> AnimationSystemExamples {
        AnimationSystemExamples::new(self.options.is_enabled)
    }
}

/// Stands in for the Swift `Alert("Animated content", …)` inside the animated
/// subtree; the alert widget is ported, the story only needs a body.
struct AnimationDemoNote;

impl cove_ui_compose::View for AnimationDemoNote {}

#[cfg(test)]
mod checks {
    use super::*;

    #[test]
    fn toggle_flips_visibility_and_label() {
        let story = AnimationStory::new(GalleryOptions::default());
        assert!(story.visible.get());
        assert_eq!(story.toggle_label(), "Hide content");
        story.toggle();
        assert!(!story.visible.get());
        assert_eq!(story.toggle_label(), "Show content");
        story.toggle();
        assert!(story.visible.get());
    }

    #[test]
    fn transition_combines_opacity_with_a_twelve_point_top_move() {
        let combined = AnimationStory::transition();
        // The story must combine both effects, not substitute one for the other.
        assert_eq!(
            combined,
            Transition::opacity().combined(Transition::move_edge(TransitionEdge::Top, 12.0))
        );
        assert_ne!(combined, Transition::opacity());
        assert_ne!(combined, Transition::collapse());
    }

    #[test]
    fn sections_inherit_the_gallery_enablement() {
        let enabled = AnimationStory::new(GalleryOptions::default());
        assert!(enabled.curve_examples().is_enabled);
        assert!(enabled.system_examples().is_enabled);

        let disabled = AnimationStory::new(GalleryOptions {
            is_enabled: false,
            ..GalleryOptions::default()
        });
        assert!(!disabled.curve_examples().is_enabled);
        assert!(!disabled.system_examples().is_enabled);
    }
}
