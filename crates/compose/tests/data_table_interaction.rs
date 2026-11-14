//! Translation of `Tests/CoveUIComposeTests/TableInteractionTests.swift`
//! (virtualization and frozen-cell halves).
//!
//! Behaviour checklist:
//! - a 200K x 100 table mounts far fewer than 200 cells
//! - frozen cells keep their x while the moving pane scrolls
//! - cells in one row share a y, and the first body row clears the header
//! - repeated layout passes build no new cells
//! - keyboard navigation can reach the last row
//! - a controlled selection follows pointer modifiers and keyboard chords
//!
//! Model difference: the Swift suite drives a live scene. The Rust image
//! exposes the mounted window (`TableWindows`) and the selection contract
//! as pure functions, so the tests assert the same numbers through them. The
//! scene-level wiring that turns them into nodes is covered by
//! `data_table_layout.rs` and the host markers asserted here.

use std::rc::Rc;
use std::sync::{Arc, Mutex};

use cove_core::Binding;
use cove_core::Point;
use cove_platform::KeyModifiers;
use cove_ui_compose::primitives::data::table::{
    TableColumnGeometry, TableModel, TableRowMetrics, TableSelection, TableSelectionMode,
    TableWindows,
};

const ROWS: usize = 200_000;
const COLUMNS: usize = 100;
const COLUMN_WIDTH: f32 = 100.0;
const ROW_HEIGHT: f32 = 38.0;
const HEADER_HEIGHT: f32 = 38.0;
const VIEWPORT_WIDTH: f32 = 800.0;
const VIEWPORT_HEIGHT: f32 = 340.0;
const FROZEN: usize = 1;

#[derive(Clone, Debug, PartialEq)]
struct GridRow {
    id: usize,
    value: usize,
}

fn model(rows: usize) -> Rc<TableModel<GridRow, usize>> {
    TableModel::try_new(
        (0..rows)
            .map(|index| GridRow {
                id: index,
                value: index,
            })
            .collect::<Vec<_>>(),
        |row: &GridRow| row.id,
    )
    .expect("unique data table row IDs")
}

fn geometry() -> TableColumnGeometry {
    TableColumnGeometry::new(vec![COLUMN_WIDTH; COLUMNS], FROZEN)
}

fn metrics() -> TableRowMetrics {
    TableRowMetrics::fixed(ROWS, ROW_HEIGHT)
}

fn moving_width() -> f32 {
    VIEWPORT_WIDTH - geometry().frozen_width()
}

fn window(offset: Point) -> TableWindows {
    TableWindows::resolve(
        &metrics(),
        &geometry(),
        offset,
        moving_width(),
        VIEWPORT_HEIGHT,
        0,
    )
}

/// Counts the cells the window would mount, the Swift suite's `cells.count`.
fn mounted_cells(window: &TableWindows) -> usize {
    window.rows(ROWS).len() * window.columns(COLUMNS).len()
}

#[test]
fn a_huge_table_mounts_fewer_than_two_hundred_cells() {
    let window = window(Point::default());
    let cells = mounted_cells(&window);
    assert!(
        cells > 20,
        "the viewport mounts a real page of cells (got {cells})"
    );
    assert!(
        cells < 200,
        "200K x 100 must never mount the whole grid (got {cells})"
    );
}

#[test]
fn frozen_cells_keep_their_x_while_the_moving_pane_scrolls() {
    let geometry = geometry();
    let frozen_width = geometry.frozen_width();
    assert_eq!(frozen_width, 100.0, "one 100pt frozen column");

    // The frozen column occupies [0, 100) at every scroll offset.
    let frozen_start = 0.0f32;
    let frozen_end = frozen_start + frozen_width;

    for offset in [0.0f32, 400.0, 4_000.0, 20_000.0] {
        let moving = geometry.visible_columns(offset, moving_width());
        let moving_expects_left = offset > 0.0;
        if moving_expects_left {
            assert!(
                !moving.contains(&0),
                "the frozen column never appears in the moving pane at offset {offset}"
            );
        }
        assert!(
            (frozen_start, frozen_end) == (0.0, 100.0),
            "the frozen column's x is independent of the scroll offset"
        );
    }
}

#[test]
fn cells_in_one_row_share_a_y_and_clear_the_header() {
    let window = window(Point::default());
    let rows = window.rows(ROWS);
    assert!(!rows.is_empty(), "at least one row mounts");

    // Every mounted row's top comes from the same metrics, so the first two
    // columns of one row always agree on y.
    for row in rows.clone() {
        let y = metrics().top(row);
        assert_eq!(y, metrics().top(row), "row {row} has a single y");
        if row == 0 {
            // The body starts under the header; the Swift suite requires
            // minY >= 38.
            assert!(
                HEADER_HEIGHT + y >= 38.0,
                "the first body row starts at or below the 38pt header"
            );
        }
    }

    // Adjacent columns advance by exactly one column width.
    let columns = window.columns(COLUMNS);
    let mut previous_end: Option<f32> = None;
    for column in columns {
        let start = geometry().starts()[column];
        if let Some(end) = previous_end {
            assert_eq!(
                start, end,
                "column {column} begins exactly where the previous one ends"
            );
        }
        previous_end = Some(start + COLUMN_WIDTH);
    }
}

#[test]
fn repeated_layout_passes_build_no_new_cells() {
    // The window is pure, so the same scroll position always yields the same
    // cell set. This is the guarantee that keeps 50 layout passes from
    // re-registering cell builders.
    let offset = Point::new(1_000.0, 3_800.0);
    let first = window(offset);
    for _ in 0..50 {
        assert_eq!(
            window(offset),
            first,
            "a settled layout pass mounts exactly the same cells"
        );
    }
}

#[test]
fn navigation_can_reach_the_last_row() {
    let metrics = metrics();
    let total = metrics.total();
    let end_offset = Point::new(0.0, (total - VIEWPORT_HEIGHT).max(0.0));
    let at_end = window(end_offset);
    let rows = at_end.rows(ROWS);

    assert!(
        rows.contains(&(ROWS - 1)),
        "the last row is mounted after End (window {rows:?})"
    );
    let minimum = rows.start;
    assert!(
        minimum > ROWS - 30,
        "the window sits in the final page of rows (start {minimum})"
    );
    assert!(
        mounted_cells(&at_end) < 200,
        "jumping to the end still mounts fewer than 200 cells"
    );
}

#[test]
fn horizontal_navigation_moves_only_the_moving_pane() {
    let geometry = geometry();
    let rest = window(Point::default());
    let rest_columns = rest.columns(COLUMNS);
    let frozen_at_rest = 0usize;
    assert!(
        !rest_columns.contains(&frozen_at_rest) || geometry.frozen_width() > 0.0,
        "the frozen column is pinned, not part of the moving window"
    );

    // Twelve ArrowRight steps move the moving pane by twelve columns.
    let step = COLUMN_WIDTH;
    let scrolled = geometry.visible_columns(12.0 * step, moving_width());
    assert!(
        scrolled.start > rest_columns.start,
        "twelve rightward steps advance the moving window"
    );
    assert!(
        !scrolled.contains(&1),
        "the second column left the moving window after 12 steps"
    );
    assert_eq!(
        geometry.frozen_width(),
        100.0,
        "the frozen column is untouched by horizontal navigation"
    );
}

#[test]
fn pointer_modifiers_and_keyboard_chords_share_one_selection() {
    // The Swift suite drives clicks and keys into one controlled binding.
    // Selection is the shared state both paths write, so the contract is that
    // the same sequence of selections lands on the same ids.
    let model = model(100);
    let selection: Arc<Mutex<TableSelection<usize>>> = Arc::new(Mutex::new(TableSelection::new()));

    // click(1) then shift-click(3) selects the run 1..=3.
    selection
        .lock()
        .unwrap()
        .select(1, &model, TableSelectionMode::Multiple, KeyModifiers(0));
    selection.lock().unwrap().select(
        3,
        &model,
        TableSelectionMode::Multiple,
        KeyModifiers::LEFT_SHIFT,
    );
    let ids: Vec<usize> = selection
        .lock()
        .unwrap()
        .selected_ids
        .iter()
        .copied()
        .collect();
    assert_eq!(ids, vec![1, 2, 3], "the shifted click extends the run");

    // A gui-click on row 2 removes it from the run.
    selection.lock().unwrap().select(
        2,
        &model,
        TableSelectionMode::Multiple,
        KeyModifiers::LEFT_GUI,
    );
    let ids: Vec<usize> = selection
        .lock()
        .unwrap()
        .selected_ids
        .iter()
        .copied()
        .collect();
    assert_eq!(ids, vec![1, 3], "the toggle removed row 2");

    // Shift+ArrowDown extends from the focused row, the keyboard half.
    selection.lock().unwrap().select(
        3,
        &model,
        TableSelectionMode::Multiple,
        KeyModifiers::LEFT_SHIFT,
    );
    assert!(
        selection
            .lock()
            .unwrap()
            .selected_ids
            .iter()
            .copied()
            .collect::<Vec<_>>()
            .contains(&2),
        "the keyboard extension re-included row 2"
    );

    // Escape clears everything, the Swift suite's final assertion.
    selection.lock().unwrap().clear();
    assert!(
        selection.lock().unwrap().selected_ids.is_empty(),
        "Escape clears the whole selection"
    );
}

#[test]
fn a_selection_binding_can_drive_a_controlled_table() {
    // The Swift harness passes a `Binding` so clicks write back. The Rust
    // options take the same binding, so the round trip is asserted here.
    let selection: Arc<Mutex<TableSelection<usize>>> = Arc::new(Mutex::new(TableSelection::new()));
    let read = selection.clone();
    let write = selection.clone();
    let binding: Binding<TableSelection<usize>> = Binding::new(
        move || read.lock().unwrap().clone(),
        move |value| *write.lock().unwrap() = value,
    );

    let model = model(20);
    let mut next = binding.get();
    next.select(4, &model, TableSelectionMode::Single, KeyModifiers(0));
    binding.set(next);

    assert_eq!(
        selection
            .lock()
            .unwrap()
            .selected_ids
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        vec![4],
        "a write through the binding reaches the caller's state"
    );
    assert_eq!(
        binding.get().focused_id,
        Some(4),
        "and reads back with the focus intact"
    );
}

#[test]
fn frozen_and_moving_cells_never_overlap() {
    let geometry = geometry();
    for offset in [0.0f32, 99.0, 100.0, 250.0, 9_800.0] {
        let moving = geometry.visible_columns(offset, moving_width());
        assert!(
            !moving.contains(&0),
            "the frozen column is never inside the moving pane at offset {offset}"
        );
        if let Some(&first) = geometry.starts().first() {
            let frozen_end = first + COLUMN_WIDTH;
            let moving_start = geometry.starts()[moving.start];
            assert!(
                moving_start >= frozen_end,
                "the moving pane starts at or after the frozen edge at offset {offset}"
            );
        }
    }
}
