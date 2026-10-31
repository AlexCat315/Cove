//! Behaviour tests for the action-sheet story.
//!
//! Translation of `GalleryContent/Stories/Overlays/ActionSheetStory.swift`. The
//! contract worth pinning: a disabled command is rendered but skipped by arrow
//! navigation, and the destructive command is distinguishable.

use cove_examples::gallery_content::catalog::GalleryOptions;
use cove_examples::gallery_content::stories::action_sheet_story::{
    ActionSheetStory, SHORTCUT_COUNT, first_available, is_destructive, navigable_ids,
};

fn story() -> ActionSheetStory {
    ActionSheetStory::new(GalleryOptions::default())
}

#[test]
fn the_project_sheet_lists_four_commands() {
    let story = story();
    let actions = story.project_actions();
    assert_eq!(actions.len(), 4);
    let ids: Vec<&str> = actions.iter().map(|action| action.id.as_str()).collect();
    assert_eq!(ids, vec!["duplicate", "archive", "export", "delete"]);
}

#[test]
fn export_starts_disabled_and_keeps_its_icon() {
    let story = story();
    let actions = story.project_actions();
    let export = actions.iter().find(|action| action.id == "export").unwrap();
    assert!(!export.is_enabled, "the checkbox starts off");
    assert!(
        export.icon.is_some(),
        "a disabled command still shows its icon"
    );
}

#[test]
fn enabling_export_makes_it_navigable() {
    let story = story();
    assert!(!navigable_ids(&story.project_actions()).contains(&"export".to_owned()));
    story.set_export_enabled(true);
    assert!(navigable_ids(&story.project_actions()).contains(&"export".to_owned()));
}

#[test]
fn the_close_label_names_the_sheet() {
    let story = story();
    assert_eq!(
        story.project_question().close_label,
        "Close project actions"
    );
    assert_eq!(story.project_question().title, "Aurora project");
}

#[test]
fn delete_is_the_destructive_command() {
    let story = story();
    let actions = story.project_actions();
    let delete = actions.iter().find(|action| action.id == "delete").unwrap();
    assert!(is_destructive(delete));
    let duplicate = actions
        .iter()
        .find(|action| action.id == "duplicate")
        .unwrap();
    assert!(!is_destructive(duplicate));
}

#[test]
fn the_first_available_command_skips_a_disabled_leader() {
    let story = story();
    let mut actions = story.project_actions();
    actions[0].is_enabled = false;
    let first = first_available(&actions).expect("archive is available");
    assert_eq!(first.id, "archive");
}

#[test]
fn running_a_command_records_it() {
    let story = story();
    let actions = story.project_actions();
    let duplicate = actions
        .iter()
        .find(|action| action.id == "duplicate")
        .unwrap();
    duplicate.perform();
    assert_eq!(story.last_action(), "Duplicated");
}

#[test]
fn the_long_sheet_generates_the_declared_shortcut_count() {
    let story = story();
    let actions = story.shortcut_actions();
    assert_eq!(actions.len(), SHORTCUT_COUNT);
    assert_eq!(actions[0].title, "Workspace shortcut 1");
    assert_eq!(
        actions[SHORTCUT_COUNT - 1].title,
        format!("Workspace shortcut {SHORTCUT_COUNT}")
    );
}

#[test]
fn shortcut_ids_are_unique() {
    // `ActionSheetQuestion::new` asserts on duplicates, so this guards the
    // generator rather than the assertion itself.
    let story = story();
    let ids: Vec<String> = story
        .shortcut_actions()
        .into_iter()
        .map(|action| action.id)
        .collect();
    let unique: std::collections::HashSet<&String> = ids.iter().collect();
    assert_eq!(unique.len(), ids.len());
}

#[test]
fn the_sheet_starts_closed_with_no_action() {
    let story = story();
    assert!(!story.is_open());
    assert_eq!(story.last_action(), "None");
    story.set_open(true);
    assert!(story.is_open());
}
