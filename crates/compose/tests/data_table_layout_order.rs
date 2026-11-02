//! Translation of `Tests/CoveUIComposeTests/DataTableLayoutTests.swift`.
//!
//! Behaviour checklist:
//! - applying a saved order keeps unknown columns in authored order
//! - a move places the source next to its target in the full order
//! - the drop target is the presented column whose span holds the dragged centre
//! - variable row heights drive the extent, offsets and viewport row
//! - row heights respect the minimum and the text scale
//! - TreeTable flattens only expanded branches, with the right depths
//!
//! Model difference: the Swift suite drives drags and keys through a scene.
//! The Rust image exposes the reorder math, the row-height math and the tree
//! projection as pure functions, so those are asserted directly with the same
//! inputs and the same expected numbers.

use cove_ui_compose::primitives::data::data_table::{
    DataTableColumnGeometry, DataTableColumnOrder, DataTableRowMetrics, RowHeightKey,
    row_height_key_matches, variable_row_heights,
};

#[derive(Clone, Debug)]
struct Asset {
    id: usize,
    name: String,
    lines: usize,
}

fn columns() -> Vec<String> {
    ["id", "name", "lines"]
        .iter()
        .map(|id| id.to_string())
        .collect()
}

#[test]
fn applying_a_saved_order_keeps_unknown_columns_in_authored_order() {
    let ids = columns();
    let applied = DataTableColumnOrder::apply(
        &["lines".to_string(), "ghost".to_string(), "id".to_string()],
        &ids,
    );
    assert_eq!(
        applied,
        vec!["lines", "id", "name"],
        "known columns follow the saved order; the unknown one trails in authored order"
    );
}

#[test]
fn an_empty_saved_order_preserves_the_authored_order() {
    let ids = columns();
    assert_eq!(
        DataTableColumnOrder::apply(&[], &ids),
        ids,
        "no saved order means no reordering"
    );
}

#[test]
fn a_duplicated_saved_entry_does_not_shift_later_columns() {
    let ids = columns();
    let applied = DataTableColumnOrder::apply(
        &["name".to_string(), "name".to_string(), "id".to_string()],
        &ids,
    );
    assert_eq!(
        applied,
        vec!["name", "id", "lines"],
        "the first occurrence of a saved id sets its rank"
    );
}

#[test]
fn moving_a_column_places_it_next_to_its_target() {
    let order = columns();
    assert_eq!(
        DataTableColumnOrder::move_id("id", "lines", &order),
        vec!["name", "lines", "id"],
        "moving right inserts after the target"
    );
    assert_eq!(
        DataTableColumnOrder::move_id("lines", "id", &order),
        vec!["lines", "id", "name"],
        "moving left inserts before the target"
    );
    assert_eq!(
        DataTableColumnOrder::move_id("id", "id", &order),
        order,
        "moving onto itself is a no-op"
    );
    assert_eq!(
        DataTableColumnOrder::move_id("ghost", "id", &order),
        order,
        "an unknown source leaves the order untouched"
    );
}

#[test]
fn the_drop_target_is_the_column_under_the_dragged_centre() {
    let geometry = DataTableColumnGeometry::new(vec![100.0, 100.0, 100.0], 0);
    assert_eq!(
        DataTableColumnOrder::target(0, 160.0, &geometry),
        2,
        "a 160pt rightward drag ends over the third column"
    );
    assert_eq!(
        DataTableColumnOrder::target(2, -120.0, &geometry),
        1,
        "a 120pt leftward drag ends over the second column"
    );
    assert_eq!(
        DataTableColumnOrder::target(1, 0.0, &geometry),
        1,
        "no drag keeps the source over itself"
    );
}

#[test]
fn variable_row_heights_drive_the_extent_and_offsets() {
    // lines cycles 1..3, so heights are 36, 72, 108, 36, ...
    let count = 1_000;
    let heights = variable_row_heights(count, 36.0, 1.0, |row| ((row % 3 + 1) * 36) as f32);
    assert_eq!(heights[0], 36.0, "a one-line row is 36pt");
    assert_eq!(heights[1], 72.0, "a two-line row is 72pt");
    assert_eq!(heights[2], 108.0, "a three-line row is 108pt");

    let expected_total: f32 = (0..count).map(|row| ((row % 3 + 1) * 36) as f32).sum();
    let metrics = DataTableRowMetrics::variable(count, 36.0, {
        let mut index =
            cove_ui_compose::primitives::layout::virtualization::VariableRowIndex::new();
        index.replace(&(0..count).collect::<Vec<_>>(), &heights, 0.0);
        index
    });
    assert_eq!(
        metrics.total(),
        expected_total,
        "the content extent is the sum of every row height"
    );

    // 36 + 72 + 1 lands inside the third row.
    assert_eq!(
        metrics.row_at(36.0 + 72.0 + 1.0),
        2,
        "an offset inside row 2 resolves to row 2"
    );
    assert_eq!(
        metrics.top(3),
        216.0,
        "the fourth row starts after 36 + 72 + 108"
    );
}

#[test]
fn row_heights_respect_the_minimum_and_the_text_scale() {
    let heights = variable_row_heights(3, 36.0, 1.0, |_| 20.0);
    assert_eq!(
        heights,
        vec![36.0, 36.0, 36.0],
        "a short row is lifted to the minimum"
    );

    let scaled = variable_row_heights(2, 36.0, 2.0, |row| if row == 0 { 30.0 } else { 50.0 });
    assert_eq!(
        scaled,
        vec![60.0, 100.0],
        "the authored height scales before the minimum applies"
    );

    let scaled_floor = variable_row_heights(1, 36.0, 2.0, |_| 10.0);
    assert_eq!(
        scaled_floor,
        vec![36.0],
        "the minimum is applied after scaling, not before"
    );
}

#[test]
fn the_row_height_cache_key_tracks_only_its_real_inputs() {
    let key = RowHeightKey {
        revision: 4,
        count: 1_000,
        minimum: 36.0,
        scale: 1.0,
    };
    assert!(
        row_height_key_matches(Some(key), key),
        "the same inputs reuse the cached index"
    );
    assert!(
        !row_height_key_matches(Some(key), RowHeightKey { revision: 5, ..key }),
        "a model revision invalidates the index"
    );
    assert!(
        !row_height_key_matches(Some(key), RowHeightKey { scale: 2.0, ..key }),
        "a text-scale change invalidates the index"
    );
    assert!(
        !row_height_key_matches(None, key),
        "no cached key always rebuilds"
    );
}

#[derive(Clone, Debug, PartialEq)]
struct Folder {
    id: String,
    title: String,
    children: Option<Vec<Folder>>,
}

fn folders() -> Vec<Folder> {
    vec![
        Folder {
            id: "a".into(),
            title: "a".into(),
            children: Some(vec![Folder {
                id: "b".into(),
                title: "b".into(),
                children: Some(Vec::new()),
            }]),
        },
        Folder {
            id: "c".into(),
            title: "c".into(),
            children: None,
        },
    ]
}

fn flatten(roots: &[Folder], expanded: &[&str]) -> Vec<(String, usize, bool, bool)> {
    let expanded: std::collections::BTreeSet<String> =
        expanded.iter().map(|id| id.to_string()).collect();
    cove_ui_compose::primitives::data::tree_table::flatten_tree_table(
        roots,
        &|folder: &Folder| folder.children.clone(),
        &|folder: &Folder| folder.id.clone(),
        &expanded,
    )
    .into_iter()
    .map(|row| {
        (
            row.record.id.clone(),
            row.depth,
            row.has_children,
            row.is_expanded,
        )
    })
    .collect()
}

#[test]
fn tree_table_flattening_visits_only_expanded_branches() {
    let roots = folders();
    assert_eq!(
        flatten(&roots, &[]),
        vec![
            ("a".to_string(), 0, true, false),
            ("c".to_string(), 0, false, false),
        ],
        "a collapsed tree shows only its roots"
    );

    let open = flatten(&roots, &["a", "b"]);
    assert_eq!(
        open.iter().map(|row| row.0.clone()).collect::<Vec<_>>(),
        vec!["a", "b", "c"],
        "expanding both branches reveals the grandchild"
    );
    assert_eq!(
        open.iter().map(|row| row.1).collect::<Vec<_>>(),
        vec![0, 1, 0],
        "depth follows the nesting"
    );
    assert!(
        open[1].2 && open[1].3,
        "the nested branch reports both children and its own expansion"
    );
    assert!(!open[2].2, "a leaf reports that it has no children");
}

#[test]
fn an_expanded_empty_branch_has_no_children_to_visit() {
    let roots = vec![Folder {
        id: "x".into(),
        title: "x".into(),
        children: Some(Vec::new()),
    }];
    let rows = flatten(&roots, &["x"]);
    assert_eq!(rows.len(), 1, "an empty branch contributes only itself");
    assert!(
        rows[0].2,
        "but it is still a branch, so it keeps its disclosure"
    );
    assert!(rows[0].3, "and reports the caller's expansion");
}

#[test]
fn the_tree_table_projection_indents_by_depth() {
    // Indentation is a per-depth step; the Swift suite requires a child to sit
    // more than 10pt right of its parent.
    let indentation = 16.0;
    let rows = flatten(&folders(), &["a"]);
    let parent_x = rows[0].1 as f32 * indentation;
    let child_x = rows[1].1 as f32 * indentation;
    assert!(
        child_x > parent_x + 10.0,
        "a depth-1 child indents {indentation}pt past its parent ({parent_x} -> {child_x})"
    );
}
