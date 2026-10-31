//! Translation of `Tests/CoveUIComposeTests/DemoCrossFadeTests.swift`.
//!
//! Behaviour checklist:
//! - a subtree with `.appearance(_)` and a semantic background resolves the
//!   active appearance's background colour
//! - wrapping it in `.animation_value(_:value:)` registers a scheduler
//!   animation when the appearance changes, and the background is still the
//!   old colour at t = 0
//! - halfway through the animation the background is neither endpoint
//! - when the animation finishes the background is the new colour and the
//!   scheduler is idle again

mod support;

use cove_composition::{AnyView, View, ViewExt};
use cove_core::State;
use cove_core::scene::{AnimatorScheduler, Animation, Node};
use cove_ui_compose::modifiers::animation_value::AnimationValueExt;
use cove_ui_compose::modifiers::style::SemanticStyleViewExt;
use cove_ui_compose::theme::SemanticColorRef;
use cove_ui_compose::theme::styles::appearance_modifier::{Appearance, AppearanceViewExt};
use cove_ui_compose::theme::styles::circular_appearance::ThemeTransition;
use cove_ui_compose::theme::Theme;

use support::{DebugNode, GlobalTestLock};

fn find_filled(root: &Node) -> Option<Node> {
    if root.background_color().is_some_and(|color| color.a > 0.0) {
        return Some(root.clone());
    }
    root.children().iter().find_map(find_filled)
}

/// Mirrors the Swift `CrossFadeHarness`: `@State appearance` and a body that
/// re-applies the animation modifer with the current value.
struct CrossFadeHarness {
    appearance: State<Appearance>,
}

impl View for CrossFadeHarness {
    fn body(&self) -> AnyView {
        let appearance = self.appearance.get();
        AnyView::new(
            DebugNode::new("x")
                .semantic_background(SemanticColorRef::background())
                .appearance(appearance, ThemeTransition::default())
                .animation_value(Some(Animation::ease_in_out(0.30)), appearance),
        )
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        vec![cove_composition::DynamicField::state(
            "appearance",
            &self.appearance,
        )]
    }
}

#[test]
fn appearance_toggle_cross_fades_the_background() {
    GlobalTestLock::with_scheduler(|| {
        let dark = Theme::default_dark().colors.background;
        let light = Theme::default_light().colors.background;
        assert_ne!(dark, light, "the two presets differ");

        let appearance = State::new(Appearance::Dark);
        let graph = support::graph();
        graph.install_view(
            AnyView::new(CrossFadeHarness {
                appearance: appearance.clone(),
            }),
            Default::default(),
        );

        let filled = {
            let root = graph.tree.borrow().root().expect("root");
            find_filled(&root).expect("a filled node mounts")
        };
        assert_eq!(
            filled.background_color(),
            Some(dark),
            "the dark appearance paints the dark background"
        );
        assert_eq!(
            AnimatorScheduler::current().active_count(),
            0,
            "the initial pass registers no animation"
        );

        appearance.set(Appearance::Light);
        assert!(
            graph.recomposer.has_pending(),
            "writing the appearance state schedules a recomposition"
        );
        graph.recomposer.commit_all();

        assert!(
            AnimatorScheduler::current().active_count() >= 1,
            "changing the appearance registers an animation"
        );
        assert_eq!(
            filled.background_color(),
            Some(dark),
            "the background still reads dark at t = 0"
        );

        AnimatorScheduler::current().tick(0.15);
        let mid = filled.background_color().expect("a colour mid-flight");
        assert_ne!(mid, dark, "the midpoint leaves the dark endpoint");
        assert_ne!(mid, light, "the midpoint has not reached the light endpoint");

        AnimatorScheduler::current().tick(0.15);
        assert_eq!(
            filled.background_color(),
            Some(light),
            "the animation lands on the light background"
        );
        assert_eq!(
            AnimatorScheduler::current().active_count(),
            0,
            "the scheduler is idle once the animation completes"
        );
    });
}
