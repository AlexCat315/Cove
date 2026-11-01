//! Behaviour tests for the sidebar examples.
//!
//! Translation of
//! `GalleryContent/Stories/Layout/{ResizableSidebarExample,NestedSidebarExample}.swift`.

use cove_examples::gallery_content::catalog::GalleryOptions;
use cove_examples::gallery_content::stories::sidebar_stories::{
    DEFAULT_SIDEBAR_WIDTH, NESTED_DEFAULT_EXPANDED, NESTED_DEFAULT_SELECTION, NestedSidebarExample,
    ResizableSidebarExample,
};

#[test]
fn the_resizable_example_starts_at_the_authored_width() {
    let story = ResizableSidebarExample::new(GalleryOptions::default());
    assert_eq!(story.width(), DEFAULT_SIDEBAR_WIDTH);
    assert_eq!(story.selected(), Some(0));
}

#[test]
fn resetting_restores_the_authored_width() {
    let story = ResizableSidebarExample::new(GalleryOptions::default());
    story.reset_width();
    assert_eq!(story.width(), DEFAULT_SIDEBAR_WIDTH);
    assert_eq!(story.requested_width_label(), "Requested width: 240 pt");
}

#[test]
fn the_resizable_example_lists_two_documents() {
    let story = ResizableSidebarExample::new(GalleryOptions::default());
    let sections = story.sections();
    assert_eq!(sections.len(), 1);
    assert_eq!(sections[0].items.len(), 2);
    assert_eq!(sections[0].items[0].title, "Overview");
}

#[test]
fn the_nested_example_starts_on_the_overview() {
    let story = NestedSidebarExample::new(GalleryOptions::default());
    assert_eq!(story.selection().as_deref(), Some(NESTED_DEFAULT_SELECTION));
    assert_eq!(story.expanded(), vec![NESTED_DEFAULT_EXPANDED.to_owned()]);
    assert_eq!(story.last_action(), "None");
}

#[test]
fn expanding_and_collapsing_groups_preserves_the_selection() {
    let story = NestedSidebarExample::new(GalleryOptions::default());
    story.expand_groups();
    assert_eq!(story.expanded().len(), 2);
    // Collapsing hides the children but must not move the selection, or the
    // page would jump every time a group closes.
    story.collapse_groups();
    assert!(story.expanded().is_empty());
    assert_eq!(
        story.selection().as_deref(),
        Some(NESTED_DEFAULT_SELECTION),
        "collapsing preserves the selected document"
    );
}

#[test]
fn the_drawer_opens_from_the_story() {
    let story = NestedSidebarExample::new(GalleryOptions::default());
    assert!(!story.is_open());
    story.open_drawer();
    assert!(story.is_open());
}

#[test]
fn selectable_ids_are_leaves_and_skip_disabled_items() {
    let story = NestedSidebarExample::new(GalleryOptions::default());
    let ids = story.selectable_ids();
    // Groups are expandable, not selectable; `nested-billing` is disabled.
    assert!(ids.contains(&"nested-welcome".to_owned()));
    assert!(ids.contains(&"nested-audit".to_owned()));
    assert!(
        !ids.contains(&"nested-projects".to_owned()),
        "a group expands instead"
    );
    assert!(
        !ids.contains(&"nested-billing".to_owned()),
        "disabled items are not selectable"
    );
}

#[test]
fn recording_an_action_replaces_the_previous_one() {
    let story = NestedSidebarExample::new(GalleryOptions::default());
    story.record("Archive");
    assert_eq!(story.last_action(), "Archive");
    story.record("Duplicate");
    assert_eq!(story.last_action(), "Duplicate");
}
