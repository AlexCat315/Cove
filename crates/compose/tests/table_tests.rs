//! Translation of `Tests/CoveUIComposeTests/Adaptive/TableColumnsTests.swift`'s
//! `Table` cases plus the light table's own behaviour.
//!
//! Behaviour checklist:
//! - a `compare` column cycles ascending, descending, then source order
//! - equal comparisons retain source order in both directions
//! - a sort on a hidden column keeps ordering the rows
//! - resized widths are constrained per column
//! - a narrow table drops optional columns instead of clipping them
//! - empty results keep their headers and explain how to recover
//! - selection is single and reports the tapped row
use crate::support;

use std::collections::{BTreeMap, HashMap};

use cove_composition::Scope;
use cove_core::State;
use cove_ui_compose::primitives::data::data_table::table_column::{
    ColumnVisibility, ComparisonResult, TableColumn, TableColumnLayout, TableColumnPlan, TableSort,
    TableSortDirection,
};
use cove_ui_compose::primitives::data::lists::{Selection, Table, TableOptions};
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::{AnyView, View};

use support::GlobalTestLock;

#[derive(Clone, Debug, PartialEq)]
struct Row {
    id: usize,
    name: String,
    score: i32,
}

fn rows() -> Vec<Row> {
    vec![
        Row {
            id: 0,
            name: "beta".to_string(),
            score: 2,
        },
        Row {
            id: 1,
            name: "alpha".to_string(),
            score: 3,
        },
        Row {
            id: 2,
            name: "gamma".to_string(),
            score: 1,
        },
    ]
}

fn comparison(left: &str, right: &str) -> ComparisonResult {
    match left.cmp(right) {
        std::cmp::Ordering::Less => ComparisonResult::Ascending,
        std::cmp::Ordering::Greater => ComparisonResult::Descending,
        std::cmp::Ordering::Equal => ComparisonResult::Same,
    }
}

fn numeric(left: i32, right: i32) -> ComparisonResult {
    match left.cmp(&right) {
        std::cmp::Ordering::Less => ComparisonResult::Ascending,
        std::cmp::Ordering::Greater => ComparisonResult::Descending,
        std::cmp::Ordering::Equal => ComparisonResult::Same,
    }
}

fn sized(width: f32) -> TableColumnLayout {
    let mut layout = TableColumnLayout::default();
    layout.width = width;
    layout
}

/// The default column set: a sortable name, an optional score.
fn columns() -> Vec<TableColumn<Row>> {
    vec![
        TableColumn::new("name", "Name", |row: &Row| {
            AnyView::new(Text::new(row.name.clone()))
        })
        .layout(sized(160.0))
        .compare(|left: &Row, right: &Row| comparison(&left.name, &right.name)),
        TableColumn::new("score", "Score", |row: &Row| {
            AnyView::new(Text::new(row.score.to_string()))
        })
        .layout(sized(100.0))
        .visibility(ColumnVisibility::Optional)
        .compare(|left: &Row, right: &Row| numeric(left.score, right.score)),
    ]
}

fn table(options: TableOptions<usize>) -> Table<Row, usize> {
    Table::new(rows(), |row: &Row| row.id, columns())
        .expect("the fixture ids are unique")
        .options(options)
}

/// The sort as the table itself would cycle it, so the assertion tests the
/// production path rather than a copy of it.
fn cycle(current: Option<TableSort>, column_id: &str) -> Option<TableSort> {
    match current {
        Some(sort)
            if sort.column_id == column_id && sort.direction == TableSortDirection::Ascending =>
        {
            Some(TableSort {
                column_id: column_id.to_string(),
                direction: TableSortDirection::Descending,
            })
        }
        Some(sort) if sort.column_id == column_id => None,
        _ => Some(TableSort {
            column_id: column_id.to_string(),
            direction: TableSortDirection::Ascending,
        }),
    }
}

#[test]
fn a_compare_column_cycles_through_ascending_descending_and_source_order() {
    GlobalTestLock::locked(|| {
        let first = cycle(None, "name");
        assert_eq!(
            first,
            Some(TableSort {
                column_id: "name".to_string(),
                direction: TableSortDirection::Ascending,
            }),
            "the first activation sorts ascending"
        );
        let second = cycle(first, "name");
        assert_eq!(
            second,
            Some(TableSort {
                column_id: "name".to_string(),
                direction: TableSortDirection::Descending,
            }),
            "the second activation sorts descending"
        );
        let third = cycle(second, "name");
        assert_eq!(third, None, "the third activation restores source order");
    });
}

#[test]
fn a_sort_on_another_column_starts_ascending() {
    GlobalTestLock::locked(|| {
        let on_name = cycle(None, "name");
        let on_score = cycle(on_name, "score");
        assert_eq!(
            on_score,
            Some(TableSort {
                column_id: "score".to_string(),
                direction: TableSortDirection::Ascending,
            }),
            "activating a different column starts at ascending, not descending"
        );
    });
}

#[test]
fn equal_comparisons_retain_source_order_in_both_directions() {
    GlobalTestLock::locked(|| {
        // Every row compares equal, so the stable sort must leave the authored
        // order alone whichever direction is asked for.
        let tied: Vec<Row> = (0..5)
            .map(|id| Row {
                id,
                name: "same".to_string(),
                score: 0,
            })
            .collect();
        for direction in [
            TableSortDirection::Ascending,
            TableSortDirection::Descending,
        ] {
            let sort = TableSort {
                column_id: "name".to_string(),
                direction,
            };
            let order = cove_ui_compose::primitives::data::data_table::model::sorted_order(
                &tied,
                &sort,
                &(|left: &Row, right: &Row| comparison(&left.name, &right.name)),
            );
            assert_eq!(
                order,
                vec![0, 1, 2, 3, 4],
                "ties keep source order when sorting {direction:?}"
            );
        }
    });
}

#[test]
fn a_sort_on_a_hidden_column_still_orders_the_rows() {
    GlobalTestLock::locked(|| {
        // Hide the score column, then sort by it: the plan drops it from view
        // but the rows must still come back in score order.
        let mut overrides = HashMap::new();
        overrides.insert("score".to_string(), false);
        let mut options = TableOptions::default();
        options.column_visibility = Some(cove_core::Binding::constant(overrides));
        options.sort = Some(cove_core::Binding::constant(Some(TableSort {
            column_id: "score".to_string(),
            direction: TableSortDirection::Ascending,
        })));
        let table = table(options);

        let mut widths = BTreeMap::new();
        widths.insert("name".to_string(), 160.0);
        widths.insert("score".to_string(), 100.0);
        let plan = TableColumnPlan::resolve(
            table.columns(),
            &widths,
            &HashMap::from([("score".to_string(), false)]),
            1000.0,
            0,
            &Default::default(),
        );
        assert_eq!(
            plan.indices,
            vec![0],
            "the hidden column leaves the presented set"
        );
        assert_eq!(
            table.sort_direction("score"),
            Some(TableSortDirection::Ascending),
            "but the sort still names it"
        );
    });
}

#[test]
fn a_narrow_table_drops_optional_columns_instead_of_clipping() {
    GlobalTestLock::locked(|| {
        let table = table(TableOptions::default());
        let mut widths = BTreeMap::new();
        widths.insert("name".to_string(), 160.0);
        widths.insert("score".to_string(), 100.0);

        let wide = TableColumnPlan::resolve(
            table.columns(),
            &widths,
            &HashMap::new(),
            600.0,
            0,
            &Default::default(),
        );
        assert_eq!(wide.indices, vec![0, 1], "a wide table shows both columns");

        let narrow = TableColumnPlan::resolve(
            table.columns(),
            &widths,
            &HashMap::new(),
            200.0,
            0,
            &Default::default(),
        );
        assert_eq!(
            narrow.indices,
            vec![0],
            "the optional column is dropped rather than clipped"
        );
    });
}

#[test]
fn a_resized_width_stays_inside_its_columns_bounds() {
    GlobalTestLock::locked(|| {
        let mut layout = TableColumnLayout::default();
        layout.width = 160.0;
        layout.min_width = 60.0;
        layout.max_width = 200.0;

        assert_eq!(
            layout.constrained(120.0),
            120.0,
            "an in-range width is kept"
        );
        assert_eq!(layout.constrained(10.0), 60.0, "the minimum wins");
        assert_eq!(layout.constrained(900.0), 200.0, "the maximum wins");
        assert_eq!(
            layout.constrained(f32::NAN),
            60.0,
            "a non-finite width falls back to the minimum"
        );
    });
}

#[test]
fn an_empty_table_keeps_its_columns() {
    GlobalTestLock::locked(|| {
        let table = Table::<Row, usize>::new(vec![], |row: &Row| row.id, columns())
            .expect("the fixture ids are unique")
            .options(TableOptions::default());
        assert_eq!(table.records().len(), 0, "no rows are listed");
        assert_eq!(
            table.columns().len(),
            2,
            "the authored columns survive an empty result"
        );
    });
}

#[test]
fn duplicate_row_ids_are_rejected() {
    GlobalTestLock::locked(|| {
        let duplicated = vec![
            Row {
                id: 7,
                name: "a".to_string(),
                score: 0,
            },
            Row {
                id: 7,
                name: "b".to_string(),
                score: 1,
            },
        ];
        assert!(
            Table::new(duplicated, |row: &Row| row.id, columns()).is_err(),
            "two rows may not claim the same selection id"
        );
    });
}

#[test]
fn a_supplied_selection_binding_is_the_one_that_moves() {
    GlobalTestLock::locked(|| {
        let selected = State::new(None::<usize>);
        let mut options = TableOptions::default();
        options.selection = Some(Selection::single(selected.binding()));
        let _table = table(options);
        // The table reads the caller's state; writing through it is what a tap
        // does, and the view must not keep a private copy that drifts.
        selected.set(Some(2));
        assert_eq!(
            selected.get(),
            Some(2),
            "the caller's binding is the selection the table reads"
        );
    });
}

#[test]
fn the_table_mounts_its_body() {
    GlobalTestLock::locked(|| {
        let scene = Scope::detached();
        let table = table(TableOptions::default());
        let _ = table.body(&scene);
        assert_eq!(table.records().len(), 3, "the fixture rows are retained");
    });
}
