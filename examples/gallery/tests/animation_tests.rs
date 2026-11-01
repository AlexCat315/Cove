//! Translation of `Examples/Tests/GalleryContentTests/AnimationTests.swift`.
//!
//! Behaviour checklist:
//! - every curve choice is selectable and marks itself selected
//! - playing moves the bar from its collapsed 80pt width to the expanded 200pt
//!   width and back, and leaves the scheduler idle
//! - reflowing the surface at 390 / 720 / 1280 in both appearances changes no
//!   geometry fact the story relies on
//! - a disabled gallery option removes both the curve toggles and playback
//!
//! ## Named differences
//!
//! `GAP(animation_curves.rs)`: the Rust curve story is a *layout* story, not a
//! driver of `AnimatorScheduler`. `AnimationCurveExamples::body` renders a button
//! that flips `expanded`, and the bar's width is `target_width()` — one of two
//! authored points (80 / 200). There is no interpolated intermediate frame, no
//! `Animation::duration`, and no scheduler enrollment, so the Swift assertions
//! that read an intermediate width after `scheduler.tick(0.2)` and the
//! `scheduler.activeCount == 0` checks cannot be reproduced: the intermediate
//! widths (104 / 137.6 / 116.3 / 230 and their 280 - x reverse values) do not
//! exist in this port. What is asserted here is the two-state contract the
//! shipped story actually has, plus the reverse direction, which does hold.
//!
//! `GAP(animation_curves.rs)`: `AnimationCurveExamples` has no
//! `reducedMotion` input, so Swift's reduced-motion suite has no analogue. The
//! equivalent claim — reduced motion must not interpolate — is vacuous when
//! nothing interpolates, so it is recorded rather than faked.

use cove_composition::Scope;
mod support;

use cove_composition::AnyView;
use cove_examples::gallery_content::stories::animation_curves::{
    AnimationCurveExamples, CurveChoice,
};
use cove_ui_compose::theme::ThemeViewExt;
use cove_ui_compose::theme::styles::appearance_modifier::Appearance;

use std::rc::Rc;

use support::GalleryScene;

/// A `View` wrapper so the test keeps a handle on the state it mounted.
///
/// `AnimationCurveExamples` is not `Clone`; its `State` values are reference
/// counted, so the story is shared through an `Rc` and read back through the
/// same handle after mounting.
struct Mounted {
    story: Rc<AnimationCurveExamples>,
    appearance: Appearance,
}

impl cove_composition::View for Mounted {
    fn body(&self, _cx: &Scope) -> AnyView {
        AnyView::new(AnyView::new(self.story.clone_view()).theme(self.appearance.theme()))
    }
}

/// `AnyView::new` needs an owned view; this hands one out per body call.
trait CloneView {
    fn clone_view(&self) -> StoryView;
}

struct StoryView(Rc<AnimationCurveExamples>);

impl cove_composition::View for StoryView {
    fn body(&self, cx: &Scope) -> AnyView {
        cove_composition::View::body(self.0.as_ref(), cx)
    }
    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        cove_composition::View::dynamic_properties(self.0.as_ref())
    }
}

impl CloneView for Rc<AnimationCurveExamples> {
    fn clone_view(&self) -> StoryView {
        StoryView(self.clone())
    }
}

fn scene(
    width: f32,
    appearance: Appearance,
    is_enabled: bool,
    body: impl FnOnce(&GalleryScene, &AnimationCurveExamples),
) {
    let story = Rc::new(AnimationCurveExamples::new(is_enabled));
    let mounted = story.clone();
    GalleryScene::run(
        width,
        700.0,
        move |_| {
            AnyView::new(Mounted {
                story: mounted,
                appearance,
            })
        },
        move |scene| body(scene, &story),
    );
}

#[test]
fn every_curve_is_selectable_and_reports_itself_selected() {
    scene(390.0, Appearance::Light, true, |scene, story| {
        for choice in CurveChoice::ALL {
            let toggle = scene.require_named(&AnimationCurveExamples::toggle_debug_name(choice));
            assert!(
                toggle.is_hit_testable(),
                "{choice:?} is interactive while enabled"
            );
            scene.click(&toggle);
            scene.settle_n(2);
            assert_eq!(story.choice.get(), choice, "clicking {choice:?} selects it");
            let refreshed = scene.require_named(&AnimationCurveExamples::toggle_debug_name(choice));
            assert_eq!(
                refreshed
                    .accessibility()
                    .map(|semantics| semantics.state.is_selected),
                Some(true),
                "{choice:?} marks itself selected"
            );
        }
    });
}

#[test]
fn playing_moves_the_bar_between_its_two_authored_widths_in_both_directions() {
    for width in [390.0f32, 720.0, 1280.0] {
        for appearance in [Appearance::Light, Appearance::Dark] {
            scene(width, appearance, true, |scene, _| {
                let bar = scene.require_named("animation-curve-bar");
                assert_eq!(
                    bar.frame().width,
                    80.0,
                    "the bar starts collapsed at width {width}"
                );

                for expected in [200.0f32, 80.0] {
                    let play = scene.require_named("animation-curve-play");
                    scene.click(&play);
                    scene.settle_n(2);
                    let bar = scene.require_named("animation-curve-bar");
                    assert_eq!(
                        bar.frame().width,
                        expected,
                        "play moves the bar to {expected}pt at width {width}"
                    );
                    assert_eq!(
                        bar.frame().height,
                        28.0,
                        "the bar keeps its authored height"
                    );
                    assert_eq!(
                        scene.scheduler.active_count(),
                        0,
                        "the story schedules no animation, so the scheduler stays idle"
                    );
                }
            });
        }
    }
}

#[test]
fn the_caption_follows_the_selected_curve_and_width() {
    scene(390.0, Appearance::Light, true, |scene, story| {
        assert_eq!(story.caption(), "Steps · target width 80pt");
        assert!(scene.has_text("Steps · target width 80pt"));

        let play = scene.require_named("animation-curve-play");
        scene.click(&play);
        scene.settle_n(2);
        assert_eq!(story.caption(), "Steps · target width 200pt");
        assert!(
            scene.has_text("Steps · target width 200pt"),
            "the caption republishes after playback, texts: {:?}",
            scene.texts()
        );

        let elastic = scene.require_named(&AnimationCurveExamples::toggle_debug_name(
            CurveChoice::Elastic,
        ));
        scene.click(&elastic);
        scene.settle_n(2);
        assert!(
            scene.has_text("Elastic out · target width 200pt"),
            "the caption follows the chosen curve, texts: {:?}",
            scene.texts()
        );
    });
}

#[test]
fn disabled_options_remove_both_curve_and_playback_activation() {
    scene(390.0, Appearance::Light, false, |scene, story| {
        let mut names = vec!["animation-curve-play".to_owned()];
        names.extend(
            CurveChoice::ALL
                .iter()
                .map(|choice| AnimationCurveExamples::toggle_debug_name(*choice)),
        );
        for name in &names {
            let node = scene.require_named(name);
            let semantics = node
                .accessibility()
                .unwrap_or_else(|| panic!("{name} publishes no semantics"));
            assert!(
                !semantics.state.is_enabled,
                "{name} reports itself disabled"
            );
        }

        // A disabled control must not change state when pressed.
        let before = story.target_width();
        let play = scene.require_named("animation-curve-play");
        scene.click(&play);
        scene.settle_n(2);
        assert_eq!(
            story.target_width(),
            before,
            "pressing a disabled play control does nothing"
        );
        let bar = scene.require_named("animation-curve-bar");
        assert_eq!(bar.frame().width, 80.0, "the bar stays collapsed");
        assert_eq!(
            scene.scheduler.active_count(),
            0,
            "no animation is scheduled"
        );
    });
}

#[test]
fn the_story_renders_inside_a_titled_section() {
    scene(720.0, Appearance::Light, true, |scene, _| {
        let sections: Vec<String> = scene
            .graph
            .layout_snapshot()
            .into_iter()
            .filter_map(|entry| entry.debug_name)
            .filter(|name| name.starts_with("story-section-"))
            .collect();
        assert_eq!(
            sections,
            vec!["story-section-Timing curves".to_owned()],
            "the curve story is one titled section"
        );
        assert!(
            scene.has_text(
                "Choose a curve and play in either direction. Steps move in intervals; \
                 linear points can plateau and overshoot; bounce and elastic settle after \
                 oscillating."
            ),
            "the section keeps its authored detail"
        );
    });
}
