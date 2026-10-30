//! Behaviour tests for row/cell host update rules.
//!
//! Translation of `Primitives/Data/DataTable/{DataTableHosts,EditorPresence}.swift`.
//! Row striping precedence and the equality-gated update are both easy to get
//! wrong and invisible without assertions.

use cove_core::Color;
use cove_ui_compose::primitives::data::data_table::{
    drop_indicator_x, editor_blur_outcome, CellHostUpdate, DataTableColumnGeometry,
    EditorBlurOutcome, RowAccessibilityRole, RowHostUpdate,
};

fn accent() -> Color { Color { r: 0.2, g: 0.4, b: 1.0, a: 1.0 } }
fn surface() -> Color { Color { r: 0.5, g: 0.5, b: 0.5, a: 1.0 } }
fn background() -> Color { Color { r: 1.0, g: 1.0, b: 1.0, a: 1.0 } }

fn row(index: usize) -> RowHostUpdate {
    RowHostUpdate {
        index,
        is_selected: false,
        is_striped: true,
        is_enabled: true,
        is_frozen_lane: false,
    }
}

#[test]
fn a_selected_row_is_accent_regardless_of_striping() {
    let mut update = row(1); // odd, so striped
    assert!(update.is_striped_row());
    update.is_selected = true;
    assert_eq!(update.background(accent(), surface(), background()), accent());
}

#[test]
fn odd_rows_carry_the_stripe_and_even_rows_do_not() {
    assert!(row(1).is_striped_row());
    assert!(!row(2).is_striped_row(), "even rows are not striped");
    assert_eq!(
        row(1).background(accent(), surface(), background()),
        surface().multiplied_alpha(0.6)
    );
    assert_eq!(row(2).background(accent(), surface(), background()), background());
}

#[test]
fn striping_can_be_turned_off() {
    let mut update = row(1);
    update.is_striped = false;
    assert!(!update.is_striped_row());
    assert_eq!(update.background(accent(), surface(), background()), background());
}

#[test]
fn an_unchanged_row_update_is_skipped() {
    let previous = row(3);
    let same = row(3);
    assert!(!same.changed_from(Some(&previous)), "identical updates do not restyle");
    let mut changed = row(3);
    changed.is_selected = true;
    assert!(changed.changed_from(Some(&previous)));
    assert!(row(3).changed_from(None), "the first update always applies");
}

#[test]
fn a_frozen_lane_row_announces_as_a_group() {
    let mut update = row(0);
    update.is_frozen_lane = true;
    assert_eq!(update.accessibility_role(), RowAccessibilityRole::Group);
    update.is_frozen_lane = false;
    assert_eq!(update.accessibility_role(), RowAccessibilityRole::Row);
}

#[test]
fn the_row_label_is_one_based() {
    assert_eq!(row(0).accessibility_label(), "Row 1");
    assert_eq!(row(4).accessibility_label(), "Row 5");
}

#[test]
fn an_active_cell_draws_a_border() {
    let active = CellHostUpdate { title: "Frame rate", is_active: true, is_enabled: true };
    let idle = CellHostUpdate { title: "Frame rate", is_active: false, is_enabled: true };
    assert_eq!(active.border_width(), 1.0);
    assert_eq!(idle.border_width(), 0.0);
}

#[test]
fn only_an_enabled_cell_exposes_activate() {
    let enabled = CellHostUpdate { title: "x", is_active: false, is_enabled: true };
    let disabled = CellHostUpdate { title: "x", is_active: false, is_enabled: false };
    assert!(enabled.exposes_activate());
    assert!(!disabled.exposes_activate());
}

#[test]
fn an_unchanged_cell_update_is_skipped() {
    let previous = CellHostUpdate { title: "a", is_active: false, is_enabled: true };
    let same = CellHostUpdate { title: "a", is_active: false, is_enabled: true };
    assert!(!same.changed_from(Some(&previous)));
    let different = CellHostUpdate { title: "b", is_active: false, is_enabled: true };
    assert!(different.changed_from(Some(&previous)));
}

#[test]
fn a_blur_commits_only_when_the_editor_left_the_tree() {
    // The editor moving between panes is not a commit.
    assert_eq!(editor_blur_outcome(true), EditorBlurOutcome::KeepDraft);
    assert_eq!(editor_blur_outcome(false), EditorBlurOutcome::Commit);
}

#[test]
fn a_drop_indicator_in_the_frozen_prefix_ignores_scroll() {
    let geometry = DataTableColumnGeometry::new(vec![100.0, 100.0, 100.0], 1);
    // Boundary 50 sits inside the frozen 100pt prefix.
    assert_eq!(drop_indicator_x(50.0, &geometry, 30.0), 50.0);
    // Boundary 150 is in the scrolling region, so the offset applies.
    assert_eq!(drop_indicator_x(150.0, &geometry, 30.0), 120.0);
}
