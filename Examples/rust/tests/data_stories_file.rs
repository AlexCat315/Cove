//! Behaviour tests for the data story file.
//!
//! Translation of `GalleryContent/Stories/Data/DataStories.swift`, which holds
//! thirteen stories. Tree search (matching ancestors stay visible) and the refresh
//! re-entrancy guard are the parts with real edge cases.

use cove_examples::gallery_content::catalog::GalleryOptions;
use cove_examples::gallery_content::stories::data_stories_file::{
    DragAndDropStory, REFRESH_ROWS, REFRESH_THRESHOLD, RefreshListStory, SWIPE_ROWS, SceneNode,
    SectionedListStory, SwipeListStory, TreeStory, VIRTUAL_MIDDLE, VIRTUAL_STACK_COUNT,
    VirtualStackStory, expandable_ids, node_matches, visible_ids,
};

fn story() -> TreeStory {
    TreeStory::new(GalleryOptions::default())
}

#[test]
fn an_empty_query_keeps_the_whole_tree() {
    let visible = visible_ids(&SceneNode::gallery(), "");
    // scene, lights, key, fill, objects, sphere, camera.
    assert_eq!(visible.len(), 7, "every node survives an empty query");
    assert!(visible.contains("scene"));
    assert!(visible.contains("camera"));
}

#[test]
fn matching_is_case_insensitive() {
    assert!(node_matches("Key Light", "key"));
    assert!(node_matches("Key Light", "KEY"));
    assert!(!node_matches("Key Light", "fill"));
}

#[test]
fn a_deep_match_keeps_its_ancestors_visible() {
    // "Sphere" is a leaf under Scene → Objects.
    let visible = visible_ids(&SceneNode::gallery(), "sphere");
    assert!(visible.contains("sphere"), "the match itself");
    assert!(visible.contains("objects"), "its parent");
    assert!(visible.contains("scene"), "and the root");
    assert!(
        !visible.contains("lights"),
        "unrelated siblings are filtered out"
    );
    assert!(!visible.contains("camera"));
}

#[test]
fn a_parent_match_keeps_it_and_its_ancestors_but_not_its_children() {
    // The contract is "retains matching ancestors": a match survives along with
    // the path to it, but its children appear only if they match themselves.
    // Including them unconditionally would make a broad query dump whole
    // subtrees the person did not search for.
    let visible = visible_ids(&SceneNode::gallery(), "lights");
    assert!(visible.contains("lights"), "the match");
    assert!(visible.contains("scene"), "its ancestor");
    assert!(
        !visible.contains("key"),
        "a non-matching child is filtered out"
    );
    assert!(!visible.contains("fill"));
}

#[test]
fn a_query_matching_nothing_hides_everything() {
    let visible = visible_ids(&SceneNode::gallery(), "zzzz");
    assert!(visible.is_empty(), "no match and no ancestors to retain");
}

#[test]
fn the_story_starts_with_scene_and_lights_expanded() {
    let story = story();
    assert_eq!(story.expanded().len(), 2);
    assert!(story.expanded().contains("scene"));
    assert!(story.expanded().contains("lights"));
    assert_eq!(story.query(), "");
    assert_eq!(story.caption(), "Selected: camera");
}

#[test]
fn expand_all_sets_exactly_the_expandable_ids() {
    let story = story();
    story.expand_all();
    let expected = expandable_ids(&SceneNode::gallery());
    assert_eq!(
        story.expanded(),
        expected.into_iter().map(str::to_owned).collect()
    );
    // Leaves are not expandable, so they must not appear.
    assert!(!story.expanded().contains("key"));
    assert_eq!(story.expanded().len(), 3, "scene, lights and objects");
}

#[test]
fn collapse_all_empties_the_expansion() {
    let story = story();
    story.expand_all();
    story.collapse_all();
    assert!(story.expanded().is_empty());
}

#[test]
fn multi_selection_toggles_one_id_at_a_time() {
    let story = story();
    assert_eq!(story.selected_ids().len(), 1);
    story.toggle_multi("scene");
    assert_eq!(story.selected_ids().len(), 2);
    assert_eq!(story.caption(), "Selected: camera, scene");
    story.toggle_multi("scene");
    assert_eq!(story.selected_ids().len(), 1, "toggling again deselects");
}

#[test]
fn the_virtual_stack_jumps_to_the_declared_targets() {
    let story = VirtualStackStory::new();
    assert_eq!(story.target(), 0);
    story.scroll_to_middle();
    assert_eq!(story.target(), VIRTUAL_MIDDLE);
    story.scroll_to_last();
    assert_eq!(
        story.target(),
        VIRTUAL_STACK_COUNT - 1,
        "the last row, not past it"
    );
    story.scroll_to_first();
    assert_eq!(story.target(), 0);
    assert_eq!(
        VirtualStackStory::row_label(0),
        "Record 1",
        "labels are one-based"
    );
}

#[test]
fn the_swipe_log_records_activation_and_actions() {
    let story = SwipeListStory::new();
    assert_eq!(story.log(), "No actions yet");
    assert!(story.activate(0));
    assert_eq!(story.log(), "Opened Inbox");
    assert!(story.run_action(0, "archive"));
    assert_eq!(story.log(), "Archived Inbox");
    assert!(story.run_action(2, "delete"));
    assert_eq!(story.log(), "Deleted Release notes");
    assert!(!story.run_action(99, "delete"), "an unknown row is a no-op");
    assert!(
        !story.run_action(0, "explode"),
        "an unknown action is a no-op"
    );
    assert_eq!(SWIPE_ROWS.len(), 5);
}

#[test]
fn the_swipe_rows_offer_two_actions_each() {
    let actions = SwipeListStory::actions(0);
    assert_eq!(actions.len(), 2, "at most two trailing actions");
    assert_eq!(actions[0].1, "Archive");
    assert_eq!(actions[1].1, "Delete");
    assert!(actions[1].2, "delete is destructive");
}

#[test]
fn a_refresh_cannot_re_enter() {
    let story = RefreshListStory::new();
    assert!(!story.is_refreshing());
    assert!(story.start_refresh(), "the first pull starts");
    assert_eq!(story.refreshes(), 1);
    assert!(
        !story.start_refresh(),
        "a second pull while in flight is ignored"
    );
    assert_eq!(story.refreshes(), 1, "the count does not advance");
}

#[test]
fn a_finished_refresh_allows_the_next_one() {
    let story = RefreshListStory::new();
    story.start_refresh();
    story.finish_refresh();
    assert!(!story.is_refreshing());
    assert!(story.start_refresh());
    assert_eq!(story.refreshes(), 2);
}

#[test]
fn the_refresh_threshold_is_sixty_four_points() {
    let story = RefreshListStory::new();
    let options = story.options();
    assert_eq!(REFRESH_THRESHOLD, 64.0);
    assert!(!options.should_trigger(63.9), "below the threshold");
    assert!(options.should_trigger(64.0), "at the threshold");
}

#[test]
fn a_refresh_in_flight_does_not_trigger_again() {
    let story = RefreshListStory::new();
    story.start_refresh();
    let options = story.options();
    assert!(
        !options.should_trigger(200.0),
        "an in-flight refresh blocks further triggers however far the pull goes"
    );
    assert_eq!(REFRESH_ROWS.len(), 6);
}

#[test]
fn a_drop_lands_in_the_lane_that_accepted_it() {
    let story = DragAndDropStory::new();
    assert_eq!(story.lane_a(), None);
    story.drop_into_a("camera");
    assert_eq!(story.lane_a().as_deref(), Some("camera"));
    assert_eq!(story.lane_b(), None, "the other lane is untouched");
    story.drop_into_b("light");
    assert_eq!(story.lane_b().as_deref(), Some("light"));
}

#[test]
fn only_a_matching_payload_kind_is_accepted() {
    assert!(DragAndDropStory::accepts("camera", "camera"));
    assert!(!DragAndDropStory::accepts("light", "camera"));
}

#[test]
fn the_sectioned_list_has_three_headers() {
    let story = SectionedListStory::new();
    assert_eq!(story.sections(), ["Today", "Yesterday", "Earlier"]);
    assert_eq!(story.selection(), Some(0));
}
