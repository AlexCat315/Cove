//! Behaviour tests for the command-palette story.
//!
//! Translation of `GalleryContent/Stories/Overlays/CommandStory.swift`. The prose
//! promises specific queries work and that a disabled command is skipped, so
//! those are asserted against the real ranking model.

use cove_examples::gallery_content::catalog::GalleryOptions;
use cove_examples::gallery_content::stories::command_story::{
    commands, CommandStory, RECENT_LIMIT,
};

fn story() -> CommandStory {
    CommandStory::new(GalleryOptions::default())
}

fn ids(items: &[cove_ui_compose::primitives::overlays::CommandItem]) -> Vec<&str> {
    items.iter().map(|item| item.id.as_str()).collect()
}

#[test]
fn the_catalogue_lists_eight_commands_across_four_groups() {
    let items = commands();
    assert_eq!(items.len(), 8);
    let groups: std::collections::HashSet<&str> =
        items.iter().map(|item| item.group.as_str()).collect();
    assert_eq!(groups.len(), 4);
    assert!(groups.contains("View"));
    assert!(groups.contains("Navigate"));
    assert!(groups.contains("Edit"));
    assert!(groups.contains("Build"));
}

#[test]
fn only_deploy_is_disabled() {
    let items = commands();
    let disabled: Vec<&str> =
        items.iter().filter(|item| !item.is_enabled).map(|item| item.id.as_str()).collect();
    assert_eq!(disabled, vec!["Deploy to Device"]);
}

#[test]
fn the_advertised_queries_find_the_expected_commands() {
    let story = story();
    // "GO" (case-insensitive) matches the two Go-to commands.
    let go = story.ranked("GO");
    let found = ids(&go);
    assert!(found.contains(&"Go to File"), "got {found:?}");
    assert!(found.contains(&"Go to Line"), "got {found:?}");
    // "go f" narrows to Go to File.
    let go_f = story.ranked("go f");
    let narrowed = ids(&go_f);
    assert_eq!(narrowed.first(), Some(&"Go to File"), "got {narrowed:?}");
    // "appearance" is a keyword of Toggle Theme.
    let appearance = story.ranked("appearance");
    assert_eq!(ids(&appearance), vec!["Toggle Theme"]);
}

#[test]
fn an_enabled_command_runs_and_is_recorded() {
    let story = story();
    assert!(story.run("Run Build"));
    assert_eq!(story.last_run(), "Run Build");
    assert_eq!(story.recent(), vec!["Run Build".to_owned()]);
    assert!(!story.is_presented(), "running closes the palette");
}

#[test]
fn a_disabled_command_never_runs() {
    let story = story();
    assert!(!story.run("Deploy to Device"));
    assert_eq!(story.last_run(), "none", "a disabled command is skipped, not run");
    assert!(story.recent().is_empty());
}

#[test]
fn recents_are_most_recent_first_and_capped() {
    let story = story();
    story.run("Run Build");
    story.run("Go to File");
    story.run("Rename Symbol");
    assert_eq!(
        story.recent(),
        vec!["Rename Symbol".to_owned(), "Go to File".to_owned(), "Run Build".to_owned()]
    );
    // A fourth run pushes the oldest out.
    story.run("Toggle Theme");
    assert_eq!(story.recent().len(), RECENT_LIMIT);
    assert_eq!(story.recent()[0], "Toggle Theme");
    assert!(!story.recent().contains(&"Run Build".to_owned()));
}

#[test]
fn running_the_same_command_twice_does_not_duplicate_it() {
    let story = story();
    story.run("Run Build");
    story.run("Run Build");
    assert_eq!(story.recent(), vec!["Run Build".to_owned()]);
}

#[test]
fn recent_rows_appear_when_no_query_is_typed() {
    let story = story();
    story.run("Run Build");
    let entries = story.entries("");
    // The first row is the Recent header, the second the recorded command.
    assert_eq!(entries[0].id(), "header:Recent");
    assert_eq!(entries[1].id(), "item:Run Build");
}

#[test]
fn a_query_suppresses_the_recent_section() {
    let story = story();
    story.run("Run Build");
    // With a query the results are ranked, never prefixed with Recent.
    assert!(!story.entries("build").iter().any(|entry| entry.id() == "header:Recent"));
}

#[test]
fn arrow_navigation_wraps() {
    let story = story();
    let matches = story.ranked("go").len();
    assert!(matches >= 2);
    assert_eq!(story.next_highlight(matches, Some(matches - 1), 1, true), Some(0));
    assert_eq!(story.next_highlight(matches, Some(0), -1, true), Some(matches - 1));
}

#[test]
fn opening_before_running_shows_an_empty_history() {
    let story = story();
    story.open();
    assert!(story.is_presented());
    assert_eq!(story.last_run(), "none");
    assert!(story.recent().is_empty());
}
