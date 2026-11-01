//! Behaviour tests for the navigation stories.
//!
//! Translation of `GalleryContent/Stories/Navigation/AppBarStory.swift`.

use cove_examples::gallery_content::catalog::GalleryOptions;
use cove_examples::gallery_content::stories::AppBarStory as Reexported;
use cove_examples::gallery_content::stories::navigation_stories::{
    AppBarStory, LONG_TITLE, SHORT_TITLE, UNAVAILABLE_TITLE, back_title, header_height,
    inline_capacity,
};

#[test]
fn the_story_starts_with_the_long_title() {
    let story = AppBarStory::new(GalleryOptions::default());
    assert_eq!(story.title(), LONG_TITLE);
    assert!(
        story.title().contains("👩🏽‍💻"),
        "the long title carries an emoji run"
    );
}

#[test]
fn the_story_is_reexported_from_the_stories_module() {
    // The dispatch reaches it through `stories::AppBarStory`, so the re-export
    // has to name the same type.
    let _: Reexported = Reexported::new(GalleryOptions::default());
}

#[test]
fn the_story_starts_with_no_action() {
    let story = AppBarStory::new(GalleryOptions::default());
    assert_eq!(story.last_action(), "No action yet");
}

#[test]
fn recording_an_action_replaces_the_previous_one() {
    let story = AppBarStory::new(GalleryOptions::default());
    story.record("Saved");
    assert_eq!(story.last_action(), "Saved");
    story.record("Exported");
    assert_eq!(story.last_action(), "Exported");
}

#[test]
fn the_short_title_is_the_compact_form() {
    assert_ne!(LONG_TITLE, SHORT_TITLE);
    assert_eq!(SHORT_TITLE, "Workspace");
}

#[test]
fn a_compact_header_shows_one_inline_action_and_a_wide_one_shows_three() {
    assert_eq!(inline_capacity(true), 1);
    assert_eq!(inline_capacity(false), 3);
}

#[test]
fn the_header_is_eight_points_taller_than_its_buttons() {
    assert_eq!(header_height(32.0), 40.0);
}

#[test]
fn the_back_command_names_its_destination() {
    assert_eq!(back_title(), "Back to projects");
}

#[test]
fn the_disabled_command_is_still_named() {
    // A disabled overflow command stays listed so the menu can explain itself.
    assert_eq!(UNAVAILABLE_TITLE, "Unavailable");
}
