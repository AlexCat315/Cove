//! Behaviour tests for `DataTableColumnOrder`.
//!
//! Translation of the ordering rules in
//! `Primitives/Data/DataTable/ColumnOrder.swift`. These are pure functions, so
//! they are cheap to pin exactly — and column ordering is the kind of thing that
//! silently drifts without a test.

use cove_ui_compose::primitives::data::data_table::{
    DataTableColumnGeometry, DataTableColumnOrder,
};

fn ids(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

#[test]
fn an_empty_order_applies_nothing() {
    assert_eq!(
        DataTableColumnOrder::apply(&[], &ids(&["a", "b", "c"])),
        ids(&["a", "b", "c"])
    );
}

#[test]
fn ordered_columns_lead_and_unordered_keep_authored_position() {
    // `c` was added after the order was saved, so it stays last rather than
    // forcing a migration of the saved order.
    assert_eq!(
        DataTableColumnOrder::apply(&ids(&["b", "a"]), &ids(&["a", "b", "c"])),
        ids(&["b", "a", "c"])
    );
}

#[test]
fn unknown_ids_in_the_order_are_ignored() {
    assert_eq!(
        DataTableColumnOrder::apply(&ids(&["ghost", "b", "a"]), &ids(&["a", "b"])),
        ids(&["b", "a"])
    );
}

#[test]
fn a_duplicated_saved_entry_keeps_the_first_rank() {
    assert_eq!(
        DataTableColumnOrder::apply(&ids(&["b", "a", "b"]), &ids(&["a", "b"])),
        ids(&["b", "a"])
    );
}

#[test]
fn moving_left_inserts_before_the_target() {
    assert_eq!(
        DataTableColumnOrder::move_id("c", "a", &ids(&["a", "b", "c"])),
        ids(&["c", "a", "b"])
    );
}

#[test]
fn moving_right_inserts_after_the_target() {
    assert_eq!(
        DataTableColumnOrder::move_id("a", "c", &ids(&["a", "b", "c"])),
        ids(&["b", "c", "a"])
    );
}

#[test]
fn moving_onto_itself_or_an_unknown_id_is_a_no_op() {
    let order = ids(&["a", "b", "c"]);
    assert_eq!(DataTableColumnOrder::move_id("b", "b", &order), order);
    assert_eq!(DataTableColumnOrder::move_id("ghost", "a", &order), order);
    assert_eq!(DataTableColumnOrder::move_id("a", "ghost", &order), order);
}

#[test]
fn target_reports_the_column_under_the_dragged_centre() {
    let geometry = DataTableColumnGeometry::new(vec![100.0, 100.0, 100.0], 0);
    // Column 0 centred at 50: no offset stays on 0.
    assert_eq!(DataTableColumnOrder::target(0, 0.0, &geometry), 0);
    // Dragging 60 to the right puts the centre at 110, inside column 1.
    assert_eq!(DataTableColumnOrder::target(0, 60.0, &geometry), 1);
    // Dragging a further 100 puts the centre at 210, inside column 2.
    assert_eq!(DataTableColumnOrder::target(0, 160.0, &geometry), 2);
    // A source outside the geometry is returned unchanged rather than panicking.
    assert_eq!(DataTableColumnOrder::target(9, 0.0, &geometry), 9);
}
