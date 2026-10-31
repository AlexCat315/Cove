//! Translation of `Examples/Tests/GalleryContentTests/SplitTests.swift`.
//!
//! Behaviour checklist:
//! - the `splitView` page declares an adaptive sample at both preview widths
//! - the story mounts at both text scales without collapsing
//! - the adaptive preview caption is rendered
//! - the two preview width bands resolve the documented sample size
//!
//! ## Named differences
//!
//! `GAP(adaptive_examples.rs AdaptiveControlExamples)`: Swift's suite mounted
//! `AdaptiveControlExamples(page: .splitView, options:)` and then located two
//! real split-view previews by debug name (`adaptive-splitView-narrow` and
//! `adaptive-splitView-wide`). Inside each it asserted:
//!   - a `Details` tab whose `acceptsSubtreeInput` is true only for the narrow
//!     preview;
//!   - a focusable text field that is initially unreachable in the narrow
//!     preview and becomes reachable after activating `Details`;
//!   - typing "Edited " through a real `EventDispatcher` updates the field's
//!     accessibility value, and that value survives switching to `Navigation`
//!     and back, with the field keeping its element id.
//! The Rust `AdaptiveControlExamples::body` renders a single caption
//! (`adaptive-preview-caption`); it does not render either preview, any tab, or
//! any text field. `sample_size` and `adaptive_debug_name` exist as pure
//! functions but nothing in the view tree uses them. Everything that is
//! reachable — the declared sample height, both width bands, the caption and the
//! mount at both scales — is asserted at full strength.

mod support;

use std::rc::Rc;

use cove_composition::AnyView;
use cove_examples::gallery_content::catalog::{GalleryOptions, GalleryPage};
use cove_examples::gallery_content::stories::adaptive_examples::{
    AdaptiveControlExamples, FRAME_PADDING, PREVIEW_WIDTHS, frame_height,
};
use cove_ui_compose::theme::tokens::TextScaler;
use cove_ui_compose::theme::TextScalerViewExt;

use support::GalleryScene;

struct Mounted {
    story: Rc<AdaptiveControlExamples>,
    scale: f32,
}

impl cove_composition::View for Mounted {
    fn body(&self) -> AnyView {
        AnyView::new(
            AnyView::new(StoryView(self.story.clone())).text_scaler(TextScaler::new(self.scale)),
        )
    }
}

struct StoryView(Rc<AdaptiveControlExamples>);

impl cove_composition::View for StoryView {
    fn body(&self) -> AnyView {
        cove_composition::View::body(self.0.as_ref())
    }
}

fn scene(scale: f32, body: impl FnOnce(&GalleryScene, &AdaptiveControlExamples)) {
    let story = Rc::new(AdaptiveControlExamples::new(GalleryPage::SplitView));
    let mounted = story.clone();
    GalleryScene::run(
        900.0,
        1100.0,
        move |_| AnyView::new(Mounted { story: mounted, scale }),
        move |scene| {
            scene.settle_n(8);
            body(scene, &story);
        },
    );
}

#[test]
fn the_split_view_page_declares_an_adaptive_sample_at_both_widths() {
    assert!(
        GalleryPage::SplitView.has_adaptive_sample(),
        "splitView is in the supported set"
    );
    assert_eq!(
        GalleryPage::SplitView.adaptive_sample_height(),
        Some(220.0),
        "the split preview reserves the authored 220pt"
    );
    assert_eq!(
        frame_height(GalleryPage::SplitView),
        Some(220.0 + FRAME_PADDING),
        "the frame height adds the authored label padding"
    );

    for width in PREVIEW_WIDTHS {
        // Both bands resolve a definite size, so the preview is measurable.
        assert!(
            AdaptiveControlExamples::new(GalleryPage::SplitView)
                .sample_size(width)
                .1
                .is_some(),
            "the {width}pt preview has a definite height"
        );
        let name = GalleryPage::SplitView.adaptive_debug_name(width);
        assert!(
            name.starts_with("adaptive-splitView-"),
            "the preview carries its page name, got {name:?}"
        );
    }
    assert_eq!(
        GalleryPage::SplitView.adaptive_debug_name(390.0),
        "adaptive-splitView-narrow"
    );
    assert_eq!(
        GalleryPage::SplitView.adaptive_debug_name(720.0),
        "adaptive-splitView-wide"
    );
}

#[test]
fn the_preview_caption_renders_at_both_text_scales() {
    for scale in [1.0f32, 2.0] {
        scene(scale, |scene, story| {
            assert!(story.supports(), "splitView has something to preview");
            assert_eq!(
                story.captions(),
                ["390 pt · Pointer", "720 pt · Pointer"],
                "the story lists both preview bands"
            );

            let caption = scene
                .named("adaptive-preview-caption")
                .unwrap_or_else(|| panic!("no preview caption at scale {scale}"));
            assert!(
                caption.frame().width > 0.0 && caption.frame().height > 0.0,
                "the caption has a real box at scale {scale}, got {:?}",
                caption.frame()
            );
            assert!(
                scene.has_text("390 pt · Pointer"),
                "the caption renders the narrow band at scale {scale}, texts: {:?}",
                scene.texts()
            );
        });
    }
}

#[test]
fn the_story_reports_the_split_sample_size_for_each_band() {
    let story = AdaptiveControlExamples::new(GalleryPage::SplitView);
    assert_eq!(
        story.sample_size(390.0),
        (390.0, Some(252.0)),
        "the narrow preview is 390pt wide plus the 32pt label band"
    );
    assert_eq!(
        story.sample_size(720.0),
        (720.0, Some(252.0)),
        "the wide preview is 720pt wide plus the 32pt label band"
    );
    let _ = GalleryOptions::default();
}
