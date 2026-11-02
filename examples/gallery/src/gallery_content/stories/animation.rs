//! The `animation` page: animated visibility plus the curve and system examples.
//!
//! Swift source: the `AnimationStory` struct in
//! `GalleryContent/Stories/Layout/LayoutStories.swift`. The story composes the
//! three `Foundation` animation stories, so this module owns the shared state and
//! delegates the host-supplied sections.

use cove_composition::Scope;
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

impl cove_ui_compose::View for AnimationStory {
    fn body(&self, _cx: &Scope) -> cove_ui_compose::AnyView {
        use cove_core::scene::{Align, FlexDirection, Justify, Wrap};
        use cove_ui_compose::modifiers::layout::FlexExt;
        use cove_ui_compose::primitives::feedback::Alert;
        use cove_ui_compose::primitives::text::Text;
        use cove_ui_compose::theme::styles::buttons::SecondaryButtonStyle;
        use cove_ui_compose::theme::styles::buttons::button_style::ButtonStyleViewExt;
        use cove_ui_compose::{AnyView, BoxView, LayoutDebugExt, TupleView};

        use crate::gallery_content::stories::StorySection;

        let enabled = self.options.is_enabled;
        let toggle_label = self.toggle_label();
        let visible = self.visible.clone();
        let toggle =
            cove_ui_compose::primitives::controls::Button::title(toggle_label, move || {
                visible.update(|value| *value = !*value);
            })
            .enabled(enabled)
            .button_style(SecondaryButtonStyle);

        // The retained subtree survives its exit animation.
        let animated = self.content();
        let section = StorySection::new(
            "Animated visibility",
            "The component retains the subtree through its exit animation and cleans up after settling.",
            AnyView::new(BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                12.0,
                AnyView::new(TupleView::new(vec![
                    AnyView::new(toggle),
                    AnyView::new(animated),
                    AnyView::new(
                        Alert::new("Animated content")
                            .message("Try toggling repeatedly during the transition.")
                            .tone(cove_ui_compose::primitives::feedback::StatusTone::Success),
                    ),
                    AnyView::new(Text::new(
                        "Timing curves and the animation system are exercised by the sections below.",
                    )),
                ])),
            )),
        );

        AnyView::new(
            BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                24.0,
                AnyView::new(TupleView::new(vec![
                    AnyView::new(section),
                    AnyView::new(
                        crate::gallery_content::stories::AnimationCurveExamples::new(enabled),
                    ),
                    AnyView::new(
                        crate::gallery_content::stories::AnimationSystemExamples::new(enabled),
                    ),
                ])),
            )
            .flex(0.0)
            .debug_name("animation-story"),
        )
    }
}

/// Stands in for the Swift `Alert("Animated content", …)` inside the animated
/// subtree; the alert widget is ported, the story only needs a body.
/// The retained subtree's body: a small card the exit animation keeps alive.
struct AnimationDemoNote;

impl cove_ui_compose::View for AnimationDemoNote {
    fn body(&self, _cx: &Scope) -> cove_ui_compose::AnyView {
        use cove_ui_compose::primitives::text::Text;
        use cove_ui_compose::theme::tokens::{SemanticColorRef, SemanticStyleViewExt};
        use cove_ui_compose::{AnyView, BoxView, TupleView};

        AnyView::new(BoxView::new(
            cove_core::scene::FlexDirection::Column,
            cove_core::scene::Align::FlexStart,
            cove_core::scene::Justify::FlexStart,
            cove_core::scene::Wrap::NoWrap,
            4.0,
            AnyView::new(TupleView::new(vec![
                AnyView::new(Text::new("Content stays mounted")),
                AnyView::new(
                    Text::new("The note keeps its identity through the exit animation.")
                        .semantic_foreground(SemanticColorRef::on_surface_muted()),
                ),
            ])),
        ))
    }

    fn tag(&self) -> &'static str {
        "AnimationDemoNote"
    }
}

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
