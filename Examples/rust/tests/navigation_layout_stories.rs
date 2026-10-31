//! Behaviour tests for the drawer and rail stories.
//!
//! Translation of `GalleryContent/Stories/Layout/{DrawerStory,RailStory}.swift`.
//! Rail width and disabled-item skipping are the measurable contract the gallery
//! documents, so they are pinned here.

use cove_examples::gallery_content::catalog::GalleryOptions;
use cove_examples::gallery_content::stories::navigation_layout_stories::{
    DRAWER_BADGED_INDEX, DRAWER_DEFAULT_SELECTION, DRAWER_DESTINATION_COUNT, DRAWER_DISABLED_INDEX,
    DrawerStory, RAIL_DESTINATIONS, RailStory,
};

fn drawer() -> DrawerStory {
    DrawerStory::new(GalleryOptions::default())
}

#[test]
fn the_drawer_starts_closed_on_the_authored_destination() {
    let story = drawer();
    assert!(!story.is_open());
    assert_eq!(story.selection(), Some(DRAWER_DEFAULT_SELECTION));
    assert_eq!(story.selection_caption(), "Selected destination: 8");
}

#[test]
fn opening_sets_the_presented_state() {
    let story = drawer();
    story.open();
    assert!(story.is_open());
}

#[test]
fn choosing_closes_the_drawer_when_configured() {
    let story = drawer();
    story.open();
    assert!(story.closes_on_selection());
    story.choose(2);
    assert_eq!(story.selection(), Some(2));
    assert!(!story.is_open(), "choosing closes it by default");
}

#[test]
fn choosing_keeps_the_drawer_open_when_that_is_disabled() {
    let story = drawer();
    story.set_closes_on_selection(false);
    story.open();
    story.choose(3);
    assert_eq!(story.selection(), Some(3));
    assert!(story.is_open(), "the drawer stays open to inspect it");
}

#[test]
fn the_drawer_lists_twenty_destinations_and_skips_the_unavailable_one() {
    let story = drawer();
    let sections = story.sections();
    assert_eq!(sections[0].items.len(), DRAWER_DESTINATION_COUNT);
    let selectable = story.selectable();
    assert_eq!(selectable.len(), DRAWER_DESTINATION_COUNT - 1);
    assert!(
        !selectable.contains(&DRAWER_DISABLED_INDEX),
        "the unavailable destination is not selectable"
    );
}

#[test]
fn the_badged_destination_keeps_its_label() {
    let story = drawer();
    let items = &story.sections()[0].items;
    assert_eq!(items[DRAWER_BADGED_INDEX].badge.as_deref(), Some("8"));
}

#[test]
fn an_empty_selection_reads_as_none() {
    // A drawer with nothing chosen still needs a caption, and `none` is it.
    let story = DrawerStory::new_without_selection(GalleryOptions::default());
    assert_eq!(story.selection(), None);
    assert_eq!(story.selection_caption(), "Selected destination: none");
}

#[test]
fn the_rail_keeps_labels_by_default_and_is_seventy_two_points_wide() {
    let story = RailStory::new(GalleryOptions::default());
    assert!(story.shows_labels());
    assert_eq!(story.rail_width(), 72.0);
}

#[test]
fn hiding_labels_narrows_the_rail_to_fifty_six_points() {
    let story = RailStory::new(GalleryOptions::default());
    story.set_shows_labels(false);
    assert_eq!(story.rail_width(), 56.0);
}

#[test]
fn the_rail_skips_the_disabled_destination() {
    let story = RailStory::new(GalleryOptions::default());
    let navigable = story.navigable();
    // Index 3 is "Unavailable".
    assert!(!navigable.contains(&3), "disabled destinations are skipped");
    assert_eq!(navigable.len(), RAIL_DESTINATIONS.len() - 1);
}

#[test]
fn long_badges_keep_their_full_value() {
    let story = RailStory::new(GalleryOptions::default());
    let items = story.items();
    assert_eq!(items[1].badge.as_deref(), Some("12"));
    assert_eq!(
        items[2].badge.as_deref(),
        Some("999"),
        "a four-digit badge is not truncated"
    );
}

#[test]
fn the_rail_starts_on_the_first_destination() {
    let story = RailStory::new(GalleryOptions::default());
    assert_eq!(story.selection(), Some(0));
}
