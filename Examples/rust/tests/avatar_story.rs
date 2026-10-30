//! Behaviour tests for the avatar story.
//!
//! Translation of `GalleryContent/Stories/Feedback/AvatarStory.swift`. The group
//! geometry decides what is measured, painted and hit-tested, so an off-by-one
//! in the overflow count misplaces the whole strip.

use cove_examples::gallery_content::catalog::GalleryOptions;
use cove_examples::gallery_content::stories::avatar_story::{
    AvatarImageSource, AvatarStory, INITIAL_CASES, MEMBER_NAMES,
};
use cove_ui_compose::primitives::data::avatar::{AvatarOverflow, AvatarStatus};

#[test]
fn the_story_starts_on_the_bundled_source() {
    let story = AvatarStory::new(GalleryOptions::default());
    assert_eq!(story.source(), AvatarImageSource::Bundled);
    assert_eq!(story.source().url(), Some("avatar-portrait.svg"));
}

#[test]
fn changing_the_source_swaps_the_url() {
    let story = AvatarStory::new(GalleryOptions::default());
    story.set_source(AvatarImageSource::Remote);
    assert!(story.source().url().unwrap().starts_with("https://"));
    story.set_source(AvatarImageSource::Initials);
    assert_eq!(story.source().url(), None, "initials need no image");
    assert!(story.source().shows_initials_only());
}

#[test]
fn an_unknown_source_id_falls_back_to_bundled() {
    assert_eq!(AvatarImageSource::from_id("nonsense"), AvatarImageSource::Bundled);
    assert_eq!(AvatarImageSource::from_id("missing"), AvatarImageSource::Missing);
}

#[test]
fn the_group_starts_at_three_of_six() {
    let story = AvatarStory::new(GalleryOptions::default());
    let geometry = story.group_geometry(32.0);
    assert_eq!(geometry.visible_count, 3);
    assert!(geometry.has_overflow, "the remaining three need a marker");
    assert_eq!(geometry.item_count(), 4, "three members plus the overflow marker");
}

#[test]
fn a_limit_covering_every_member_shows_no_overflow() {
    let story = AvatarStory::new(GalleryOptions::default());
    story.set_limit(6);
    let geometry = story.group_geometry(32.0);
    assert_eq!(geometry.visible_count, MEMBER_NAMES.len());
    assert!(!geometry.has_overflow);
}

#[test]
fn a_zero_limit_shows_only_the_overflow_indicator() {
    let story = AvatarStory::new(GalleryOptions::default());
    story.set_limit(0);
    let geometry = story.group_geometry(32.0);
    assert_eq!(geometry.visible_count, 0);
    assert!(geometry.has_overflow);
    assert_eq!(geometry.item_count(), 1, "only the indicator");
}

#[test]
fn hidden_overflow_suppresses_the_marker_entirely() {
    let story = AvatarStory::new(GalleryOptions::default());
    story.set_overflow(AvatarOverflow::Hidden);
    let geometry = story.group_geometry(32.0);
    assert!(!geometry.has_overflow);
    assert_eq!(geometry.item_count(), 3, "just the visible members");
}

#[test]
fn overlap_reduces_the_step_between_members() {
    let story = AvatarStory::new(GalleryOptions::default());
    // 0.3 overlap at a 32pt diameter means each member advances 22.4pt.
    let geometry = story.group_geometry(32.0);
    assert!((geometry.step - 22.4).abs() < 0.001, "step was {}", geometry.step);
}

#[test]
fn every_status_is_rendered_in_declared_order() {
    let story = AvatarStory::new(GalleryOptions::default());
    let statuses = story.statuses();
    assert_eq!(statuses, AvatarStatus::ALL);
    assert_eq!(statuses.len(), 4);
}

#[test]
fn the_initial_cases_include_unicode_and_an_empty_name() {
    assert!(INITIAL_CASES.contains(&"李 小龙"));
    assert!(INITIAL_CASES.contains(&"Émile Zola"));
    assert!(INITIAL_CASES.contains(&""), "the empty name exercises the Anonymous fallback");
}

#[test]
fn the_story_lists_six_members() {
    let story = AvatarStory::new(GalleryOptions::default());
    assert_eq!(story.members().len(), 6);
    assert_eq!(story.selected_member(), "None");
}
