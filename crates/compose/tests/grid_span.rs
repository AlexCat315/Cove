//! Translation of `Tests/CoveUIComposeTests/GridSpanTests.swift`.
//!
//! Behaviour checklist:
//! - row-major placement skips occupied cells and wraps cells that do not fit
//! - a span wider than the column count clamps to the full row
//! - a multi-row cell grows the last row it spans by its deficit
//! - lazy grids pack spans per row and window columns of a wide grid

use cove_ui_compose::primitives::layout::grid::{
    GridRowHeights, GridSpan, GridSpanPlacement, LazyGrid, LazyGridRows,
};

#[test]
fn row_major_placement_skips_occupied_cells_and_wraps() {
    // Two-column cell, two-row cell, single cell, three-column cell, single cell
    // across a three-column grid. Swift expects the origins below.
    let plan = GridSpanPlacement::new(
        &[
            GridSpan::new(2, 1),
            GridSpan::new(1, 2),
            GridSpan::new(1, 1),
            GridSpan::new(3, 1),
            GridSpan::new(1, 1),
        ],
        3,
    );
    let origins: Vec<(usize, usize)> = plan
        .cells
        .iter()
        .map(|cell| (cell.row, cell.column))
        .collect();
    assert_eq!(
        origins,
        vec![(0, 0), (0, 2), (1, 0), (2, 0), (3, 0)],
        "the two-row cell pushes the third cell to the next row's first column"
    );
    assert_eq!(plan.row_count, 4, "four rows are occupied");

    // A span wider than the grid clamps to the full row instead of failing.
    let clamped = GridSpanPlacement::new(&[GridSpan::new(9, 1)], 2);
    assert_eq!(
        clamped.cells[0].span.columns, 2,
        "a 9-column span clamps to the 2 available columns"
    );
}

#[test]
fn multi_row_cells_grow_the_last_row_they_span() {
    let spans: Vec<GridSpan> = vec![
        GridSpan::new(2, 1),
        GridSpan::new(1, 2),
        GridSpan::new(1, 1),
        GridSpan::new(3, 1),
        GridSpan::new(1, 1),
    ];
    let plan = GridSpanPlacement::new(&spans, 3);
    let heights = plan.row_heights(&[20.0, 70.0, 20.0, 30.0, 10.0], 10.0);
    // Rows 0 and 1 initially measure 20 + 10(spacing) + 20 = 50; the 70pt
    // two-row cell needs 70, so the last spanned row grows to 40.
    assert_eq!(
        heights,
        vec![20.0, 40.0, 30.0, 10.0],
        "the deficit lands on the last row the cell spans"
    );
}

#[test]
fn lazy_grid_packs_spans_per_row() {
    let spans = [2usize, 1, 3, 1, 2, 5];
    let rows = LazyGridRows::new(6, 3, Some(&spans));
    assert_eq!(rows.count, 4, "six items with spans pack into four rows");
    assert_eq!(rows.items(0), 0..2, "row 0 holds items 0 and 1");
    assert_eq!(rows.items(1), 2..3, "item 2's 3-span starts a fresh row");
    assert_eq!(rows.items(2), 3..5, "items 3 and 4 share row 2");
    assert_eq!(rows.column_of_item(4), 1, "item 4 starts at column 1");
    assert_eq!(rows.span_of_item(5), 3, "a 5-span clamps to the 3 columns");
}

#[test]
fn uniform_lazy_grid_rows_need_no_span_table() {
    let uniform = LazyGridRows::new(7, 3, None);
    assert_eq!(
        uniform.count, 3,
        "seven items in three columns make three rows"
    );
    assert_eq!(uniform.items(2), 6..7, "the last row holds the remainder");
    assert_eq!(
        uniform.span_of_item(0),
        1,
        "unspanned items occupy one column"
    );
    assert_eq!(
        uniform.column_of_item(4),
        1,
        "column is index modulo columns"
    );
}

#[test]
fn visible_columns_window_a_wide_grid() {
    let window = LazyGrid::<i32>::visible_columns(100, 50.0, 10.0, 600.0, 240.0, 1);
    // stride 60: first = floor(600/60) - 1 = 9, last = ceil(840/60) + 1 = 15.
    assert_eq!(
        window,
        9..15,
        "the window covers the viewport plus one of overscan"
    );
}

#[test]
fn visible_columns_clamp_and_handle_degenerate_inputs() {
    assert_eq!(
        LazyGrid::<i32>::visible_columns(0, 50.0, 10.0, 0.0, 100.0, 0),
        0..0,
        "an empty grid has no visible columns"
    );
    assert_eq!(
        LazyGrid::<i32>::visible_columns(10, 0.0, 10.0, 0.0, 100.0, 0),
        0..0,
        "a zero column width has no visible columns"
    );
    assert_eq!(
        LazyGrid::<i32>::visible_columns(10, 50.0, 10.0, 0.0, 10_000.0, 0),
        0..10,
        "a viewport wider than the grid clamps at the column count"
    );
}

#[test]
fn row_heights_report_offsets_with_spacing() {
    let heights = GridRowHeights::variable(&[20.0, 40.0, 30.0]);
    assert!(heights.matches_count(3));
    assert_eq!(
        heights.height(1),
        40.0,
        "variable rows keep their authored height"
    );
    assert_eq!(
        heights.offset(2, 10.0),
        20.0 + 40.0 + 20.0,
        "offsets include the gaps"
    );
    let fixed = GridRowHeights::fixed(30.0);
    assert_eq!(fixed.height(5), 30.0, "fixed rows are uniform");
    assert_eq!(fixed.offset(3, 0.0), 90.0, "fixed offsets are arithmetic");
}

#[test]
fn row_heights_scale_with_the_text_scaler() {
    let heights = GridRowHeights::fixed(20.0).scaled(2.0);
    assert_eq!(
        heights.height(0),
        40.0,
        "the scale factor applies to every row"
    );
    assert_eq!(
        heights.offset(2, 0.0),
        80.0,
        "offsets scale with the heights"
    );
}
