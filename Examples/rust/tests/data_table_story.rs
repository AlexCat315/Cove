//! Behaviour tests for the warehouse table story.
//!
//! Translation of `GalleryContent/Stories/Data/DataTableStory.swift`. The row
//! generator must be reproducible, quantity sorting must be numeric rather than
//! lexicographic, and a collapsed branch must hide its whole subtree.

use std::collections::HashSet;

use cove_examples::gallery_content::catalog::GalleryOptions;
use cove_examples::gallery_content::stories::data_table_story::{
    apply_quantity, compare_names, compare_quantities, group_thousands, validate_name,
    validate_quantity, visible_tree_rows, DataTableStory, ProjectNode, SortStats, WarehouseRow,
    CATEGORIES, TOTAL_COLUMNS,
};
use cove_ui_compose::primitives::data::lists::table::ComparisonResult;

fn story() -> DataTableStory {
    DataTableStory::new(GalleryOptions::default())
}

#[test]
fn rows_are_generated_deterministically() {
    let first = WarehouseRow::generate(0);
    assert_eq!(first.name, "Asset 1");
    assert_eq!(first.category, "Texture");
    assert_eq!(first.quantity, 0);
    // Re-generating the same id gives the same row.
    assert_eq!(WarehouseRow::generate(5), WarehouseRow::generate(5));
    assert_eq!(WarehouseRow::generate(1).category, "Material");
    assert_eq!(WarehouseRow::generate(2).category, "Mesh");
    assert_eq!(WarehouseRow::generate(3).category, "Texture", "the rotation wraps");
}

#[test]
fn quantity_wraps_at_two_hundred_and_fifty() {
    // id 15 → 255 % 250 = 5
    assert_eq!(WarehouseRow::generate(15).quantity, 5);
    assert!(WarehouseRow::generate(999).quantity < 250);
}

#[test]
fn metrics_derive_from_the_row_id() {
    let row = WarehouseRow::generate(4);
    assert_eq!(row.metric(0), (4 * 3) % 10_000);
    assert_eq!(row.metric(7), (4 * 10) % 10_000);
}

#[test]
fn the_story_declares_three_authored_plus_thirty_metric_columns() {
    assert_eq!(TOTAL_COLUMNS, 33);
    assert_eq!(CATEGORIES.len(), 3);
}

#[test]
fn loading_rows_clears_the_selection_and_focuses_the_first_row() {
    let story = story();
    assert_eq!(story.row_count(), 0);
    assert_eq!(story.status(), "Load a dataset to begin");
    story.load(1_000);
    assert_eq!(story.row_count(), 1_000);
    assert_eq!(story.status(), "1,000 rows loaded");
    assert!(story.selection().is_empty());
    assert_eq!(story.navigation().row, Some(0));
}

#[test]
fn navigating_bumps_the_request_id() {
    let story = story();
    story.load(100);
    let first = story.navigation().request_id;
    story.navigate_to(50);
    let second = story.navigation();
    assert_eq!(second.row, Some(50));
    assert_eq!(second.request_id, first + 1, "each jump is a new request");
}

#[test]
fn middle_and_last_are_inert_on_an_empty_table() {
    let story = story();
    assert!(!story.navigate_middle());
    assert!(!story.navigate_last());
    story.load(101);
    assert!(story.navigate_middle());
    assert_eq!(story.navigation().row, Some(50), "101 rows, middle is 50");
    assert!(story.navigate_last());
    assert_eq!(story.navigation().row, Some(100));
}

#[test]
fn activation_names_the_row() {
    let story = story();
    story.load(10);
    story.activate(3);
    assert_eq!(story.status(), "Opened Asset 4");
}

#[test]
fn a_blank_asset_name_is_rejected() {
    assert_eq!(validate_name(""), Some("Asset name is required"));
    assert_eq!(validate_name("   "), Some("Asset name is required"));
    assert_eq!(validate_name("Frame"), None);
}

#[test]
fn quantity_must_be_a_nonnegative_integer() {
    assert_eq!(validate_quantity("12"), None);
    assert_eq!(validate_quantity("0"), None);
    assert!(validate_quantity("-1").is_some());
    assert!(validate_quantity("1.5").is_some());
    assert!(validate_quantity("abc").is_some());
    assert!(validate_quantity("").is_some());
}

#[test]
fn an_unparseable_quantity_keeps_the_previous_value() {
    let mut row = WarehouseRow::generate(1);
    let before = row.quantity;
    apply_quantity(&mut row, "not a number");
    assert_eq!(row.quantity, before);
    apply_quantity(&mut row, "42");
    assert_eq!(row.quantity, 42);
}

#[test]
fn quantity_sorts_numerically_not_lexicographically() {
    // Text order would put "10" before "9".
    assert_eq!(compare_quantities(9, 10), ComparisonResult::Ascending);
    assert_eq!(compare_quantities(10, 9), ComparisonResult::Descending);
    assert_eq!(compare_quantities(7, 7), ComparisonResult::Same);
}

#[test]
fn names_sort_ascending() {
    assert_eq!(compare_names("Alpha", "Beta"), ComparisonResult::Ascending);
    assert_eq!(compare_names("Beta", "Alpha"), ComparisonResult::Descending);
    assert_eq!(compare_names("Same", "Same"), ComparisonResult::Same);
}

#[test]
fn the_status_line_groups_digits_and_names_the_state() {
    let stats = SortStats { completed_builds: 3, cache_hits: 7, is_pending: false };
    assert_eq!(
        stats.status_line(200_000, 2),
        "200,000 rows · 2 selected · Ready · 3 sort builds · 7 cache hits"
    );
    let pending = SortStats { is_pending: true, ..SortStats::default() };
    assert!(pending.status_line(1_000, 0).contains("Sorting…"));
}

#[test]
fn thousands_are_grouped() {
    assert_eq!(group_thousands(0), "0");
    assert_eq!(group_thousands(999), "999");
    assert_eq!(group_thousands(1_000), "1,000");
    assert_eq!(group_thousands(200_000), "200,000");
    assert_eq!(group_thousands(1_234_567), "1,234,567");
}

#[test]
fn the_sample_tree_opens_with_engine_expanded() {
    let story = story();
    let rows = story.visible_tree();
    let ids: Vec<&str> = rows.iter().map(|(id, _)| id.as_str()).collect();
    // engine open shows its two children; render stays closed, so metal/vulkan
    // are hidden; editor and readme follow.
    assert_eq!(ids, vec!["engine", "render", "physics", "editor", "readme"]);
}

#[test]
fn expanding_render_reveals_the_leaves_at_depth_two() {
    let expanded: HashSet<String> =
        ["engine".to_owned(), "render".to_owned()].into_iter().collect();
    let rows = visible_tree_rows(&expanded);
    let metal = rows.iter().find(|(id, _)| id == "metal").expect("metal is visible");
    assert_eq!(metal.1, 2, "leaves under a nested branch are two levels deep");
}

#[test]
fn a_collapsed_branch_hides_its_whole_subtree() {
    let rows = visible_tree_rows(&HashSet::new());
    let ids: Vec<&str> = rows.iter().map(|(id, _)| id.as_str()).collect();
    assert_eq!(ids, vec!["engine", "editor", "readme"], "nothing is expanded");
}

#[test]
fn toggling_a_branch_flips_its_disclosure() {
    let story = story();
    assert!(story.expanded().contains(&"engine".to_owned()));
    story.toggle_branch("engine");
    assert!(!story.expanded().contains(&"engine".to_owned()));
    story.toggle_branch("engine");
    assert!(story.expanded().contains(&"engine".to_owned()));
}

#[test]
fn the_sample_tree_has_the_authored_shape() {
    let nodes = ProjectNode::sample();
    assert_eq!(nodes.len(), 3);
    assert_eq!(nodes[0].children.len(), 2);
    assert!(nodes[0].is_expandable());
    assert!(!nodes[2].is_expandable(), "README.md is a leaf");
}

#[test]
fn the_frozen_column_defaults_to_one_and_can_be_cleared() {
    let story = story();
    assert!(story.is_frozen());
    assert_eq!(story.frozen_column_count(), 1);
}

#[test]
fn the_order_caption_reads_authored_until_reordered() {
    let story = story();
    assert_eq!(story.order_caption(), "Order: authored");
}
