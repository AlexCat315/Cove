//! Behaviour tests for the overlay stories.
//!
//! Translation of `GalleryContent/Stories/Overlays/{PopconfirmStory,HoverStory}.swift`.
//! The confirm/follow state machines are what the gallery exercises, so they are
//! worth pinning independently of rendering.

use cove_examples::gallery_content::catalog::GalleryOptions;
use cove_examples::gallery_content::stories::{HoverCardStory, PopconfirmStory};

#[test]
fn the_popconfirm_story_starts_with_no_action() {
    let story = PopconfirmStory::new(GalleryOptions::default());
    assert_eq!(story.last_action(), "None");
}

#[test]
fn confirming_records_the_action() {
    let story = PopconfirmStory::new(GalleryOptions::default());
    story.record("Deleted");
    assert_eq!(story.last_action(), "Deleted");
    // A second confirmation replaces the record rather than appending.
    story.record("Archived");
    assert_eq!(story.last_action(), "Archived");
}

#[test]
fn the_hover_story_starts_not_following() {
    let story = HoverCardStory::new(GalleryOptions::default());
    assert!(!story.is_following());
}

#[test]
fn following_toggles() {
    let story = HoverCardStory::new(GalleryOptions::default());
    story.toggle_following();
    assert!(story.is_following());
    story.toggle_following();
    assert!(!story.is_following(), "toggling back unfollows");
}
