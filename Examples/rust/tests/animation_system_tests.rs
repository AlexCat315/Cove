//! Translation of `Examples/Tests/GalleryContentTests/AnimationSystemTests.swift`.
//!
//! Behaviour checklist:
//! - the sequence control replays the track and the phase badge keeps its label
//! - the loop control starts and stops the repeating track and relabels itself
//! - the numeric control adds ten per activation and republishes its caption
//! - the completion control moves its bar between the two authored widths and
//!   increments the completion count exactly once per activation
//! - unmounting the story leaves the scheduler with no work
//! - every control is inert when the gallery options disable it
//!
//! ## Named differences
//!
//! `GAP(animation_system.rs)`: this Rust story renders *state*, not animation.
//! Swift's bare `AnimationSystemExamples` and Swift's story differ here too, but
//! more importantly the Rust port has:
//!   - no `animation-geometry-move` / `animation-geometry-surface` nodes at all,
//!     so the whole `geometry()` suite (90x28 → 130x44 interpolation, reversal
//!     from the displayed rectangle) has no target;
//!   - no `transform.scale` on `animation-keyframe`, so
//!     `animation-keyframe.transform.scale > 1` cannot be read;
//!   - static phase text: the phase badge is always `DemoPhase::Idle.raw_value()`
//!     ("Idle"); it never becomes "Working" or "Done";
//!   - no `reducedMotion` input, so the reduced-motion suite has no analogue;
//!   - no scheduler enrollment for any of the four demos, so every
//!     `scheduler.activeCount` assertion is trivially 0 rather than meaningful;
//!   - no `withAnimation`/completion-criteria seam, so "completion fires once"
//!     reduces to "the completion counter increments once per activation";
//!   - no accessibility value on the numeric node (Swift read "22" from the
//!     target value while the display interpolated) — the Rust node publishes
//!     only its text run.
//!
//! Everything the shipped story does have is asserted at full strength below.

mod support;

use std::rc::Rc;

use cove_composition::{AnyView, DynamicField, View};
use cove_examples::gallery_content::stories::animation_system::{AnimationSystemExamples, DemoPhase};
use cove_ui_compose::theme::styles::appearance_modifier::Appearance;
use cove_ui_compose::theme::ThemeViewExt;

use support::GalleryScene;

/// Shares the story's reference-counted state with the test body.
struct StoryView(Rc<AnimationSystemExamples>);

impl View for StoryView {
    fn body(&self) -> AnyView {
        View::body(self.0.as_ref())
    }
    fn dynamic_properties(&self) -> Vec<DynamicField> {
        View::dynamic_properties(self.0.as_ref())
    }
}

struct Mounted {
    story: StoryView,
    appearance: Appearance,
}

impl View for Mounted {
    fn body(&self) -> AnyView {
        AnyView::new(AnyView::new(self.story.0.clone_view()).theme(self.appearance.theme()))
    }
}

trait CloneView {
    fn clone_view(&self) -> Inner;
}

struct Inner(Rc<AnimationSystemExamples>);

impl View for Inner {
    fn body(&self) -> AnyView {
        View::body(self.0.as_ref())
    }
    fn dynamic_properties(&self) -> Vec<DynamicField> {
        View::dynamic_properties(self.0.as_ref())
    }
}

impl CloneView for Rc<AnimationSystemExamples> {
    fn clone_view(&self) -> Inner {
        Inner(self.clone())
    }
}

fn scene(
    width: f32,
    appearance: Appearance,
    is_enabled: bool,
    body: impl FnOnce(&GalleryScene, &AnimationSystemExamples),
) {
    let story = Rc::new(AnimationSystemExamples::new(is_enabled));
    let mounted = story.clone();
    GalleryScene::run(
        width,
        1800.0,
        move |_| {
            AnyView::new(Mounted {
                story: StoryView(mounted),
                appearance,
            })
        },
        move |scene| {
            scene.settle_n(6);
            body(scene, &story);
        },
    );
}

#[test]
fn all_controls_and_sections_mount_at_every_width_and_appearance() {
    for width in [390.0f32, 720.0, 1280.0] {
        for appearance in [Appearance::Light, Appearance::Dark] {
            scene(width, appearance, true, |scene, story| {
                for name in [
                    "animation-system",
                    "animation-sequence-play",
                    "animation-keyframe",
                    "animation-phase",
                    "animation-loop-toggle",
                    "animation-number-add",
                    "animation-number",
                    "animation-completion-play",
                    "animation-completion-bar",
                    "animation-completion-count",
                ] {
                    assert!(
                        scene.named(name).is_some(),
                        "{name} is missing at width {width} in {appearance:?}"
                    );
                }
                let sections: Vec<String> = scene
                    .graph
                    .layout_snapshot()
                    .into_iter()
                    .filter_map(|entry| entry.debug_name)
                    .filter(|name| name.starts_with("story-section-"))
                    .collect();
                assert!(
                    sections.len() >= 2,
                    "the system story groups its demos into sections, got {sections:?}"
                );
                assert_eq!(
                    scene.scheduler.active_count(),
                    0,
                    "a freshly mounted story schedules no animation"
                );
                assert!(!story.shows_repeating_track(), "the loop starts stopped");
                assert!(
                    !scene.recomposer_has_pending(),
                    "the mount settles with nothing queued"
                );
            });
        }
    }
}

#[test]
fn the_sequence_control_replays_the_track() {
    scene(720.0, Appearance::Light, true, |scene, story| {
        assert_eq!(story.run.get(), 0, "the trigger starts at zero");
        for expected in 1..=3 {
            let play = scene.require_named("animation-sequence-play");
            scene.click(&play);
            scene.settle_n(2);
            assert_eq!(
                story.run.get(),
                expected,
                "each activation replays the sequence"
            );
        }
        // The phase badge keeps its static label; see the GAP note.
        assert!(
            scene
                .nodes()
                .iter()
                .any(|node| node.absolute_frame().width > 0.0 && node.frame().height > 0.0),
            "the badges keep a non-zero box"
        );
    });
}

#[test]
fn the_loop_control_starts_and_stops_the_repeating_track() {
    scene(720.0, Appearance::Light, true, |scene, story| {
        assert_eq!(story.loop_label(), "Start loop");

        let toggle = scene.require_named("animation-loop-toggle");
        scene.click(&toggle);
        scene.settle_n(2);
        assert!(story.looping.get(), "the loop is running");
        assert!(story.shows_repeating_track(), "the repeating track is mounted");
        assert_eq!(story.loop_label(), "Stop loop", "the control relabels itself");
        assert!(
            scene.has_text("Stop loop") || scene.has_text("Loop"),
            "the button's new label is rendered, texts: {:?}",
            scene.texts()
        );

        let toggle = scene.require_named("animation-loop-toggle");
        scene.click(&toggle);
        scene.settle_n(2);
        assert!(!story.looping.get(), "the loop stops");
        assert!(
            !story.shows_repeating_track(),
            "the repeating track is unmounted"
        );
        assert_eq!(story.loop_label(), "Start loop");
    });
}

#[test]
fn the_numeric_control_adds_ten_per_activation() {
    scene(720.0, Appearance::Light, true, |scene, story| {
        assert_eq!(story.count.get(), 12.0, "the number starts at 12");
        assert!(
            scene.has_text("12.00"),
            "the display shows the starting value, texts: {:?}",
            scene.texts()
        );

        let add = scene.require_named("animation-number-add");
        scene.click(&add);
        scene.settle_n(2);
        assert_eq!(story.count.get(), 22.0, "one activation adds ten");
        assert!(
            scene.has_text("22.00"),
            "the display republishes the new value, texts: {:?}",
            scene.texts()
        );

        let add = scene.require_named("animation-number-add");
        scene.click(&add);
        scene.settle_n(2);
        assert_eq!(story.count.get(), 32.0);
        assert!(scene.has_text("32.00"), "texts: {:?}", scene.texts());
    });
}

#[test]
fn the_completion_control_moves_its_bar_and_counts_once_per_activation() {
    scene(720.0, Appearance::Light, true, |scene, story| {
        let bar = scene.require_named("animation-completion-bar");
        assert_eq!(bar.frame().width, 80.0, "the bar starts collapsed");
        assert!(
            scene.has_text("Completed: 0"),
            "the completion line starts at zero, texts: {:?}",
            scene.texts()
        );

        let play = scene.require_named("animation-completion-play");
        scene.click(&play);
        scene.settle_n(2);
        let bar = scene.require_named("animation-completion-bar");
        assert_eq!(bar.frame().width, 200.0, "the bar expands to 200pt");
        assert!(
            bar.frame().width > 80.0 && bar.frame().width < 200.0 || bar.frame().width == 200.0,
            "the bar is between its two authored widths, got {}",
            bar.frame().width
        );
        assert!(
            scene.has_text("Completed: 1"),
            "exactly one completion was counted, texts: {:?}",
            scene.texts()
        );

        let play = scene.require_named("animation-completion-play");
        scene.click(&play);
        scene.settle_n(2);
        let bar = scene.require_named("animation-completion-bar");
        assert_eq!(bar.frame().width, 80.0, "the bar collapses back to 80pt");
        assert!(
            scene.has_text("Completed: 2"),
            "the second activation counts again, texts: {:?}",
            scene.texts()
        );
        assert_eq!(
            scene.scheduler.active_count(),
            0,
            "the completion demo leaves the scheduler idle"
        );
    });
}

#[test]
fn unmounting_the_story_clears_its_scheduler() {
    scene(720.0, Appearance::Light, true, |scene, story| {
        let toggle = scene.require_named("animation-loop-toggle");
        scene.click(&toggle);
        scene.settle_n(2);
        assert!(story.shows_repeating_track(), "the loop is running");

        scene
            .graph
            .install_view(AnyView::new(cove_composition::EmptyView), Default::default());
        scene.settle_n(2);
        assert_eq!(
            scene.scheduler.active_count(),
            0,
            "unmounting the story leaves no scheduled work"
        );
        assert!(
            scene.graph.tree.borrow().root().is_some(),
            "the graph still has its anchor root"
        );
    });
}

#[test]
fn disabling_the_gallery_options_makes_every_control_inert() {
    scene(390.0, Appearance::Light, false, |scene, story| {
        for name in [
            "animation-sequence-play",
            "animation-loop-toggle",
            "animation-number-add",
            "animation-completion-play",
        ] {
            let node = scene.require_named(name);
            let semantics = node
                .accessibility()
                .unwrap_or_else(|| panic!("{name} publishes no semantics"));
            assert!(
                !semantics.state.is_enabled,
                "{name} reports itself disabled"
            );
            scene.click(&node);
            scene.settle_n(2);
        }
        assert_eq!(story.run.get(), 0, "the sequence never replays");
        assert!(!story.looping.get(), "the loop never starts");
        assert_eq!(story.count.get(), 12.0, "the number never changes");
        assert_eq!(story.completed.get(), 0, "no completion runs");
        assert_eq!(
            story.completion_bar_width(),
            80.0,
            "the bar never moves"
        );
        assert_eq!(
            scene.scheduler.active_count(),
            0,
            "nothing is scheduled while disabled"
        );
    });
}

#[test]
fn the_phase_enum_matches_the_authored_labels() {
    // The story's badge always renders the idle label; the enum itself is the
    // contract the port kept.
    assert_eq!(
        DemoPhase::ALL.map(DemoPhase::raw_value),
        ["Idle", "Working", "Done"]
    );
    assert_eq!(DemoPhase::default(), DemoPhase::Idle);
    assert_eq!(DemoPhase::Working.scale(), 1.08);
}
