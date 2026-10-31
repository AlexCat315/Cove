//! Behaviour tests for the toolbar story.
//!
//! Translation of `GalleryContent/Stories/Controls/ToolbarStory.swift`. The
//! adaptive bar's contract is which commands stay inline as it narrows.

use cove_examples::gallery_content::catalog::GalleryOptions;
use cove_examples::gallery_content::stories::ToolbarStory;

fn story() -> ToolbarStory {
    ToolbarStory::new(GalleryOptions::default())
}

#[test]
fn the_run_command_flips_with_playback() {
    let story = story();
    assert_eq!(story.run_title(), "Run");
    assert!(!story.is_playing());
    let items = story.items();
    items[0].command.perform();
    assert!(story.is_playing());
    assert_eq!(story.run_title(), "Pause", "the title follows the state");
}

#[test]
fn a_wide_bar_keeps_every_automatic_command_inline() {
    let story = story();
    // Five commands plus the overflow button, all 40pt wide, with room to spare.
    let arrangement = story.arrangement(600.0, 40.0);
    // Settings is declared `Overflow`, so it is always in the menu regardless of
    // width; the other four stay inline when there is room.
    assert_eq!(
        arrangement.hidden,
        vec![4],
        "only the overflow-only command hides"
    );
    let mut visible = arrangement.visible.clone();
    visible.sort_unstable();
    assert_eq!(
        visible,
        vec![0, 1, 2, 3, 5],
        "commands 0..3 plus the More button"
    );
}

#[test]
fn a_narrow_bar_moves_overflow_only_commands_out_first() {
    let story = story();
    // Only enough room for three items.
    let arrangement = story.arrangement(3.0 * 40.0, 40.0);
    // The `Overflow`-placed settings command must be hidden, never inlined.
    assert!(
        arrangement.hidden.contains(&4),
        "settings is overflow-only, hidden = {:?}",
        arrangement.hidden
    );
    // And `Always`-placed Run must stay.
    assert!(
        arrangement.visible.contains(&0),
        "run is always visible, visible = {:?}",
        arrangement.visible
    );
}

#[test]
fn a_very_narrow_bar_still_keeps_the_always_command() {
    let story = story();
    let arrangement = story.arrangement(40.0, 40.0);
    assert!(
        arrangement.visible.contains(&0),
        "always-visible commands survive"
    );
}

#[test]
fn the_story_starts_with_no_applied_action() {
    let story = story();
    assert_eq!(story.applied_action(), "None");
    story.record("Saved");
    assert_eq!(story.applied_action(), "Saved");
}

#[test]
fn the_story_declares_five_commands_in_authored_order() {
    let story = story();
    assert_eq!(story.command_placements().len(), 5);
    assert_eq!(story.items().len(), 5);
    // IDs must be unique or the toolbar asserts.
    let ids: Vec<u64> = story.items().iter().map(|item| item.command.id).collect();
    let unique: std::collections::HashSet<u64> = ids.iter().copied().collect();
    assert_eq!(unique.len(), ids.len());
}
