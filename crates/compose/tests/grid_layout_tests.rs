//! Translation of `Tests/CoveUIComposeTests/GridLayoutTests.swift`.
//!
//! Behaviour checklist:
//! - fixed and flexible tracks share remaining width and preserve minimums
//! - a flexible maximum redistributes unused width to another track
//! - adaptive tracks account for gaps and the cell maximum
//! - intrinsic proposals use content widths within each track's bounds
//! - large adaptive proposals repeat tracks without a per-column allocation
//! - rows use the tallest child and preserve authored child dimensions
//! - RTL mirrors cells and the leading alignment within each cell
//! - center alignment uses the full measured row height
//! - an adaptive grid changes its column count when its width changes
//!
//! Name mapping: `GridTrackResolver.widths(columns:availableWidth:spacing:)` is
//! `GridTrackResolver::widths(&columns, available_width, spacing, &[])`, and
//! `Grid(columns:columnSpacing:rowSpacing:)` is `Grid::try_new(..)`.

use crate::support;

use std::collections::HashMap;

use cove_composition::layout::LayoutDirectionExt;
use cove_composition::{Alignment, AnyView, LayoutDebugExt, TupleView};
use cove_core::scene::{Align, Direction, FlexDirection, Justify, Node, Wrap};
use cove_ui_compose::BoxView;
use cove_ui_compose::modifiers::layout::{GridExt, LayoutFrameExt};
use cove_ui_compose::primitives::layout::grid::{Grid, GridColumn, GridTrackResolver};

use support::GlobalTestLock;

fn widths(columns: &[GridColumn], available: Option<f32>, spacing: f32) -> Vec<f32> {
    GridTrackResolver::widths(columns, available, spacing, &[])
        .iter()
        .collect()
}

#[test]
fn fixed_and_flexible_tracks() {
    assert_eq!(
        widths(
            &[
                GridColumn::Fixed(40.0),
                GridColumn::flexible(30.0, f32::INFINITY),
                GridColumn::flexible(50.0, f32::INFINITY),
            ],
            Some(260.0),
            10.0,
        ),
        vec![40.0, 90.0, 110.0],
        "the two flexible tracks share the remaining 200pt"
    );
    assert_eq!(
        widths(
            &[
                GridColumn::Fixed(80.0),
                GridColumn::flexible(60.0, f32::INFINITY)
            ],
            Some(100.0),
            10.0,
        ),
        vec![80.0, 60.0],
        "a minimum is preserved even when the available width is short"
    );
}

#[test]
fn bounded_flexible_tracks() {
    assert_eq!(
        widths(
            &[
                GridColumn::flexible(10.0, 30.0),
                GridColumn::flexible(10.0, f32::INFINITY),
            ],
            Some(200.0),
            10.0,
        ),
        vec![30.0, 160.0],
        "the bounded track saturates and the rest goes to the other"
    );
}

#[test]
fn adaptive_tracks() {
    let third = 250.0f32 / 3.0;
    assert_eq!(
        widths(
            &[GridColumn::adaptive(80.0, f32::INFINITY)],
            Some(270.0),
            10.0
        ),
        vec![third, third, third],
        "an adaptive column expands into three 250/3 cells"
    );
    assert_eq!(
        widths(&[GridColumn::adaptive(80.0, 90.0)], Some(300.0), 10.0),
        vec![90.0, 90.0, 90.0],
        "the cell maximum caps each adaptive column"
    );
    assert_eq!(
        widths(
            &[GridColumn::adaptive(80.0, f32::INFINITY)],
            Some(60.0),
            10.0
        ),
        vec![80.0],
        "a width narrower than the minimum still yields one minimum-width cell"
    );
}

#[test]
fn intrinsic_tracks() {
    let columns = [
        GridColumn::Fixed(40.0),
        GridColumn::flexible(20.0, 70.0),
        GridColumn::adaptive(30.0, f32::INFINITY),
    ];
    let tracks = GridTrackResolver::widths(&columns, None, 5.0, &[90.0, 100.0, 50.0]);
    assert_eq!(
        tracks.iter().collect::<Vec<f32>>(),
        vec![40.0, 70.0, 50.0],
        "each track clamps its intrinsic width to its own bounds"
    );
}

#[test]
fn compact_adaptive_tracks() {
    let tracks = GridTrackResolver::widths(
        &[GridColumn::adaptive(1.0, f32::INFINITY)],
        Some(1_000_000_000.0),
        0.0,
        &[],
    );
    assert_eq!(tracks.count, 1_000_000_000, "one cell per pixel of width");
    assert_eq!(tracks.get(999_999_999), 1.0, "the last cell is addressable");
    assert_eq!(
        tracks.total_width, 1_000_000_000.0,
        "the total width matches the available width"
    );
}

fn cell(name: &str, width: Option<f32>, height: Option<f32>) -> AnyView {
    AnyView::new(
        AnyView::new(BoxView::new(
            FlexDirection::Column,
            Align::Stretch,
            Justify::FlexStart,
            Wrap::NoWrap,
            0.0,
            AnyView::new(cove_composition::EmptyView),
        ))
        .frame(
            width.map(cove_ui_compose::FrameDimension::Points),
            height.map(cove_ui_compose::FrameDimension::Points),
            None,
            None,
            None,
            None,
        )
        .debug_name(name),
    )
}

fn cells(root: &Node) -> HashMap<String, Node> {
    let mut result = HashMap::new();
    fn walk(node: &Node, into: &mut HashMap<String, Node>) {
        if let Some(name) = support::debug_name(node)
            && name.starts_with("cell-")
        {
            into.insert(name, node.clone());
        }
        for child in node.children() {
            walk(&child, into);
        }
    }
    walk(root, &mut result);
    result
}

fn grid_node(
    columns: Vec<GridColumn>,
    column_spacing: f32,
    row_spacing: f32,
    alignment: Alignment,
    content: Vec<AnyView>,
) -> Grid {
    Grid::try_new(columns, AnyView::new(TupleView::new(content)))
        .expect("valid grid columns")
        .spacing(column_spacing, row_spacing)
        .alignment(alignment)
}

#[test]
fn grid_frames() {
    GlobalTestLock::locked(|| {
        let graph = support::graph();
        let grid = grid_node(
            vec![
                GridColumn::Fixed(60.0),
                GridColumn::flexible(20.0, f32::INFINITY),
            ],
            10.0,
            7.0,
            Alignment::TopLeading,
            vec![
                cell("cell-0", Some(40.0), Some(20.0)),
                cell("cell-1", Some(30.0), Some(40.0)),
                cell("cell-2", Some(50.0), Some(10.0)),
            ],
        );
        graph.install_view(
            AnyView::new(AnyView::new(grid).frame_width(200.0)),
            Default::default(),
        );
        graph.compute_layout(200.0, 200.0);
        let root = graph.tree.borrow().root().expect("root");
        let found = cells(&root);
        assert_eq!(found.len(), 3, "all three cells mount");

        let frame = |name: &str| found[name].frame();
        assert_eq!(
            (
                frame("cell-0").x,
                frame("cell-0").y,
                frame("cell-0").width,
                frame("cell-0").height
            ),
            (0.0, 0.0, 40.0, 20.0),
            "the first cell keeps its authored size at the origin"
        );
        assert_eq!(
            (
                frame("cell-1").x,
                frame("cell-1").y,
                frame("cell-1").width,
                frame("cell-1").height
            ),
            (70.0, 0.0, 30.0, 40.0),
            "the second cell starts after the 60pt track plus the 10pt gap"
        );
        assert_eq!(
            (
                frame("cell-2").x,
                frame("cell-2").y,
                frame("cell-2").width,
                frame("cell-2").height
            ),
            (0.0, 47.0, 50.0, 10.0),
            "the second row starts below the tallest child plus the 7pt row gap"
        );
    });
}

#[test]
fn right_to_left() {
    GlobalTestLock::locked(|| {
        let graph = support::graph();
        let grid = grid_node(
            vec![GridColumn::Fixed(60.0), GridColumn::Fixed(80.0)],
            10.0,
            0.0,
            Alignment::TopLeading,
            vec![
                cell("cell-0", Some(40.0), Some(20.0)),
                cell("cell-1", Some(30.0), Some(20.0)),
            ],
        );
        graph.install_view(
            AnyView::new(
                AnyView::new(grid)
                    .layout_direction(Direction::Rtl)
                    .frame_width(150.0),
            ),
            Default::default(),
        );
        graph.compute_layout(150.0, 100.0);
        let root = graph.tree.borrow().root().expect("root");
        let found = cells(&root);
        assert_eq!(
            found["cell-0"].frame().x,
            110.0,
            "RTL mirrors the first cell to the right edge of its 60pt track"
        );
        assert_eq!(
            found["cell-1"].frame().x,
            50.0,
            "RTL mirrors the second cell into the second track"
        );
    });
}

#[test]
fn centered_cells() {
    GlobalTestLock::locked(|| {
        let graph = support::graph();
        let grid = grid_node(
            vec![GridColumn::Fixed(80.0), GridColumn::Fixed(80.0)],
            0.0,
            0.0,
            Alignment::Center,
            vec![
                cell("cell-0", Some(20.0), Some(20.0)),
                cell("cell-1", Some(40.0), Some(60.0)),
            ],
        );
        graph.install_view(
            AnyView::new(AnyView::new(grid).frame_width(160.0)),
            Default::default(),
        );
        graph.compute_layout(160.0, 100.0);
        let root = graph.tree.borrow().root().expect("root");
        let found = cells(&root);
        let first = found["cell-0"].frame();
        let second = found["cell-1"].frame();
        assert_eq!(
            (first.x, first.y, first.width, first.height),
            (30.0, 20.0, 20.0, 20.0),
            "a centered cell sits in the middle of its 80pt track and the 60pt row"
        );
        assert_eq!(
            (second.x, second.y, second.width, second.height),
            (100.0, 0.0, 40.0, 60.0),
            "the tall cell fills the row and centers in its own track"
        );
    });
}

#[test]
fn adaptive_reflow() {
    GlobalTestLock::locked(|| {
        let graph = support::graph();
        let grid = grid_node(
            vec![GridColumn::adaptive(70.0, f32::INFINITY)],
            10.0,
            5.0,
            Alignment::TopLeading,
            (0..5)
                .map(|index| {
                    AnyView::new(
                        AnyView::new(
                            AnyView::new(BoxView::new(
                                FlexDirection::Column,
                                Align::Stretch,
                                Justify::FlexStart,
                                Wrap::NoWrap,
                                0.0,
                                AnyView::new(cove_composition::EmptyView),
                            ))
                            .frame_height(20.0),
                        )
                        .debug_name(&format!("cell-{index}")),
                    )
                })
                .collect(),
        );
        graph.install_view(AnyView::new(grid), Default::default());

        graph.compute_layout(230.0, 200.0);
        let root = graph.tree.borrow().root().expect("root");
        assert_eq!(
            cells(&root)["cell-3"].frame().y,
            25.0,
            "at 230pt the grid fits three columns, so cell 3 starts row 1"
        );

        graph.compute_layout(150.0, 200.0);
        let root = graph.tree.borrow().root().expect("root");
        let found = cells(&root);
        assert_eq!(
            found["cell-2"].frame().y,
            25.0,
            "at 150pt only two columns fit, so cell 2 starts row 1"
        );
        assert_eq!(found["cell-4"].frame().y, 50.0, "cell 4 lands in row 2");
    });
}

#[test]
fn table_grid_reads_the_authored_span() {
    GlobalTestLock::locked(|| {
        let graph = support::graph();
        let grid = grid_node(
            vec![
                GridColumn::Fixed(60.0),
                GridColumn::Fixed(60.0),
                GridColumn::Fixed(60.0),
            ],
            10.0,
            0.0,
            Alignment::TopLeading,
            vec![
                AnyView::new(cell("cell-0", Some(40.0), Some(20.0)).grid_span(2, 1)),
                cell("cell-1", Some(30.0), Some(20.0)),
                cell("cell-2", Some(20.0), Some(10.0)),
            ],
        );
        graph.install_view(
            AnyView::new(AnyView::new(grid).frame_width(200.0)),
            Default::default(),
        );
        graph.compute_layout(200.0, 200.0);
        let root = graph.tree.borrow().root().expect("root");
        let found = cells(&root);
        assert_eq!(
            (found["cell-0"].frame().x, found["cell-0"].frame().width),
            (0.0, 40.0),
            "the spanning cell keeps its authored width in the first track"
        );
        assert_eq!(
            found["cell-1"].frame().x,
            140.0,
            "the next cell starts in the third track, proving the span covered two"
        );
        assert_eq!(
            found["cell-2"].frame().y,
            20.0,
            "the third cell wraps to the row below the tallest spanned content"
        );
    });
}
