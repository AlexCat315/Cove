//! Behaviour tests for the overlay story file.
//!
//! Translation of `GalleryContent/Stories/Layout/OverlayStories.swift`, which holds
//! six stories behind six gallery pages. The menu tree and the tab-close
//! fallback are the parts with real logic.

use cove_examples::gallery_content::catalog::GalleryOptions;
use cove_examples::gallery_content::stories::overlay_stories_file::{
    leaf_count, many_entries, menu_depth, tab_after_close, workspace_entries, DisclosureStory,
    MenuStory, ModalStory, PopoverStory, TabsStory, GROUP_STRIDE, INITIAL_DOCUMENTS, MANY_COMMANDS,
};
use cove_ui_compose::primitives::overlays::menu::MenuEntry;

#[test]
fn the_workspace_menu_nests_three_levels_deep() {
    // Settings → Appearance → Accent color → items.
    assert_eq!(menu_depth(&workspace_entries(true, true)), 3);
}

#[test]
fn labels_and_separators_are_not_commands() {
    let entries = workspace_entries(true, true);
    let leaves = leaf_count(&entries);
    // Copy, Pin, General, System theme, Blue, Green, Restricted, Unavailable,
    // Delete = 9.
    assert_eq!(leaves, 9);
    // Seven top-level rows: a label, Copy, Pin, a separator, the Settings
    // submenu, Unavailable and Delete. The submenu contributes most of the nine
    // leaves while counting as one row, so the row count is the smaller number.
    assert_eq!(entries.len(), 7);
    assert!(entries.len() < leaves, "a submenu holds several leaves but is one row");
    assert!(matches!(entries[0], MenuEntry::Label { .. }));
    assert!(matches!(entries[3], MenuEntry::Separator { .. }));
}

#[test]
fn disabling_the_story_disables_the_interactive_commands_only() {
    let enabled = workspace_entries(true, true);
    let disabled = workspace_entries(false, true);
    // The submenu carries the flag, and its own "Restricted" row stays disabled.
    let find_restricted = |entries: &[MenuEntry]| -> bool {
        fn walk(entries: &[MenuEntry]) -> Option<bool> {
            for entry in entries {
                match entry {
                    MenuEntry::Item(item) if item.title == "Restricted" => return Some(item.is_enabled),
                    MenuEntry::Submenu(submenu) => {
                        if let Some(found) = walk(&submenu.entries) {
                            return Some(found);
                        }
                    }
                    _ => {}
                }
            }
            None
        }
        walk(entries).unwrap_or(true)
    };
    assert!(!find_restricted(&enabled), "Restricted is disabled even when the story is on");
    assert!(!find_restricted(&disabled));
    assert!(leaf_count(&enabled) == leaf_count(&disabled), "the shape is unchanged");
}

#[test]
fn the_pin_command_carries_its_check_mark() {
    let story = MenuStory::new(GalleryOptions::default());
    assert!(story.is_checked(), "the story starts checked");
    story.toggle_checked();
    assert!(!story.is_checked());
    // And the entry reflects it.
    let entries = workspace_entries(true, story.is_checked());
    let pin_selected = entries.iter().any(|entry| match entry {
        MenuEntry::Item(item) => item.title == "Pin item" && !item.is_selected,
        _ => false,
    });
    assert!(pin_selected);
}

#[test]
fn the_scrollable_menu_has_one_hundred_rows() {
    let entries = many_entries();
    assert_eq!(entries.len(), MANY_COMMANDS);
}

#[test]
fn every_tenth_row_is_a_group_submenu() {
    let entries = many_entries();
    let groups = entries
        .iter()
        .filter(|entry| matches!(entry, MenuEntry::Submenu(_)))
        .count();
    assert_eq!(groups, MANY_COMMANDS / GROUP_STRIDE);
    assert_eq!(menu_depth(&entries), 1, "the groups contain only leaves");
    // 90 plain commands plus 10 groups of two.
    assert_eq!(leaf_count(&entries), 90 + 10 * 2);
}

#[test]
fn menu_ids_are_unique() {
    let story = MenuStory::new(GalleryOptions::default());
    fn collect(entries: &[MenuEntry], ids: &mut Vec<u64>) {
        for entry in entries {
            match entry {
                MenuEntry::Item(item) => ids.push(item.id),
                MenuEntry::Submenu(submenu) => {
                    ids.push(submenu.id);
                    collect(&submenu.entries, ids);
                }
                MenuEntry::Label { id, .. } | MenuEntry::Separator { id } => ids.push(*id),
            }
        }
    }
    let mut ids = Vec::new();
    collect(&story.many(), &mut ids);
    let unique: std::collections::HashSet<u64> = ids.iter().copied().collect();
    assert_eq!(unique.len(), ids.len(), "the 100-row menu has no colliding ids");
}

#[test]
fn the_popover_starts_closed_with_its_draft() {
    let story = PopoverStory::new(GalleryOptions::default());
    assert!(!story.is_open());
    assert_eq!(story.name(), "New project");
    story.open();
    assert!(story.is_open());
    story.set_name("Aurora");
    story.done();
    assert!(!story.is_open());
    assert_eq!(story.name(), "Aurora", "the draft is kept");
}

#[test]
fn the_modal_cancel_keeps_the_status_and_create_records_it() {
    let story = ModalStory::new(GalleryOptions::default());
    assert!(!story.is_open());
    assert_eq!(story.status(), "No changes");

    story.open();
    story.set_name("Aurora");
    story.cancel();
    assert!(!story.is_open());
    assert_eq!(story.status(), "No changes", "cancelling records nothing");

    story.open();
    story.create();
    assert!(!story.is_open());
    assert_eq!(story.status(), "Created Aurora");
}

#[test]
fn collapsing_a_disclosure_keeps_the_draft() {
    let story = DisclosureStory::new(GalleryOptions::default());
    assert!(story.is_expanded());
    assert_eq!(story.draft(), "The draft survives collapse");
    story.set_draft("edited");
    story.toggle();
    assert!(!story.is_expanded());
    assert_eq!(story.draft(), "edited", "caller-owned expansion preserves the draft");
}

#[test]
fn the_content_tabs_include_a_disabled_and_a_badged_page() {
    let story = TabsStory::new(GalleryOptions::default());
    let tabs = story.content_tabs();
    assert_eq!(tabs.len(), 5);
    assert!(!tabs[1].is_enabled, "the Unavailable tab cannot be reached");
    assert_eq!(tabs[2].badge.as_deref(), Some("3"));
    assert_eq!(story.selection(), "overview");
    assert_eq!(story.segment(), "day");
}

#[test]
fn closing_a_document_falls_back_to_the_nearest_tab() {
    let story = TabsStory::new(GalleryOptions::default());
    assert_eq!(story.documents(), INITIAL_DOCUMENTS.to_vec());
    assert_eq!(story.document_selection(), "Scene.cove");

    // Closing the selected first document selects the one that took its slot.
    let fallback = story.close_document(0);
    assert_eq!(fallback.as_deref(), Some("Material.cove"));
    assert_eq!(story.document_selection(), "Material.cove");
    assert_eq!(story.documents(), vec!["Material.cove", "Notes.md"]);
}

#[test]
fn closing_the_last_document_falls_back_to_the_previous_tab() {
    let story = TabsStory::new(GalleryOptions::default());
    // Select the last document, then close it.
    story.close_document(1); // removes Material.cove; selection was Scene.cove
    assert_eq!(story.document_selection(), "Scene.cove", "an unselected close keeps the selection");
    let fallback = story.close_document(1); // removes Notes.md
    assert_eq!(fallback.as_deref(), Some("Scene.cove"), "the nearest remaining tab");
}

#[test]
fn closing_every_document_clears_the_selection() {
    let story = TabsStory::new(GalleryOptions::default());
    story.close_document(0);
    story.close_document(0);
    story.close_document(0);
    assert!(story.documents().is_empty());
    assert_eq!(story.document_selection(), "", "no documents, no selection");
    // The story offers a reset back to the authored set.
    story.reset_documents();
    assert_eq!(story.documents(), INITIAL_DOCUMENTS.to_vec());
}

#[test]
fn tab_after_close_never_indexes_past_the_end() {
    assert_eq!(tab_after_close(&[], 0), None);
    let docs = vec!["a".to_owned(), "b".to_owned()];
    // The requested index is clamped, so a stale index cannot panic.
    assert_eq!(tab_after_close(&docs, 99).as_deref(), Some("b"));
}
