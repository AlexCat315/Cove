//! Behaviour tests for the overlay stories.
//!
//! Translation of `GalleryContent/Stories/Overlays/{PopconfirmStory,HoverStory}.swift`.
//! The confirm/follow state machines are what the gallery exercises, so they are
//! worth pinning independently of rendering.

use cove_examples::gallery_content::catalog::GalleryOptions;
use cove_examples::gallery_content::stories::{ContextMenuStory, HoverCardStory, PopconfirmStory};
use cove_ui_compose::primitives::overlays::menu::{MenuEntry, MenuItem, MenuSubmenu};

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

#[test]
fn the_hover_story_tracks_the_keyboard_profile_card() {
    let story = HoverCardStory::new(GalleryOptions::default());
    assert!(!story.is_profile_open());
    story.open_profile();
    assert!(story.is_profile_open(), "Open profile presents the anchored card");
    story.close_profile();
    assert!(!story.is_profile_open());
}

fn item_named<'a>(entries: &'a [MenuEntry], title: &str) -> Option<&'a MenuItem> {
    entries.iter().find_map(|entry| match entry {
        MenuEntry::Item(item) if item.title == title => Some(item),
        _ => None,
    })
}

fn submenu_named<'a>(entries: &'a [MenuEntry], title: &str) -> Option<&'a MenuSubmenu> {
    entries.iter().find_map(|entry| match entry {
        MenuEntry::Submenu(submenu) if submenu.title == title => Some(submenu),
        _ => None,
    })
}

#[test]
fn the_context_menu_keeps_rich_rows_and_nested_export() {
    let story = ContextMenuStory::new(GalleryOptions::default());
    let entries = story.entries();

    let open = item_named(&entries, "Open").expect("Open row");
    assert!(open.content.icon.is_some(), "Open keeps its folder glyph");

    let duplicate = item_named(&entries, "Duplicate").expect("Duplicate row");
    assert!(duplicate.content.icon.is_some(), "Duplicate keeps its copy glyph");
    assert_eq!(
        duplicate.content.detail.as_deref(),
        Some("Creates a copy next to the original"),
        "the row keeps its secondary line"
    );

    let documentation = item_named(&entries, "Documentation").expect("Documentation row");
    assert_eq!(
        documentation.content.rich_title.len(),
        2,
        "the link row draws a styled title instead of plain text"
    );
    assert_eq!(
        documentation.content.link.as_deref(),
        Some("https://cove.example/docs")
    );

    let organize = submenu_named(&entries, "Organize").expect("Organize submenu");
    let export = submenu_named(&organize.entries, "Export").expect("nested Export submenu");
    let labels: Vec<&str> = export
        .entries
        .iter()
        .filter_map(|entry| match entry {
            MenuEntry::Item(item) => Some(item.title.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(labels, ["JSON", "Swift"], "Export nests one level deeper");

    let delete = item_named(&entries, "Delete").expect("Delete row");
    assert!(
        !delete.is_enabled,
        "an unavailable command stays visible but skipped"
    );
}
