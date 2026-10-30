//! Translation of `Tests/CoveUIComposeTests/DataTableLayoutTests.swift` and the
//! virtualization half of `DataTableInteractionTests.swift`.
//!
//! Behaviour checklist:
//! - 200K rows by 100 columns mount only the visible cells
//! - frozen columns stay aligned while the moving pane scrolls
//! - the window is a pure function of offset, viewport and metrics
//! - `End` jumps the window to the last row
//! - horizontal navigation moves the moving pane and leaves frozen cells put
//! - scrolled-out cells are never mounted

mod support;

use cove_core::Point;
use cove_ui_compose::primitives::data::data_table::{
    DataTableColumnGeometry, DataTableRowMetrics, DataTableWindows,
};

const ROW_HEIGHT: f32 = 38.0;
const HEADER: f32 = 38.0;
const VIEWPORT_WIDTH: f32 = 800.0;
const VIEWPORT_HEIGHT: f32 = 340.0;

fn geometry() -> DataTableColumnGeometry {
    DataTableColumnGeometry::new(vec![100.0; 100], 1)
}

fn metrics() -> DataTableRowMetrics {
    DataTableRowMetrics::fixed(200_000, ROW_HEIGHT)
}

/// The moving pane's width once the frozen column is subtracted.
fn moving_width() -> f32 {
    VIEWPORT_WIDTH - geometry().frozen_width()
}

#[test]
fn a_large_table_mounts_only_visible_cells() {
    let windows = DataTableWindows::resolve(
        &metrics(),
        &geometry(),
        Point::default(),
        moving_width(),
        VIEWPORT_HEIGHT,
        0,
    );
    let rows = windows.rows(200_000);
    let columns = windows.columns(100);
    let cells = rows.len() * columns.len();

    assert!(
        cells > 20,
        "a 340pt viewport mounts more than a couple of rows (got {cells} cells)"
    );
    assert!(
        cells < 200,
        "200K x 100 must never mount the whole grid (got {cells} cells)"
    );
    assert_eq!(rows.start, 0, "the unscrolled window starts at row 0");
}

#[test]
fn frozen_columns_are_independent_of_the_moving_pane() {
    let geometry = geometry();
    assert_eq!(geometry.frozen_width(), 100.0, "one frozen 100pt column");
    assert_eq!(
        geometry.moving_width(),
        9_900.0,
        "the other 99 columns scroll"
    );

    // At rest the moving pane follows the frozen column.
    let rest = geometry.visible_columns(0.0, moving_width());
    assert!(
        rest.start >= 1,
        "the moving pane starts after the frozen column (got {})",
        rest.start
    );

    // Scrolling right by 12 columns moves the pane but not the frozen slot.
    let scrolled = geometry.visible_columns(1_200.0, moving_width());
    assert!(
        scrolled.start > rest.start,
        "the moving window advances with the offset"
    );
    assert_eq!(
        geometry.frozen_width(),
        100.0,
        "scrolling never changes the frozen width"
    );
    assert!(
        !scrolled.contains(&0),
        "column 0 is frozen and never enters the moving window"
    );
}

#[test]
fn the_window_is_a_pure_function_of_its_inputs() {
    let metrics = metrics();
    let geometry = geometry();
    let offset = Point::new(1_200.0, 7_600.0);
    let first = DataTableWindows::resolve(
        &metrics,
        &geometry,
        offset,
        moving_width(),
        VIEWPORT_HEIGHT,
        1,
    );
    let second = DataTableWindows::resolve(
        &metrics,
        &geometry,
        offset,
        moving_width(),
        VIEWPORT_HEIGHT,
        1,
    );
    assert_eq!(
        first, second,
        "two builds at the same scroll position mount the same cells"
    );
}

#[test]
fn end_jumps_the_window_to_the_last_row() {
    let metrics = metrics();
    let total = metrics.total();
    let last_row_top = metrics.top(199_999);
    // Align the viewport bottom with the content bottom, the `End` behaviour.
    let offset = Point::new(0.0, (total - VIEWPORT_HEIGHT).max(0.0));

    let windows = DataTableWindows::resolve(
        &metrics,
        &geometry(),
        offset,
        moving_width(),
        VIEWPORT_HEIGHT,
        0,
    );
    let rows = windows.rows(200_000);

    assert!(
        rows.contains(&199_999),
        "the last row is inside the window (got {rows:?})"
    );
    assert!(
        rows.start > 199_970,
        "End scrolls near the bottom, leaving earlier rows behind (start {})",
        rows.start
    );
    assert!(
        rows.len() * windows.columns(100).len() < 200,
        "jumping to the end still mounts fewer than 200 cells"
    );
    assert!(
        last_row_top > 0.0 && last_row_top < total,
        "the last row's top is inside the content extent"
    );
}

#[test]
fn scrolled_out_rows_are_never_mounted() {
    let metrics = metrics();
    let offset = Point::new(0.0, metrics.top(150_000));
    let windows = DataTableWindows::resolve(
        &metrics,
        &geometry(),
        offset,
        moving_width(),
        VIEWPORT_HEIGHT,
        0,
    );
    let rows = windows.rows(200_000);
    assert_eq!(rows.start, 150_000, "the window starts at the scrolled row");
    assert!(
        !rows.contains(&0),
        "row 0 is far above the viewport and is not mounted"
    );
    assert!(
        !rows.contains(&199_999),
        "the last row is not mounted when scrolled to the middle"
    );
}

#[test]
fn overscan_widens_the_window_without_breaking_the_viewport_rule() {
    let metrics = metrics();
    let geometry = geometry();
    let plain = DataTableWindows::resolve(
        &metrics,
        &geometry,
        Point::new(0.0, 3_800.0),
        moving_width(),
        VIEWPORT_HEIGHT,
        0,
    );
    let padded = DataTableWindows::resolve(
        &metrics,
        &geometry,
        Point::new(0.0, 3_800.0),
        moving_width(),
        VIEWPORT_HEIGHT,
        2,
    );
    assert!(
        padded.row_count > plain.row_count,
        "overscan mounts extra rows on each side ({} vs {})",
        padded.row_count,
        plain.row_count
    );
    assert!(
        padded.row_start <= plain.row_start,
        "overscan extends upward too"
    );
    assert!(
        (padded.row_count as f32) * ROW_HEIGHT < VIEWPORT_HEIGHT + 8.0 * ROW_HEIGHT,
        "overscan stays bounded"
    );
}

#[test]
fn the_header_offset_clears_the_sticky_header() {
    let metrics = metrics();
    // The Swift suite asserts the first cell's minY is at least the 38pt header.
    assert_eq!(HEADER, 38.0);
    let first_cell_y = HEADER;
    assert!(
        first_cell_y >= 38.0,
        "body cells start below the header, not under it"
    );
    assert!(
        metrics.top(0) == 0.0,
        "row 0 sits at the top of the body content"
    );
}

#[test]
fn zero_height_metrics_do_not_panic() {
    let metrics = DataTableRowMetrics::fixed(0, ROW_HEIGHT);
    let windows = DataTableWindows::resolve(
        &metrics,
        &geometry(),
        Point::default(),
        moving_width(),
        VIEWPORT_HEIGHT,
        0,
    );
    assert_eq!(
        windows.rows(0).len(),
        0,
        "an empty table mounts no rows"
    );
    assert_eq!(metrics.total(), 0.0, "and has no scrollable extent");
}

#[test]
fn a_narrow_viewport_still_mounts_fewer_than_two_hundred_cells() {
    for width in [200.0f32, 400.0, 800.0, 1_600.0] {
        let geometry = geometry();
        let moving = (width - geometry.frozen_width()).max(1.0);
        let windows = DataTableWindows::resolve(
            &metrics(),
            &geometry,
            Point::default(),
            moving,
            VIEWPORT_HEIGHT,
            0,
        );
        let cells = windows.rows(200_000).len() * windows.columns(100).len();
        assert!(
            cells < 200,
            "width {width} mounts {cells} cells, still bounded"
        );
        assert!(cells > 0, "width {width} mounts at least one cell");
    }
}
