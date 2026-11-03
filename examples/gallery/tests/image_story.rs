//! Behaviour tests for the image story.
//!
//! Translation of `GalleryContent/Stories/Layout/ImageStory.swift`. The four
//! sources and three loading phases are a state machine; the failure and empty
//! states deliberately share a layout but not their copy.

use cove_examples::gallery_content::catalog::GalleryOptions;
use cove_examples::gallery_content::stories::image_story::{
    ImageSourceKind, ImageStory, ImageSurface, MISSING_PATH, STACKING_BREAKPOINT, surface_caption,
    surface_detail, surface_for,
};
use cove_ui_compose::primitives::media::{AsyncImagePhase, ImageContentMode};

fn story() -> ImageStory {
    ImageStory::new(GalleryOptions::default())
}

#[test]
fn the_story_starts_with_the_bundled_source() {
    let story = story();
    assert_eq!(story.source(), ImageSourceKind::Bundled);
    assert_eq!(story.request_number(), 1);
}

#[test]
fn each_source_resolves_to_its_own_url() {
    let story = story();
    let bundled = story.resolved_url().expect("the bundled source has a URL");
    assert!(
        bundled.starts_with("file://") && bundled.ends_with("/resources/gallery-landscape.png"),
        "the bundled source is a file URL the loader accepts, got {bundled}"
    );
    assert!(
        std::path::Path::new(bundled.trim_start_matches("file://")).exists(),
        "the bundled sample picture ships with the gallery: {bundled}"
    );

    story.set_source(ImageSourceKind::Missing);
    assert_eq!(story.resolved_url().as_deref(), Some(MISSING_PATH));

    story.set_source(ImageSourceKind::Empty);
    assert_eq!(story.resolved_url(), None, "the empty source has no URL");
}

#[test]
fn the_remote_source_uses_the_edited_url() {
    let story = story();
    story.set_source(ImageSourceKind::Remote);
    story.set_remote_url("  https://example.com/a.png  ");
    assert_eq!(
        story.resolved_url().as_deref(),
        Some("https://example.com/a.png"),
        "the URL is trimmed"
    );
}

#[test]
fn retry_advances_the_request_number() {
    let story = story();
    assert!(story.retry());
    assert_eq!(story.retry_count(), 1);
    assert_eq!(story.request_number(), 2);
    assert!(story.retry());
    assert_eq!(story.request_number(), 3);
}

#[test]
fn the_empty_source_cannot_retry() {
    let story = story();
    story.set_source(ImageSourceKind::Empty);
    assert!(!story.source().can_retry());
    assert!(!story.retry(), "there is nothing to re-request");
    assert_eq!(story.request_number(), 1, "the count is unchanged");
}

#[test]
fn every_non_empty_source_can_retry() {
    for kind in [
        ImageSourceKind::Bundled,
        ImageSourceKind::Remote,
        ImageSourceKind::Missing,
    ] {
        assert!(kind.can_retry(), "{kind:?} should retry");
    }
}

#[test]
fn an_unknown_source_id_falls_back_to_bundled() {
    assert_eq!(
        ImageSourceKind::from_id("nonsense"),
        ImageSourceKind::Bundled
    );
    assert_eq!(ImageSourceKind::from_id("empty"), ImageSourceKind::Empty);
}

#[test]
fn the_loading_phase_shows_a_spinner_caption() {
    let surface = surface_for(&AsyncImagePhase::Loading, || unreachable!());
    assert_eq!(surface, ImageSurface::Loading);
    assert_eq!(surface_caption(&surface), "Loading…");
    assert_eq!(surface_detail(&surface), "");
}

#[test]
fn the_empty_phase_prompts_for_a_source() {
    let surface = surface_for(&AsyncImagePhase::Empty, || unreachable!());
    assert_eq!(surface, ImageSurface::Empty);
    assert_eq!(surface_caption(&surface), "No image");
    assert_eq!(surface_detail(&surface), "Choose an image source");
}

#[test]
fn the_empty_and_failure_states_share_a_caption_but_not_their_detail() {
    let empty = surface_for(&AsyncImagePhase::Empty, || unreachable!());
    let failure = surface_for(
        &AsyncImagePhase::Failure(
            cove_ui_compose::primitives::media::AsyncImageError::FileNotFound,
        ),
        || "Image file not found".to_owned(),
    );
    assert_eq!(surface_caption(&empty), "No image");
    assert_eq!(surface_caption(&failure), "Image unavailable");
    assert_eq!(
        surface_detail(&failure),
        "Image file not found",
        "the error description is the detail"
    );
    assert_ne!(surface_detail(&empty), surface_detail(&failure));
}

#[test]
fn the_story_offers_three_content_modes() {
    let story = story();
    assert_eq!(
        story.content_modes(),
        [
            ImageContentMode::Fit,
            ImageContentMode::Fill,
            ImageContentMode::Stretch
        ]
    );
}

#[test]
fn the_preview_stacks_below_the_breakpoint() {
    let story = story();
    assert!(story.stacks_vertically(STACKING_BREAKPOINT - 1.0));
    assert!(!story.stacks_vertically(STACKING_BREAKPOINT));
}
