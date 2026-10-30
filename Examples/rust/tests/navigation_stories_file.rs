//! Behaviour tests for the navigation story file.
//!
//! Translation of `GalleryContent/Stories/Layout/NavigationStories.swift`, which
//! holds twelve stories behind twelve gallery pages. Accordion single-expansion
//! and pagination window arithmetic are the parts with real edge cases.

use cove_examples::gallery_content::catalog::GalleryOptions;
use cove_examples::gallery_content::stories::navigation_stories_file::{
    AccordionStory, AlertDialogStory, BottomSheetStory, BreadcrumbStory, ButtonGroupStory,
    DialogStory, LabelStory, ModalEdge, NavigationStackStory, PaginationStory, SheetStory, TagStory,
    BOTTOM_SHEET_DETENTS, DEFAULT_PAGE, DEFAULT_SECTION, DRAG_DISMISS_THRESHOLD, INITIAL_TAGS,
    TOTAL_PAGES,
};
use cove_ui_compose::primitives::layout::navigation_components::PaginationItem;

fn accordion() -> AccordionStory {
    AccordionStory::new(GalleryOptions::default())
}

#[test]
fn the_accordion_starts_with_only_the_project_section_open() {
    let story = accordion();
    assert_eq!(story.expanded().len(), 1);
    assert!(story.expanded().contains(DEFAULT_SECTION));
    assert_eq!(story.caption(), "1 sections open · Autosave on");
}

#[test]
fn single_mode_replaces_the_open_section() {
    let story = accordion();
    story.toggle("diagnostics");
    assert_eq!(story.expanded().len(), 1, "single mode closes the previous section");
    assert!(story.expanded().contains("diagnostics"));
    assert!(!story.expanded().contains(DEFAULT_SECTION));
}

#[test]
fn multiple_mode_accumulates_sections() {
    let story = accordion();
    story.set_allows_multiple(true);
    story.toggle("diagnostics");
    assert_eq!(story.expanded().len(), 2);
    assert!(story.expanded().contains(DEFAULT_SECTION));
    assert!(story.expanded().contains("diagnostics"));
}

#[test]
fn turning_multiple_mode_off_collapses_back_to_project() {
    let story = accordion();
    story.set_allows_multiple(true);
    story.toggle("diagnostics");
    assert_eq!(story.expanded().len(), 2);
    story.set_allows_multiple(false);
    assert_eq!(
        story.expanded().len(),
        1,
        "leaving several open would contradict the chosen mode"
    );
    assert!(story.expanded().contains(DEFAULT_SECTION));
}

#[test]
fn a_disabled_section_cannot_be_toggled() {
    let story = accordion();
    story.set_allows_multiple(true);
    story.toggle("unavailable");
    assert!(!story.expanded().contains("unavailable"));
    assert_eq!(story.expanded().len(), 1);
}

#[test]
fn toggling_an_open_section_closes_it() {
    let story = accordion();
    story.toggle(DEFAULT_SECTION);
    assert!(story.expanded().is_empty());
    assert_eq!(story.caption(), "0 sections open · Autosave on");
}

#[test]
fn collapse_all_reopens_project_when_nothing_is_open() {
    let story = accordion();
    story.toggle_all();
    assert!(story.expanded().is_empty(), "first press collapses");
    story.toggle_all();
    assert!(story.expanded().contains(DEFAULT_SECTION), "second press reopens");
}

#[test]
fn the_pagination_window_shows_a_gap_when_pages_were_skipped() {
    let story = PaginationStory::new(GalleryOptions::default());
    assert_eq!(story.page(), DEFAULT_PAGE);
    let items = story.items();
    // First and last pages always appear; the middle is a window around page 8.
    assert_eq!(items.first(), Some(&PaginationItem::Page(1)));
    assert_eq!(items.last(), Some(&PaginationItem::Page(TOTAL_PAGES)));
    assert!(items.contains(&PaginationItem::Page(DEFAULT_PAGE)));
    assert!(items.contains(&PaginationItem::LeadingGap), "pages 2..6 are skipped");
    assert!(items.contains(&PaginationItem::TrailingGap));
}

#[test]
fn paging_clamps_at_both_ends() {
    let story = PaginationStory::new(GalleryOptions::default());
    for _ in 0..40 {
        story.next();
    }
    assert_eq!(story.page(), TOTAL_PAGES, "next clamps at the last page");
    assert_eq!(story.caption(), "page 25 of 25");
    for _ in 0..40 {
        story.previous();
    }
    assert_eq!(story.page(), 1, "previous clamps at the first page");
}

#[test]
fn an_empty_dataset_exposes_no_page_number() {
    let story = PaginationStory::new(GalleryOptions::default());
    assert!(
        story.empty_dataset_items().is_empty(),
        "no invalid page number for an empty dataset"
    );
}

#[test]
fn tags_are_independently_removable_and_resettable() {
    let story = TagStory::new(GalleryOptions::default());
    assert_eq!(story.tags().len(), INITIAL_TAGS.len());
    assert!(story.remove_tag("Editor"));
    assert_eq!(story.tags().len(), INITIAL_TAGS.len() - 1);
    assert!(!story.remove_tag("Editor"), "removing again is a no-op");
    story.reset();
    assert_eq!(story.tags().len(), INITIAL_TAGS.len());
}

#[test]
fn removing_every_tag_shows_the_empty_caption() {
    let story = TagStory::new(GalleryOptions::default());
    for tag in INITIAL_TAGS {
        assert!(story.remove_tag(tag), "{tag} should be removable");
    }
    assert_eq!(story.empty_caption(), Some("No tags selected"));
}

#[test]
fn the_breadcrumb_records_the_opened_destination() {
    let story = BreadcrumbStory::new();
    assert_eq!(story.opened(), "Project");
    story.open("textures");
    assert_eq!(story.opened(), "Textures");
    story.open("nope");
    assert_eq!(story.opened(), "Textures", "an unknown id is ignored");
    assert_eq!(story.items().len(), 6);
}

#[test]
fn the_dialog_cancel_does_not_commit_and_save_does() {
    let story = DialogStory::new();
    story.open();
    story.set_name("Aurora");
    story.cancel();
    assert!(!story.is_open());
    assert!(!story.is_saved(), "cancelling records no save");

    story.open();
    story.save();
    assert!(story.is_saved());
    assert!(!story.is_open());
}

#[test]
fn the_alert_dialog_only_deletes_on_confirm() {
    let story = AlertDialogStory::new();
    story.open();
    story.cancel();
    assert!(!story.is_deleted(), "cancelling leaves the target alone");

    story.open();
    story.confirm();
    assert!(story.is_deleted());
    assert!(!story.is_open());
}

#[test]
fn removing_the_label_tag_offers_a_restore() {
    let story = LabelStory::new();
    assert!(story.shows_tag());
    story.remove_tag();
    assert!(!story.shows_tag());
    story.restore_tag();
    assert!(story.shows_tag());
}

#[test]
fn the_sheet_records_the_edge_it_opened_from() {
    let story = SheetStory::new(GalleryOptions::default());
    assert_eq!(story.edge(), ModalEdge::Trailing);
    assert!(!story.is_open());
    story.open_from(ModalEdge::Top);
    assert_eq!(story.edge(), ModalEdge::Top);
    assert!(story.is_open());
    story.close();
    assert!(!story.is_open());
}

#[test]
fn the_bottom_sheet_reopens_at_its_initial_detent() {
    let story = BottomSheetStory::new(GalleryOptions::default());
    story.open();
    assert_eq!(story.detent(), 0);
    // Dragging past the larger detent promotes the sheet.
    story.drag(BOTTOM_SHEET_DETENTS[1] - BOTTOM_SHEET_DETENTS[0]);
    assert_eq!(story.detent(), 1);
    // Reopening resets, so a dragged sheet does not stay tall.
    story.open();
    assert_eq!(story.detent(), 0);
}

#[test]
fn dragging_down_past_the_threshold_dismisses() {
    let story = BottomSheetStory::new(GalleryOptions::default());
    story.open();
    let dismissed = story.drag(-DRAG_DISMISS_THRESHOLD);
    assert!(dismissed);
    assert!(!story.is_open());
}

#[test]
fn a_small_drag_changes_nothing() {
    let story = BottomSheetStory::new(GalleryOptions::default());
    story.open();
    assert!(!story.drag(-10.0));
    assert!(story.is_open());
    assert_eq!(story.detent(), 0);
}

#[test]
fn the_navigation_stack_pushes_and_pops_while_keeping_the_draft() {
    let story = NavigationStackStory::new();
    assert_eq!(story.caption(), "Path: root");
    story.set_draft("edited");
    story.push("details");
    story.push("settings");
    assert_eq!(story.caption(), "Path: details → settings");
    assert!(story.pop());
    assert_eq!(story.caption(), "Path: details");
    assert_eq!(story.draft(), "edited", "the root owns the draft and keeps it");
    assert!(story.pop());
    assert!(!story.pop(), "popping an empty stack is a no-op");
    assert_eq!(story.caption(), "Path: root");
}

#[test]
fn the_button_group_caption_reports_both_selections() {
    let story = ButtonGroupStory::new(GalleryOptions::default());
    assert_eq!(story.caption(), "Mode: grid  ·  Category: grid");
    // The unavailable segment is present but disabled in the authored list.
    let segments = story.mode_segments();
    assert_eq!(segments.len(), 3);
    assert!(!segments[2].2, "Map is unavailable");
    assert_eq!(story.category_segments().len(), 5);
}
