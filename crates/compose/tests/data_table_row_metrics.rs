//! Behaviour tests for row geometry and the sort projection.
//!
//! Translation of `Primitives/Data/Table/{VariableRows,TableSortWorker}.swift`.
//! Stable sorting and prefix-sum row metrics are both silent-failure territory:
//! ties that reverse, or a row that reports the wrong top, are invisible without
//! an assertion.

use std::rc::Rc;

use cove_ui_compose::primitives::data::table::table_column::{
    ComparisonResult, TableSort, TableSortDirection,
};
use cove_ui_compose::primitives::data::table::{
    TableRowMetrics, TableSortRequest, accepts_projection,
};
use cove_ui_compose::primitives::layout::virtualization::VariableRowIndex;

#[test]
fn a_fixed_table_uses_arithmetic() {
    let metrics = TableRowMetrics::fixed(4, 30.0);
    assert_eq!(metrics.top(0), 0.0);
    assert_eq!(metrics.top(2), 60.0);
    assert_eq!(metrics.height(3), 30.0);
    assert_eq!(metrics.total(), 120.0);
}

#[test]
fn a_variable_table_uses_the_prefix_index() {
    let mut index = VariableRowIndex::new();
    index.replace(&[0, 1, 2], &[20.0, 50.0, 10.0], 0.0);
    let metrics = TableRowMetrics::variable(3, 20.0, index);
    assert_eq!(metrics.top(0), 0.0);
    assert_eq!(metrics.top(1), 20.0);
    assert_eq!(metrics.top(2), 70.0);
    assert_eq!(metrics.height(1), 50.0);
    assert_eq!(metrics.total(), 80.0);
}

#[test]
fn row_at_finds_the_containing_row_and_clamps() {
    let mut index = VariableRowIndex::new();
    index.replace(&[0, 1, 2], &[20.0, 50.0, 10.0], 0.0);
    let metrics = TableRowMetrics::variable(3, 20.0, index);
    assert_eq!(metrics.row_at(0.0), 0);
    assert_eq!(metrics.row_at(19.0), 0);
    assert_eq!(
        metrics.row_at(25.0),
        1,
        "offset 25 falls inside the 50pt row"
    );
    assert_eq!(
        metrics.row_at(500.0),
        2,
        "past the end clamps to the last row"
    );
}

#[test]
fn row_at_on_a_fixed_table_clamps_to_the_last_row() {
    let metrics = TableRowMetrics::fixed(3, 30.0);
    assert_eq!(metrics.row_at(0.0), 0);
    assert_eq!(metrics.row_at(35.0), 1);
    assert_eq!(metrics.row_at(1000.0), 2);
}

#[test]
fn an_empty_table_has_no_row() {
    assert_eq!(TableRowMetrics::fixed(0, 30.0).row_at(50.0), 0);
}

#[test]
fn visible_range_covers_the_viewport_with_overscan() {
    let metrics = TableRowMetrics::fixed(20, 30.0);
    // A 60pt viewport at offset 0 covers rows 0..2; overscan 1 adds one each side.
    let visible = metrics.visible_range(0.0, 60.0, 1);
    assert_eq!(visible.start, 0);
    assert_eq!(visible.end, 3);
}

#[test]
fn a_descending_sort_keeps_ties_in_source_order() {
    // Two records share a key; a descending sort must NOT reverse them.
    let keys = [1, 2, 2, 3];
    let request = TableSortRequest {
        records: keys.to_vec(),
        ids: (0..keys.len()).collect(),
        sort: TableSort {
            column_id: "k".into(),
            direction: TableSortDirection::Descending,
        },
        // Convention: `Ascending` means "left sorts before right", so a
        // larger left is `Descending`.
        compare: Rc::new(|left: &i32, right: &i32| {
            if left < right {
                ComparisonResult::Ascending
            } else if left > right {
                ComparisonResult::Descending
            } else {
                ComparisonResult::Same
            }
        }),
    };
    let projection = request.build();
    // Values 3, 2, 2, 1 with the tied 2s in their original relative order (1 then 2).
    assert_eq!(projection.order, vec![3, 1, 2, 0]);
}

#[test]
fn an_ascending_sort_orders_by_key() {
    let keys = [3, 1, 2];
    let request = TableSortRequest {
        records: keys.to_vec(),
        ids: (0..keys.len()).collect(),
        sort: TableSort {
            column_id: "k".into(),
            direction: TableSortDirection::Ascending,
        },
        // Convention: `Ascending` means "left sorts before right", so a
        // larger left is `Descending`.
        compare: Rc::new(|left: &i32, right: &i32| {
            if left < right {
                ComparisonResult::Ascending
            } else if left > right {
                ComparisonResult::Descending
            } else {
                ComparisonResult::Same
            }
        }),
    };
    let projection = request.build();
    assert_eq!(projection.order, vec![1, 2, 0]);
    // Positions map each id to its display slot.
    assert_eq!(projection.positions.get(&1), Some(&0));
    assert_eq!(projection.positions.get(&0), Some(&2));
}

#[test]
fn a_stale_projection_is_rejected() {
    let sort_a = TableSort {
        column_id: "a".into(),
        direction: TableSortDirection::Ascending,
    };
    let sort_b = TableSort {
        column_id: "b".into(),
        direction: TableSortDirection::Ascending,
    };
    assert!(
        accepts_projection(7, 7, &sort_a, &sort_a),
        "matching revision and sort"
    );
    assert!(
        !accepts_projection(6, 7, &sort_a, &sort_a),
        "older row revision"
    );
    assert!(
        !accepts_projection(7, 7, &sort_a, &sort_b),
        "superseded sort command"
    );
}
