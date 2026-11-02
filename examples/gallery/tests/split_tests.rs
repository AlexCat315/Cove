//! Behaviour tests for the split-view story.
//!
//! Translation of `Examples/Tests/GalleryContentTests/SplitTests.swift`, minus
//! the fixed-width adaptive previews Swift wrapped around the story. Swift
//! mounted `AdaptiveControlExamples(page: .splitView, options:)` to place two
//! copies of the story inside 390pt and 720pt containers and then located them
//! by debug name (`adaptive-splitView-narrow` / `-wide`). That preview shell is
//! not ported, so the story is mounted directly and the same contracts are
//! asserted where they live: the three authored splits, their authored
//! fractions, and the divider bounds.
//!
//! ## Named differences
//!
//! - Swift's per-preview tabs and text field (`Details` / `Navigation`) belong to
//!   a split-view *demo* that never shipped in the Rust story; the Rust page
//!   shows three splits instead, so those focus assertions have no target.
//! - The adaptive sample tables (`supports`, per-page sample heights, preview
//!   widths) were deleted with `adaptive_examples.rs`; the previews are not part
//!   of the port.

use cove_composition::Scope;
mod support;

use std::rc::Rc;

use cove_composition::AnyView;
use cove_examples::gallery_content::catalog::GalleryOptions;
use cove_examples::gallery_content::stories::SplitViewStory;
use cove_ui_compose::theme::TextScalerViewExt;
use cove_ui_compose::theme::tokens::TextScaler;

use support::GalleryScene;

struct Mounted {
    story: Rc<SplitViewStory>,
    scale: f32,
}

impl cove_composition::View for Mounted {
    fn body(&self, _cx: &Scope) -> AnyView {
        AnyView::new(
            AnyView::new(StoryView(self.story.clone())).text_scaler(TextScaler::new(self.scale)),
        )
    }
}

struct StoryView(Rc<SplitViewStory>);

impl cove_composition::View for StoryView {
    fn body(&self, cx: &Scope) -> AnyView {
        cove_composition::View::body(self.0.as_ref(), cx)
    }
}

fn scene(scale: f32, body: impl FnOnce(&GalleryScene, &SplitViewStory)) {
    let story = Rc::new(SplitViewStory::new(GalleryOptions::default()));
    let mounted = story.clone();
    GalleryScene::run(
        900.0,
        1100.0,
        move |_| {
            AnyView::new(Mounted {
                story: mounted,
                scale,
            })
        },
        move |scene| {
            scene.settle_n(8);
            body(scene, &story);
        },
    );
}

#[test]
fn the_story_mounts_every_authored_split() {
    for scale in [1.0f32, 2.0] {
        scene(scale, |scene, _story| {
            for name in ["split-control", "split-proportional", "split-vertical"] {
                let node = scene
                    .named(name)
                    .unwrap_or_else(|| panic!("no {name} at scale {scale}"));
                let frame = node.frame();
                assert!(
                    frame.width > 0.0 && frame.height > 0.0,
                    "{name} has a real box at scale {scale}, got {frame:?}"
                );
            }
        });
    }
}

#[test]
fn the_authored_frames_are_preserved() {
    scene(1.0, |scene, _story| {
        let resizable = scene.require_named("split-control").frame();
        assert_eq!(resizable.width, 420.0, "the resizable split keeps 420pt");
        assert_eq!(resizable.height, 160.0, "the resizable split keeps 160pt");
        let vertical = scene.require_named("split-vertical").frame();
        assert_eq!(vertical.width, 320.0, "the vertical split keeps 320pt");
        assert_eq!(vertical.height, 180.0, "the vertical split keeps 180pt");
    });
}

#[test]
fn the_resizable_fraction_stays_inside_its_bounds() {
    let story = SplitViewStory::new(GalleryOptions::default());
    assert_eq!(story.fraction(), 0.5, "the divider starts centred");
    assert_eq!(story.readout(), "50 / 50");

    story.set_fraction(0.95);
    assert_eq!(story.fraction(), SplitViewStory::RANGE.1, "clamped at the top");
    assert_eq!(story.readout(), "80 / 20");

    story.set_fraction(0.05);
    assert_eq!(story.fraction(), SplitViewStory::RANGE.0, "clamped at the bottom");
    assert_eq!(story.readout(), "20 / 80");
}
