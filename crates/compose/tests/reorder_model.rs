//! Behaviour tests for the reorder model.
//!
//! Translation of the drag arithmetic in
//! `Primitives/Data/Lists/Reorder/Reorderable.swift`. Reordering is index
//! arithmetic where an off-by-one silently moves the wrong row, so these pin the
//! boundary → destination conversion exactly.

use cove_ui_compose::primitives::data::lists::{ReorderableListModel, reorder_row_height};

#[test]
fn a_drag_within_one_stride_stays_on_its_own_row() {
    // Row stride 40; moving 19 points rounds to 0.
    assert_eq!(
        ReorderableListModel::drop_boundary(2, 19.0, 0.0, 40.0, 5),
        2
    );
    // Moving 21 points rounds to one row down.
    assert_eq!(
        ReorderableListModel::drop_boundary(2, 21.0, 0.0, 40.0, 5),
        3
    );
}

#[test]
fn auto_scroll_adds_to_pointer_travel() {
    // 20 points of travel plus 60 of auto-scroll is 80: two rows at stride 40.
    assert_eq!(
        ReorderableListModel::drop_boundary(0, 20.0, 60.0, 40.0, 6),
        2
    );
}

#[test]
fn the_boundary_is_clamped_to_the_list() {
    assert_eq!(
        ReorderableListModel::drop_boundary(0, -500.0, 0.0, 40.0, 5),
        0
    );
    assert_eq!(
        ReorderableListModel::drop_boundary(4, 500.0, 0.0, 40.0, 5),
        5
    );
}

#[test]
fn dropping_below_the_source_shifts_the_destination_back() {
    // Source 1 dropped at boundary 3: removing row 1 closes the gap, so the row
    // lands at index 2.
    assert_eq!(ReorderableListModel::destination(1, 3), Some(2));
    // Dropping above needs no shift.
    assert_eq!(ReorderableListModel::destination(3, 1), Some(1));
}

#[test]
fn dropping_on_its_own_position_reports_no_move() {
    assert_eq!(ReorderableListModel::destination(2, 2), None);
    // The boundary just past the row is still its own position.
    assert_eq!(ReorderableListModel::destination(2, 3), None);
}

/// The live projection while row 1 is lifted and the drop lands at 3: the lifted
/// slot travels there, rows 2 and 3 close up behind it, and the list is painted
/// exactly as the drop would order it.
#[test]
fn rows_between_the_source_and_the_drop_slide_up_behind_it() {
    let offset = |index: usize| ReorderableListModel::live_offset(index, 1, 3, 40.0);
    assert_eq!(
        (offset(0), offset(1), offset(2), offset(3), offset(4)),
        (0.0, 80.0, -40.0, -40.0, 0.0)
    );
    // Every slot stays occupied: no gap, no overlap.
    let painted = |index: usize| index as f32 * 40.0 + offset(index);
    assert_eq!(
        (painted(0), painted(1), painted(2), painted(3), painted(4)),
        (0.0, 120.0, 40.0, 80.0, 160.0)
    );
}

/// Symmetrically, pulling row 3 up to 1 carries it to slot 1 and pushes rows 1
/// and 2 down behind it.
#[test]
fn rows_between_the_drop_and_the_source_slide_down_behind_it() {
    let offset = |index: usize| ReorderableListModel::live_offset(index, 3, 1, 40.0);
    assert_eq!(
        (offset(0), offset(1), offset(2), offset(3), offset(4)),
        (0.0, 40.0, 40.0, -80.0, 0.0)
    );
    let painted = |index: usize| index as f32 * 40.0 + offset(index);
    assert_eq!(
        (painted(0), painted(1), painted(2), painted(3), painted(4)),
        (0.0, 80.0, 120.0, 40.0, 160.0)
    );
}

/// With no hole, no row moves — the projection collapses to the authored order.
#[test]
fn an_unmoved_drag_displaces_nothing() {
    let offset = |index: usize| ReorderableListModel::live_offset(index, 2, 2, 40.0);
    assert_eq!((offset(1), offset(2), offset(3)), (0.0, 0.0, 0.0));
}

#[test]
fn keyboard_stepping_clamps_rather_than_wrapping() {
    assert_eq!(ReorderableListModel::stepped(0, -1, 5), None);
    assert_eq!(ReorderableListModel::stepped(4, 1, 5), None);
    assert_eq!(ReorderableListModel::stepped(2, 1, 5), Some(3));
    assert_eq!(ReorderableListModel::stepped(2, -1, 5), Some(1));
    assert_eq!(ReorderableListModel::stepped(0, 9, 5), Some(4));
}

#[test]
fn an_empty_list_has_no_step() {
    assert_eq!(ReorderableListModel::stepped(0, 1, 0), None);
}

#[test]
fn mounted_rows_include_retained_offscreen_rows() {
    // The dragged row (0) and the last keyboard-moved row (9) stay mounted even
    // though the window only covers 4..10.
    let mounted = ReorderableListModel::mounted_indices(4..10, 20, &[0, 9]);
    assert_eq!(mounted, vec![4, 5, 6, 7, 8, 9, 0]);
}

#[test]
fn mounted_rows_are_clamped_to_the_list() {
    let mounted = ReorderableListModel::mounted_indices(18..30, 20, &[25]);
    assert_eq!(mounted, vec![18, 19]);
}

#[test]
fn handle_labels_name_the_row_when_a_title_exists() {
    assert_eq!(
        ReorderableListModel::handle_label(Some("Frame rate"), 1, 5),
        "Reorder Frame rate, position 2 of 5"
    );
    assert_eq!(
        ReorderableListModel::handle_label(None, 0, 5),
        "Reorder item 1 of 5"
    );
    assert_eq!(
        ReorderableListModel::handle_label(Some(""), 0, 5),
        "Reorder item 1 of 5",
        "an empty authored title falls back to the positional form"
    );
}

#[test]
fn a_touch_surface_raises_the_row_to_the_minimum_extent() {
    // Authored height scaled to 30, touch floor 44.
    assert_eq!(reorder_row_height(30.0, true, 44.0), 44.0);
    // Pointer input keeps the smaller row.
    assert_eq!(reorder_row_height(30.0, false, 44.0), 30.0);
    // A comfortably tall row is untouched on touch.
    assert_eq!(reorder_row_height(56.0, true, 44.0), 56.0);
}
