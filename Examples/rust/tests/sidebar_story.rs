//! Behaviour tests for the workspace sidebar story.
//!
//! Translation of `GalleryContent/Stories/Layout/SidebarStory.swift`. The detail
//! pane must never claim a row is selected when it is not, and a disabled
//! destination must refuse activation.

use cove_examples::gallery_content::catalog::GalleryOptions;
use cove_examples::gallery_content::stories::sidebar_story::{
    ACCESSORY_WIDTH, DEFAULT_SELECTION, DISABLED_DESTINATION, DestinationIcon, SECTION_IDS,
    SidebarStory, destination_detail,
};

fn story() -> SidebarStory {
    SidebarStory::new(GalleryOptions::default())
}

#[test]
fn the_story_starts_on_downloads() {
    let story = story();
    assert_eq!(story.selection().as_deref(), Some(DEFAULT_SELECTION));
    assert_eq!(story.detail().title, "Downloads");
    assert_eq!(story.detail().icon, DestinationIcon::Download);
}

#[test]
fn the_sidebar_lists_two_titled_sections() {
    let story = story();
    let sections = story.sections();
    assert_eq!(sections.len(), SECTION_IDS.len());
    assert_eq!(sections[0].id, "workspace");
    assert_eq!(sections[0].title, "Workspace");
    assert_eq!(sections[1].id, "preferences");
    assert_eq!(sections[1].title, "Preferences");
}

#[test]
fn every_destination_is_listed_but_one_is_not_navigable() {
    let story = story();
    let all = story.destination_ids();
    assert_eq!(all.len(), 7, "four workspace and three preference rows");
    assert!(
        all.contains(&DISABLED_DESTINATION.to_owned()),
        "it stays listed"
    );
    assert!(
        !story.navigable().contains(&DISABLED_DESTINATION.to_owned()),
        "but the keyboard skips it"
    );
}

#[test]
fn selecting_a_destination_updates_the_detail_pane() {
    let story = story();
    assert!(story.select("overview"));
    assert_eq!(story.detail().title, "Overview");
    assert_eq!(story.detail().icon, DestinationIcon::Dashboard);
}

#[test]
fn a_disabled_destination_refuses_activation() {
    let story = story();
    let before = story.selection();
    assert!(!story.select(DISABLED_DESTINATION));
    assert_eq!(story.selection(), before, "the selection is unchanged");
}

#[test]
fn an_unknown_destination_is_refused() {
    let story = story();
    assert!(!story.select("nowhere"));
    assert_eq!(story.selection().as_deref(), Some(DEFAULT_SELECTION));
}

#[test]
fn an_absent_selection_prompts_instead_of_claiming_a_row() {
    let detail = destination_detail(None);
    assert_eq!(detail.title, "Choose a destination");
}

#[test]
fn every_known_destination_has_its_own_icon() {
    let expected = [
        ("overview", DestinationIcon::Dashboard),
        ("downloads", DestinationIcon::Download),
        ("projects", DestinationIcon::Folder),
        ("recent", DestinationIcon::Activity),
        ("notifications", DestinationIcon::Bell),
        ("settings", DestinationIcon::Settings),
    ];
    for (id, icon) in expected {
        assert_eq!(destination_detail(Some(id)).icon, icon, "{id}");
    }
    // The slugs are distinct, so no two destinations share a glyph.
    let slugs: Vec<&str> = expected.iter().map(|(_, icon)| icon.slug()).collect();
    let unique: std::collections::HashSet<&&str> = slugs.iter().collect();
    assert_eq!(unique.len(), slugs.len());
}

#[test]
fn toggling_icons_and_badges_controls_the_rows() {
    let story = story();
    assert!(story.shows_icons());
    assert!(story.shows_badges());
    // Downloads carries the "8" badge.
    let badges: Vec<Option<String>> = story.sections()[0]
        .items
        .iter()
        .map(|item| item.badge.clone())
        .collect();
    assert!(badges.iter().flatten().any(|badge| badge == "8"));

    story.set_shows_badges(false);
    let hidden: Vec<Option<String>> = story.sections()[0]
        .items
        .iter()
        .map(|item| item.badge.clone())
        .collect();
    assert!(hidden.iter().all(Option::is_none), "badges are off");
}

#[test]
fn the_slotted_row_uses_the_declared_accessory_lane() {
    let story = story();
    let recent = story.sections()[0]
        .items
        .iter()
        .find(|item| item.id == "recent")
        .expect("the recent row exists")
        .clone();
    assert_eq!(recent.slots.accessory_width, ACCESSORY_WIDTH);
    assert!(recent.slots.trailing.is_some(), "the muted count is set");
}

#[test]
fn pinning_toggles() {
    let story = story();
    assert!(!story.is_pinned());
    story.toggle_pinned();
    assert!(story.is_pinned());
    story.toggle_pinned();
    assert!(!story.is_pinned());
}

#[test]
fn the_drawer_starts_closed_and_opens() {
    let story = story();
    assert!(!story.is_drawer_open());
    story.open_drawer();
    assert!(story.is_drawer_open());
}
