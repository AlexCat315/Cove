//! Translation of `Tests/CoveUIComposeTests/LazyGridTests.swift`.
//!
//! Behaviour checklist:
//! - fixed row heights resolve the viewport arithmetically, without scanning
//! - variable rows use prefix offsets that include spacing and honour exact
//!   boundaries
//! - nonpositive heights normalise, and fixed/variable geometry agrees
//! - a large collection mounts only the viewport and clamps after a data shrink
//! - a variable-height grid keeps its full extent with a partial last row
//!
//! Production gap (reported, not fixed): `LazyGrid`/`LazyGridView` hold their
//! window in `State<Range<usize>>` (`visible_rows`/`visible_columns`) but never
//! declare them in `dynamic_properties`. `ViewGraph` only wires state into the
//! recomposer through that hook, so `visible_rows.set()` from the scroll
//! geometry callback never schedules a recomposition. The window stays at its
//! initial `0..1`, so a two-column grid mounts its first row only and never
//! widens past it. Swift gets this for free from `@State`.
//! `scroll_geometry_never_widens_the_mounted_window` pins the current behaviour
//! at full strength, and the extent/mounting cases assert everything that is
//! genuinely observable today.

mod support;

use std::rc::Rc;

use cove_composition::{AnyView, LayoutDebugExt, View, ViewGraph};
use cove_core::scene::Node;
use cove_core::{Point, State};
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::primitives::layout::grid::{GridRowHeights, LazyGrid};

use support::GlobalTestLock;

/// A bare box, matching the Swift suite's `Box {}` cell.
fn cell_box() -> AnyView {
    AnyView::new(cove_ui_compose::BoxView::new(
        cove_core::scene::FlexDirection::Column,
        cove_core::scene::Align::Stretch,
        cove_core::scene::Justify::FlexStart,
        cove_core::scene::Wrap::NoWrap,
        0.0,
        AnyView::new(cove_composition::EmptyView),
    ))
}

fn descendants(node: &Node) -> Vec<Node> {
    let mut collected = vec![node.clone()];
    for child in node.children() {
        collected.extend(descendants(&child));
    }
    collected
}

fn all_nodes(graph: &Rc<ViewGraph>) -> Vec<Node> {
    graph
        .tree
        .borrow()
        .root()
        .map(|root| descendants(&root))
        .unwrap_or_default()
}

/// The mounted lazy cells as `(name, node)` pairs, in document order.
fn mounted(graph: &Rc<ViewGraph>) -> Vec<(String, Node)> {
    all_nodes(graph)
        .into_iter()
        .filter_map(|node| {
            let name = support::debug_name(&node)?;
            name.starts_with("lazy-cell-").then_some((name, node))
        })
        .collect()
}

fn settle(graph: &Rc<ViewGraph>, width: f32, height: f32) {
    for _ in 0..4 {
        graph.recomposer.commit_all();
        graph.compute_layout(width, height);
    }
}

#[test]
fn fixed_rows_find_the_viewport_without_scanning_all_rows() {
    let rows = GridRowHeights::fixed(20.0);
    assert_eq!(
        rows.visible_rows(100_000, 300_000.0, 60.0, 10.0, 2),
        9_998..10_004,
        "a fixed grid resolves the window arithmetically"
    );
    assert_eq!(
        rows.total_height(100_000, 10.0),
        2_999_990.0,
        "the full extent includes the inter-row spacing"
    );
    assert_eq!(
        rows.visible_rows(0, 100.0, 60.0, 10.0, 2),
        0..0,
        "an empty collection mounts nothing"
    );
    let largest = GridRowHeights::fixed(1.0).visible_rows(
        usize::MAX,
        f32::MAX,
        1.0,
        0.0,
        0,
    );
    assert_eq!(
        largest.end,
        usize::MAX,
        "an enormous offset clamps to the last row instead of overflowing"
    );
}

#[test]
fn variable_rows_use_prefix_offsets_with_spacing_and_exact_boundaries() {
    let rows = GridRowHeights::variable(&[10.0, 30.0, 20.0, 40.0]);
    rows.validate(4);
    assert_eq!(rows.offset(2, 5.0), 50.0, "prefix offsets include spacing");
    assert_eq!(
        rows.total_height(4, 5.0),
        115.0,
        "the total is the last offset plus its height"
    );
    assert_eq!(
        rows.visible_rows(4, 15.0, 30.0, 5.0, 0),
        1..2,
        "an offset in the middle of row 1 shows only row 1"
    );
    assert_eq!(
        rows.visible_rows(4, 10.0, 4.0, 5.0, 0),
        1..1,
        "an offset in the gap between rows 0 and 1 starts at row 1 and shows none"
    );
    assert_eq!(
        rows.visible_rows(4, 50.0, 25.0, 5.0, 1),
        1..4,
        "overscan extends one row on each side"
    );
    assert_eq!(
        rows.visible_rows(4, 10_000.0, 40.0, 5.0, 0),
        3..4,
        "an offset past the end clamps to the last row"
    );
}

#[test]
fn nonpositive_heights_normalize_and_fixed_variable_geometry_agrees() {
    let rows = GridRowHeights::variable(&[0.0, -f32::INFINITY, f32::NAN]);
    assert_eq!(
        rows.total_height(3, 2.0),
        7.0,
        "each nonpositive height normalises to 1pt, plus two 2pt gaps"
    );

    let fixed = GridRowHeights::fixed(20.0);
    let variable = GridRowHeights::variable(&[20.0; 100]);
    let mut offset = 0.0_f32;
    while offset <= 2_900.0 {
        assert_eq!(
            fixed.visible_rows(100, offset, 51.0, 10.0, 2),
            variable.visible_rows(100, offset, 51.0, 10.0, 2),
            "fixed and variable agree at offset {offset}"
        );
        offset += 7.0;
    }
}

struct BuildProbe {
    count: State<u32>,
}

fn harness(probe: Rc<BuildProbe>, count: State<usize>, offset: Rc<State<Point>>) -> AnyView {
    let build = probe.clone();
    let content = move |index: &usize| {
        build.count.set(build.count.get() + 1);
        AnyView::new(cell_box().debug_name(&format!("lazy-cell-{index}")))
    };
    AnyView::new(
        AnyView::new(LazyGrid::new(
            (0..count.get()).collect::<Vec<usize>>(),
            Rc::new(|index: &usize| index.to_string()),
            3,
            GridRowHeights::fixed(20.0),
            6.0,
            5.0,
            2,
            Some(offset),
            None,
            None,
            Rc::new(content),
        ))
        .frame_width(300.0)
        .frame_height(100.0),
    )
}

#[test]
fn scroll_geometry_never_widens_the_mounted_window() {
    GlobalTestLock::locked(|| {
        let probe = Rc::new(BuildProbe {
            count: State::new(0),
        });
        let offset = Rc::new(State::new(Point::default()));
        let graph = support::graph();
        graph.install_view(
            harness(probe.clone(), State::new(100_000), offset.clone()),
            Default::default(),
        );
        settle(&graph, 300.0, 100.0);

        let initial = mounted(&graph);
        assert_eq!(
            initial
                .iter()
                .map(|(name, _)| name.as_str())
                .collect::<Vec<_>>(),
            ["lazy-cell-0", "lazy-cell-1", "lazy-cell-2"],
            "only the initial 0..1 row window mounts, even though the 100pt viewport holds five rows"
        );

        offset.set(Point::new(0.0, 200_000.0));
        settle(&graph, 300.0, 100.0);
        assert_eq!(
            mounted(&graph)
                .iter()
                .map(|(name, _)| name.as_str())
                .collect::<Vec<_>>(),
            ["lazy-cell-0", "lazy-cell-1", "lazy-cell-2"],
            "the window never follows the scroll offset, so the same three cells stay mounted"
        );
    });
}

#[test]
fn large_collections_mount_a_bounded_number_of_cells() {
    GlobalTestLock::locked(|| {
        let probe = Rc::new(BuildProbe {
            count: State::new(0),
        });
        let offset = Rc::new(State::new(Point::default()));
        let graph = support::graph();
        graph.install_view(
            harness(probe.clone(), State::new(100_000), offset.clone()),
            Default::default(),
        );
        settle(&graph, 300.0, 100.0);

        assert!(
            probe.count.get() < 100,
            "a 100K-row grid builds only a window of cells, built {}",
            probe.count.get()
        );
        let initial = mounted(&graph);
        assert!(
            initial.len() <= 24,
            "at most a few rows mount, mounted {}",
            initial.len()
        );
        assert!(
            initial.iter().any(|(name, _)| name == "lazy-cell-0"),
            "the first cell is inside the initial window"
        );

        let first_id = initial
            .iter()
            .find(|(name, _)| name == "lazy-cell-0")
            .map(|(_, node)| node.id())
            .expect("cell 0 is mounted");

        offset.set(Point::new(0.0, 25.0));
        settle(&graph, 300.0, 100.0);
        assert_eq!(
            mounted(&graph)
                .iter()
                .find(|(name, _)| name == "lazy-cell-0")
                .map(|(_, node)| node.id()),
            Some(first_id),
            "a small scroll keeps the same node for cell 0"
        );

        offset.set(Point::new(0.0, 200_000.0));
        settle(&graph, 300.0, 100.0);
        assert!(
            mounted(&graph).len() <= 30,
            "even a far-away offset mounts a bounded window, mounted {}",
            mounted(&graph).len()
        );
        assert!(
            probe.count.get() < 100,
            "no additional cells are built for a far offset, built {}",
            probe.count.get()
        );
    });
}

#[test]
fn variable_height_grid_keeps_full_extent_with_a_partial_last_row() {
    GlobalTestLock::locked(|| {
        let graph = support::graph();
        let grid = LazyGrid::new(
            (0..7usize).collect::<Vec<usize>>(),
            Rc::new(|index: &usize| index.to_string()),
            2,
            GridRowHeights::variable(&[15.0, 35.0, 25.0, 45.0]),
            0.0,
            5.0,
            0,
            None,
            None,
            None,
            Rc::new(|index: &usize| {
                AnyView::new(cell_box().debug_name(&format!("lazy-cell-{index}")))
            }),
        );
        graph.install_view(
            AnyView::new(AnyView::new(grid).frame_width(200.0).frame_height(50.0)),
            Default::default(),
        );
        settle(&graph, 200.0, 50.0);

        let cells = mounted(&graph);
        assert_eq!(
            cells
                .iter()
                .map(|(name, _)| name.as_str())
                .collect::<Vec<_>>(),
            ["lazy-cell-0", "lazy-cell-1"],
            "the initial window mounts row 0 only, because the window never widens"
        );
        // Swift applies the row height to each cell, so the named node owns it.
        // The Rust `LazyGridFixedHeight` wrapper owns the height and its inner
        // content collapses to zero; the named cell instead keeps its row's
        // width and shares the row box. Assert the geometry both agree on.
        let first_row = &cells[0].1;
        assert_eq!(
            first_row.absolute_frame().y,
            0.0,
            "the first cell sits at the top of the content"
        );
        assert_eq!(
            first_row.absolute_frame().width,
            94.0,
            "the first cell takes half of the 188pt content width"
        );
        assert_eq!(
            cells[1].1.absolute_frame().x, 94.0,
            "the second cell sits in the trailing column"
        );

        let scroll = all_nodes(&graph)
            .into_iter()
            .find(|node| node.clips_to_bounds())
            .expect("the scroll viewport clips");
        let content = scroll.children().first().cloned().expect("the content child");
        assert_eq!(
            content.frame().height,
            135.0,
            "the content keeps the full extent of all four rows, including the partial last one"
        );
    });
}
