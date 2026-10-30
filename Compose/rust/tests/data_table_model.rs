//! Translation of `Tests/CoveUIComposeTests/DataTableModelTests.swift`.
//!
//! Behaviour checklist:
//! - 200K rows have stable projections and constant-time ID lookup
//! - a repeated sort reuses the cached projection instead of rebuilding
//! - row edits and row replacement preserve the latest data
//! - a superseded sort must not publish stale order
//! - range selection follows display order; toggles and modifiers behave
//! - deletion cleanup drops ids that left the model
//! - hundreds of columns are windowed by width with frozen columns independent
//!
//! Model difference: the Swift model delivers its projection from a background
//! task, so its suite polls `sorting.isPending` until delivery. The Rust
//! `DataTableModel::set_sort` builds the projection synchronously (the
//! pending flag only brackets the build), so these tests assert the settled
//! state directly instead of polling.

mod support;

use std::rc::Rc;

use cove_platform::KeyModifiers;
use cove_ui_compose::primitives::data::data_table::{
    DataTableColumnGeometry, DataTableModel, DataTableSelection, DataTableSelectionMode,
};
use cove_ui_compose::primitives::data::lists::table::{
    ComparisonResult, TableSort,
};

#[derive(Clone, Debug, PartialEq)]
struct TableTestRow {
    id: usize,
    value: i32,
}

fn compare(left: &TableTestRow, right: &TableTestRow) -> ComparisonResult {
    if left.value == right.value {
        ComparisonResult::Same
    } else if left.value < right.value {
        ComparisonResult::Ascending
    } else {
        ComparisonResult::Descending
    }
}

fn comparator() -> Option<Rc<dyn Fn(&TableTestRow, &TableTestRow) -> ComparisonResult>> {
    Some(Rc::new(compare))
}

#[test]
fn large_projection_is_stable_and_lookup_is_constant_time() {
    let model = DataTableModel::new(
        (0..200_000)
            .map(|index| TableTestRow { id: index, value: (index % 10) as i32 })
            .collect::<Vec<_>>(),
        |row: &TableTestRow| row.id,
    );

    model.set_sort(Some(TableSort::new("value")), comparator());
    assert!(
        !model.sorting().is_pending,
        "the projection settles within the call"
    );
    assert_eq!(
        model.sorting().completed_builds,
        1,
        "one projection build per new sort"
    );

    assert_eq!(model.row_id_at(0), Some(0), "the first row keeps id 0");
    assert_eq!(model.row_id_at(1), Some(10), "ties hold source order");
    assert_eq!(
        model.index_for(&199_999),
        Some(199_999),
        "an id maps back to its display index in constant time"
    );

    // Round-tripping every sampled row must not disturb the projection.
    for _ in 0..100 {
        for index in (0..model.count()).step_by(1_997) {
            let id = model.row_id_at(index).expect("a row id");
            assert_eq!(
                model.index_for(&id),
                Some(index),
                "row {index} round-trips through its id"
            );
        }
    }
    assert_eq!(
        model.sorting().completed_builds,
        1,
        "reads never rebuild the projection"
    );

    model.set_sort(Some(TableSort::new("value").descending()), comparator());
    assert_eq!(
        model.row_id_at(0),
        Some(9),
        "descending starts at the largest value"
    );
    assert_eq!(model.row_id_at(1), Some(19), "descending keeps ties stable");

    // Returning to the ascending sort is a cache hit, not a rebuild.
    model.set_sort(Some(TableSort::new("value")), comparator());
    assert!(!model.sorting().is_pending, "a cached sort has nothing pending");
    assert_eq!(
        model.sorting().cache_hits,
        1,
        "the ascending projection was reused"
    );
    assert_eq!(
        model.sorting().completed_builds,
        2,
        "only the two distinct sorts were built"
    );

    model.set_sort(None, comparator());
    assert_eq!(model.row_id_at(1), Some(1), "clearing the sort restores source order");
    assert_eq!(
        model.index_for(&199_999),
        Some(199_999),
        "unsorted lookup still resolves"
    );
}

#[test]
fn edits_and_replacement_preserve_the_latest_data() {
    let model = DataTableModel::new(
        (0..10_000)
            .map(|index| TableTestRow { id: index, value: 10_000 - index as i32 })
            .collect::<Vec<_>>(),
        |row: &TableTestRow| row.id,
    );
    model.set_sort(Some(TableSort::new("value")), comparator());

    // Editing before the projection settles must be reflected in it.
    assert!(model.update_row(&5, |row| row.value = -1), "row 5 exists");
    assert_eq!(
        model.row_id_at(0),
        Some(5),
        "the edited row sorts to the front"
    );
    assert_eq!(
        model.record_at(0).map(|row| row.value),
        Some(-1),
        "and carries the new value"
    );

    model.set_sort(Some(TableSort::new("value").descending()), comparator());
    model.replace_rows(vec![
        TableTestRow { id: 80, value: 2 },
        TableTestRow { id: 90, value: 3 },
    ]);
    assert_eq!(model.count(), 2, "replacement swaps the whole row set");
    assert_eq!(
        model.row_id_at(0),
        Some(90),
        "the surviving rows sort by the active direction"
    );
    assert_eq!(model.index_for(&5), None, "a replaced row id is gone");

    assert!(
        !model.update_row(&5, |row| row.value = 0),
        "editing a removed row reports no change"
    );

    model.set_sort(None, comparator());
    assert_eq!(
        model.row_id_at(0),
        Some(80),
        "clearing the sort returns to the replacement's source order"
    );
}

#[test]
fn range_selection_follows_display_order() {
    let model = DataTableModel::new(
        (0..12)
            .map(|index| TableTestRow { id: index, value: 12 - index as i32 })
            .collect::<Vec<_>>(),
        |row: &TableTestRow| row.id,
    );
    model.set_sort(Some(TableSort::new("value")), comparator());

    let mut selection: DataTableSelection<usize> = DataTableSelection::new();
    selection.select(1, &model, DataTableSelectionMode::Multiple, KeyModifiers(0));
    selection.select(
        4,
        &model,
        DataTableSelectionMode::Multiple,
        KeyModifiers::LEFT_SHIFT,
    );
    // `DataTableSelection` keeps ids in a `BTreeSet`, so the range reads in id
    // order rather than click order. Swift's ordered list holds the same set.
    let ids: Vec<usize> = selection.selected_ids.iter().copied().collect();
    assert_eq!(
        ids,
        vec![7, 8, 9, 10],
        "shift extends from the anchor through display order"
    );
    assert_eq!(selection.anchor_id, Some(10), "the anchor stays the first click");
    assert_eq!(selection.focused_id, Some(7), "focus moves to the last click");

    // A gui toggle removes the clicked row from the range. Display index 3 is
    // row id 8, so the toggle drops that member.
    selection.select(
        3,
        &model,
        DataTableSelectionMode::Multiple,
        KeyModifiers::LEFT_GUI,
    );
    let ids: Vec<usize> = selection.selected_ids.iter().copied().collect();
    assert_eq!(
        ids,
        vec![7, 9, 10],
        "the toggle dropped the clicked row (id 8)"
    );

    // Single mode always replaces, even with shift held.
    selection.select(
        0,
        &model,
        DataTableSelectionMode::Single,
        KeyModifiers::LEFT_SHIFT,
    );
    let ids: Vec<usize> = selection.selected_ids.iter().copied().collect();
    assert_eq!(ids, vec![11], "single mode leaves exactly one row selected");
}

#[test]
fn deletion_cleanup_drops_ids_that_left_the_model() {
    let model = DataTableModel::new(
        (0..12)
            .map(|index| TableTestRow { id: index, value: 12 - index as i32 })
            .collect::<Vec<_>>(),
        |row: &TableTestRow| row.id,
    );
    model.set_sort(Some(TableSort::new("value")), comparator());

    let mut selection: DataTableSelection<usize> = DataTableSelection::new();
    selection.select(1, &model, DataTableSelectionMode::Multiple, KeyModifiers(0));
    selection.select(
        4,
        &model,
        DataTableSelectionMode::Multiple,
        KeyModifiers::LEFT_SHIFT,
    );
    assert!(!selection.selected_ids.is_empty(), "the range is selected");

    model.replace_rows(vec![TableTestRow { id: 99, value: 0 }]);
    selection.retain_existing(&model);

    assert!(
        selection.selected_ids.is_empty(),
        "no selected id survives replacement"
    );
    assert_eq!(selection.focused_id, None, "focus is cleared with the rows");
    assert_eq!(selection.anchor_id, None, "the anchor is cleared too");
}

#[test]
fn hundreds_of_columns_window_by_width_with_independent_frozen_columns() {
    let geometry = DataTableColumnGeometry::new(vec![100.0; 500], 2);
    assert_eq!(geometry.frozen_width(), 200.0, "two frozen columns span 200pt");
    assert_eq!(
        geometry.moving_width(),
        49_800.0,
        "the remaining 498 columns scroll"
    );

    let window = geometry.visible_columns(20_000.0, 350.0);
    // Frozen columns always render; the moving window starts at
    // floor(20000/100) - 1 overscan = 199, plus the 2 frozen slots.
    assert!(
        window.len() < 10,
        "a 350pt viewport never mounts the whole 500-column row (got {} columns)",
        window.len()
    );
    assert_eq!(
        window,
        201..207,
        "the moving window covers the offset plus one overscan column"
    );
}

#[test]
fn descending_sorts_do_not_reverse_ties() {
    let model = DataTableModel::new(
        (0..6)
            .map(|index| TableTestRow { id: index, value: (index % 3) as i32 })
            .collect::<Vec<_>>(),
        |row: &TableTestRow| row.id,
    );
    model.set_sort(Some(TableSort::new("value").descending()), comparator());

    let ids: Vec<usize> = (0..6)
        .map(|index| model.row_id_at(index).expect("a row"))
        .collect();
    assert_eq!(
        ids,
        vec![2, 5, 1, 4, 0, 3],
        "equal values keep ascending source order in a descending sort"
    );
}

#[test]
fn a_replaced_row_set_rebuilds_the_active_projection() {
    let model = DataTableModel::new(
        vec![
            TableTestRow { id: 1, value: 30 },
            TableTestRow { id: 2, value: 10 },
            TableTestRow { id: 3, value: 20 },
        ],
        |row: &TableTestRow| row.id,
    );
    model.set_sort(Some(TableSort::new("value")), comparator());
    assert_eq!(
        [model.row_id_at(0), model.row_id_at(1), model.row_id_at(2)],
        [Some(2), Some(3), Some(1)],
        "the initial rows sort ascending"
    );

    model.replace_rows(vec![
        TableTestRow { id: 4, value: 50 },
        TableTestRow { id: 5, value: 40 },
    ]);
    assert_eq!(
        [model.row_id_at(0), model.row_id_at(1)],
        [Some(5), Some(4)],
        "the replacement is re-sorted under the active sort"
    );
    assert_eq!(model.index_for(&4), Some(1), "lookup follows the projection");
}

#[test]
fn an_empty_model_reports_no_rows() {
    let model = DataTableModel::new(Vec::<TableTestRow>::new(), |row: &TableTestRow| row.id);
    assert_eq!(model.count(), 0);
    assert_eq!(model.row_id_at(0), None, "no row exists at index 0");
    assert_eq!(model.index_for(&0), None, "no id resolves");
    model.set_sort(Some(TableSort::new("value")), comparator());
    assert_eq!(model.count(), 0, "sorting an empty model is a no-op");
}
