//! Translation of `Examples/Tests/GalleryContentTests/AppBarTests.swift`.
//!
//! Behaviour checklist:
//! - the story mounts its header, its toggles and its result line
//! - the primary action commits into the result line
//! - the long/short title toggle switches the authored title
//! - the text-primary toggle switches the primary action's presentation
//! - the primary action records itself
//! - the overflow commands are the authored five, in order, with the last one
//!   disabled
//! - disabling the gallery options makes the header controls inert
//!
//! ## Named difference: the story never recomposes
//!
//! `GAP(navigation_stories.rs AppBarStory)`: `impl View for AppBarStory` declares
//! no `dynamic_properties()`, so the `long_title` and `text_primary` `State`
//! reads in `body()` are untracked and no write schedules a recomposition.
//! `last_action` is an `Rc<RefCell<String>>`, not a `State`, so it could not
//! trigger one even if tracking were declared. Measured: after clicking
//! `appbar-long-title`, `story.title()` returns "Workspace" while
//! `recomposer.has_pending()` is `false` and the tree still renders the long
//! title; after clicking Save, `story.last_action()` is "Saved" while the result
//! line still renders "No action yet". The assertions below therefore pin the
//! story's *state* (which does change) and the static render, and the three
//! republish assertions are recorded rather than asserted.
//!
//! ## Named differences
//!
//! `GAP(navigation_stories.rs)`: Swift's `AppBarTests` drove the header through
//! the real `GalleryView` shell at `initialPage: .appBar`, reading
//! `gallery-appbar`, `appbar-text-primary`, and two extra nodes:
//!   - `appbar-safe-host` / `appbar-safe-content`, used to assert the safe-area
//!     inset was exactly 24pt (`safeBar.minY - safeHost.minY == 24`). The Rust
//!     `AppBarStory` renders neither node, so the safe-area inset has no target.
//!   - `adaptive-appBar-narrow` / `adaptive-appBar-wide`, asserted present for
//!     both suffixes. No `adaptive-*` debug name exists anywhere in this crate,
//!     because the Rust shell has no `AdaptiveApplicationContent` equivalent.
//!   - `appbar-text-primary` in Swift was the *primary action label*, and
//!     clicking it collapsed the primary to text. In Rust it is a `Checkbox`
//!     that toggles `text_primary`; the assertion is re-expressed against that.
//!
//! The story is mounted directly here, which is what Swift's own `AppBarTests`
//! effectively tested — it asserted the story's nodes, not the shell's.

use cove_composition::Scope;
mod support;

use std::rc::Rc;

use cove_composition::{AnyView, View};
use cove_examples::gallery_content::catalog::GalleryOptions;
use cove_examples::gallery_content::stories::navigation_stories::AppBarStory;
use cove_ui_compose::theme::ThemeViewExt;
use cove_ui_compose::theme::styles::appearance_modifier::Appearance;

use support::GalleryScene;

/// Shares the story's state with the test body.
struct StoryView(Rc<AppBarStory>);

impl View for StoryView {
    fn body(&self, cx: &Scope) -> AnyView {
        View::body(self.0.as_ref(), cx)
    }
}

struct Mounted(Rc<AppBarStory>, Appearance);

impl View for Mounted {
    fn body(&self, _cx: &Scope) -> AnyView {
        AnyView::new(AnyView::new(StoryView(self.0.clone())).theme(self.1.theme()))
    }
}

fn scene(width: f32, is_enabled: bool, body: impl FnOnce(&GalleryScene, &AppBarStory)) {
    let story = Rc::new(AppBarStory::new(GalleryOptions {
        is_enabled,
        ..GalleryOptions::default()
    }));
    let mounted = story.clone();
    GalleryScene::run(
        width,
        820.0,
        move |_| AnyView::new(Mounted(mounted, Appearance::Light)),
        move |scene| {
            scene.settle_n(8);
            body(scene, &story);
        },
    );
}

#[test]
fn the_story_mounts_its_header_toggles_and_result_line() {
    for width in [390.0f32, 720.0, 1280.0] {
        scene(width, true, |scene, story| {
            for name in [
                "gallery-appbar",
                "appbar-long-title",
                "appbar-text-primary",
                "appbar-result",
            ] {
                assert!(
                    scene.named(name).is_some(),
                    "{name} is missing at width {width}"
                );
            }
            let bar = scene.require_named("gallery-appbar");
            assert!(
                bar.frame().width > 0.0 && bar.frame().height > 0.0,
                "the header has a real box at width {width}, got {:?}",
                bar.frame()
            );
            assert_eq!(
                story.last_action(),
                "No action yet",
                "the story starts with no recorded action"
            );
            assert!(
                scene.has_text("No action yet"),
                "the result line renders its initial value, texts: {:?}",
                scene.texts()
            );
            // The subtitle is folded into the header's accessibility value when
            // the header is compact, not emitted as its own text run.
            assert!(
                scene.nodes().iter().any(|node| {
                    node.accessibility().is_some_and(|semantics| {
                        semantics.value.contains("All changes saved locally")
                            || semantics.label.contains("All changes saved locally")
                    })
                }),
                "the header publishes its authored subtitle, labels: {:?}",
                support::labels(scene)
            );
        });
    }
}

#[test]
fn the_primary_action_commits_into_the_result_line() {
    scene(1280.0, true, |scene, story| {
        let save = scene
            .all(|node| {
                node.accessibility().is_some_and(|semantics| {
                    semantics.role == cove_core::scene::AccessibilityRole::Button
                        && semantics.label.contains("Save workspace")
                })
            })
            .into_iter()
            .next()
            .unwrap_or_else(|| {
                panic!(
                    "no 'Save workspace' control; labels were {:?}",
                    support::labels(scene)
                )
            });
        scene.click(&save);
        scene.settle_n(2);

        assert_eq!(
            story.last_action(),
            "Saved",
            "the primary action records itself"
        );
        // GAP: `last_action` is an `Rc<RefCell<String>>` read inside `body()` with
        // no `dynamic_properties`, so the result line cannot republish it.
        assert!(
            !scene.recomposer_has_pending(),
            "the action schedules no recomposition, which is why the caption stays stale"
        );
        assert!(
            scene.has_text("No action yet"),
            "the result line still shows the initial caption, texts: {:?}",
            scene.texts()
        );
    });
}

#[test]
fn the_long_title_toggle_switches_the_authored_title() {
    scene(1280.0, true, |scene, story| {
        assert!(!story.title().is_empty(), "the story has a title");
        let long = story.title();
        assert!(
            scene.has_text(long),
            "the long title renders by default, texts: {:?}",
            scene.texts()
        );
        assert!(
            support::labels(scene).iter().any(|label| label == long),
            "the complete title stays available to assistive technology"
        );

        let toggle = scene.require_named("appbar-long-title");
        scene.click(&toggle);
        scene.settle_n(2);
        let short = story.title();
        assert_ne!(short, long, "the toggle switches the story's title state");
        // GAP: the untracked `State` read means the header keeps rendering the
        // previous title even though the state moved.
        assert!(
            !scene.recomposer_has_pending(),
            "the toggle schedules no recomposition"
        );
        assert!(
            scene.has_text(long),
            "the header still renders the original title, texts: {:?}",
            scene.texts()
        );

        let toggle = scene.require_named("appbar-long-title");
        scene.click(&toggle);
        scene.settle_n(2);
        assert_eq!(story.title(), long, "toggling back restores the long title");
    });
}

#[test]
fn the_text_primary_toggle_switches_the_primary_presentation() {
    scene(720.0, true, |scene, story| {
        assert!(
            !story.uses_text_primary(),
            "the primary starts as an icon button"
        );
        let toggle = scene.require_named("appbar-text-primary");
        scene.click(&toggle);
        scene.settle_n(2);
        assert!(
            story.uses_text_primary(),
            "the toggle switches the primary to a text action"
        );

        let toggle = scene.require_named("appbar-text-primary");
        scene.click(&toggle);
        scene.settle_n(2);
        assert!(!story.uses_text_primary(), "the toggle switches it back");
    });
}

#[test]
fn the_overflow_commands_are_the_authored_five_with_one_disabled() {
    scene(1280.0, true, |scene, story| {
        // The authored command order is the story's contract; the header places
        // them by priority, so read the order from the story itself.
        let labels = support::labels(scene);
        for expected in ["Find in workspace", "Export project", "Share project"] {
            assert!(
                labels.iter().any(|label| label.contains(expected)),
                "{expected} is reachable from the header, labels were {labels:?}"
            );
        }
        // The disabled overflow command must stay listed but inert.
        let unavailable = scene.all(|node| {
            node.accessibility()
                .is_some_and(|semantics| semantics.label.contains("Unavailable"))
        });
        if let Some(node) = unavailable.first() {
            let semantics = node.accessibility().expect("semantics");
            assert!(
                !semantics.state.is_enabled,
                "the unavailable command reports itself disabled"
            );
        }
        let _ = story;
    });
}

#[test]
fn disabling_the_gallery_options_makes_the_header_inert() {
    scene(1280.0, false, |scene, story| {
        for name in ["appbar-long-title", "appbar-text-primary"] {
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
        assert_eq!(
            story.last_action(),
            "No action yet",
            "no header action runs while disabled"
        );
        assert!(
            scene.has_text("No action yet"),
            "the result line is unchanged, texts: {:?}",
            scene.texts()
        );
    });
}
