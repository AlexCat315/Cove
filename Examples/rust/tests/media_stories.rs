//! Behaviour tests for the feedback/media stories.
//!
//! Translation of `GalleryContent/Stories/Feedback/{SkeletonStory,RatingSample}.swift`.
//! The state machines are what the gallery actually exercises, so they are worth
//! pinning independently of rendering.

use cove_examples::gallery_content::catalog::GalleryOptions;
use cove_examples::gallery_content::stories::{RatingSample, SkeletonStory};

#[test]
fn the_skeleton_story_starts_loading() {
    let story = SkeletonStory::new(GalleryOptions::default());
    assert!(story.is_loading(), "the placeholder is shown first");
}

#[test]
fn toggling_the_skeleton_story_swaps_between_placeholder_and_content() {
    let story = SkeletonStory::new(GalleryOptions::default());
    assert!(story.is_loading());
    story.toggle_loading();
    assert!(!story.is_loading(), "the first toggle reveals content");
    story.toggle_loading();
    assert!(
        story.is_loading(),
        "the second toggle restores the placeholder"
    );
}

#[test]
fn the_rating_sample_starts_at_three_of_five() {
    let sample = RatingSample::new(GalleryOptions::default());
    assert_eq!(sample.value(), 3.0);
}
