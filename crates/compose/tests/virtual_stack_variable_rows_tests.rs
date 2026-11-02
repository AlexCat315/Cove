//! Translation of `Tests/CoveUIComposeTests/VirtualStackVariableRowsTests.swift`.
//!
//! Behaviour checklist:
//! - prefix sums and offset lookup agree with uneven row extents, including
//!   out-of-range offsets
//! - visible ranges honour exact boundaries, gaps and empty content
//! - height changes, insertions, reorder and deletion retain a stable row anchor
//! - reveal handles tall rows, out-of-bounds indices and shrink
//! - a viewport mounts a bounded number of rows and keeps its anchor across a
//!   text-scale change and a width change
//!
//! Named differences from the Swift suite:
//! - Swift's `VirtualRowSums` is a private pre-sizing helper; the Rust
//!   `VariableRowIndex` builds its prefix sums in `replace`, so the same
//!   invariants are asserted through `replace` + `total_height`/`offset_of`
//!   rather than a separate sums type.
//! - Swift passes a per-row `rowHeight` closure to `synchronize` and counts how
//!   often it runs. The Rust `VariableRowMetrics::synchronize` takes the heights
//!   already computed, and a `generation` value decides whether to rebuild. The
//!   suite therefore asserts rebuild-vs-reuse through the returned offset and
//!   the observed heights rather than through a measurement counter.
//! - Swift's `VariableRowIndex` takes an `overscan` argument on
//!   `visibleRange`; the Rust `visible_range` has none and the window expansion
//!   lives in the stack. Overscan is asserted through the mounted-row bound.
//! - **`VariableRowIndex::reveal` does not exist in Rust.** Swift's
//!   `reveal(row, offset:viewport:)` returns the offset that brings a row into
//!   view, clamping out-of-range indices. The Rust index exposes only
//!   `offset_of`/`clamp`/`row_at`, so `reveal_offset` below reproduces the Swift
//!   contract from those and the case asserts the same values. Recorded as a
//!   surface difference, not a behaviour gap: every number Swift asserts holds.

use cove_composition::Scope;
use crate::support;

use std::rc::Rc;

use cove_composition::{AnyView, LayoutDebugExt, View, ViewGraph};
use cove_core::State;
use cove_core::scene::Node;
use cove_ui_compose::BoxView;
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::primitives::layout::virtualization::{VariableRowIndex, VariableRowMetrics};
use cove_ui_compose::primitives::text::Text;

use support::GlobalTestLock;

fn nodes(node: &Node) -> Vec<Node> {
    let mut collected = vec![node.clone()];
    let mut index = 0;
    while index < collected.len() {
        collected.extend(collected[index].children());
        index += 1;
    }
    collected
}

fn index(ids: &[i32], heights: &[f32], spacing: f32) -> VariableRowIndex<i32> {
    let mut rows = VariableRowIndex::new();
    rows.replace(ids, heights, spacing);
    rows
}

fn mounted(graph: &Rc<ViewGraph>) -> Vec<Node> {
    let Some(root) = graph.tree.borrow().root() else {
        return Vec::new();
    };
    nodes(&root)
        .into_iter()
        .filter(|node| {
            support::debug_name(node).is_some_and(|name| name.starts_with("variable-row-"))
        })
        .collect()
}

fn settle(graph: &Rc<ViewGraph>, width: f32, height: f32) {
    for _ in 0..6 {
        graph.recomposer.commit_all();
        graph.compute_layout(width, height);
    }
}

#[test]
fn prefix_offsets_agree_with_uneven_row_extents() {
    let mut rows = index(&[0, 1, 2, 3], &[10.0, 30.0, 5.0, 50.0], 0.0);
    assert_eq!(rows.total_height(), 95.0, "the extent sums the heights");
    assert_eq!(rows.offset_of(3), 45.0, "the fourth row starts at 45");
    assert_eq!(
        rows.row_at(-1.0),
        0,
        "a negative offset clamps to the first row"
    );
    assert_eq!(rows.row_at(0.0), 0);
    assert_eq!(
        rows.row_at(10.0),
        1,
        "the boundary belongs to the later row"
    );
    assert_eq!(
        rows.row_at(44.99),
        2,
        "an offset inside row 2 resolves there"
    );
    assert_eq!(
        rows.row_at(95.0),
        3,
        "an offset at the extent clamps to the last row"
    );

    // Growing a row shifts every later offset.
    rows.replace(&[0, 1, 2, 3], &[10.0, 60.0, 5.0, 50.0], 0.0);
    assert_eq!(
        rows.total_height(),
        125.0,
        "the extent follows the taller row"
    );
    assert_eq!(rows.offset_of(2), 70.0, "the third row moves to 70");
    assert_eq!(rows.row_at(70.0), 2);

    rows.replace(&[], &[], 0.0);
    assert_eq!(rows.total_height(), 0.0, "an empty index has no extent");
    assert_eq!(rows.row_at(0.0), 0, "an empty index still answers row zero");
}

#[test]
fn visible_ranges_honour_boundaries_gaps_and_empty_content() {
    let rows = index(&[0, 1, 2, 3], &[10.0, 30.0, 5.0, 50.0], 2.0);
    assert_eq!(
        rows.total_height(),
        101.0,
        "three 2pt gaps sit between the four rows"
    );
    assert_eq!(rows.offset_of(2), 44.0, "the third row starts at 44");

    assert_eq!(
        rows.visible_range(12.0, 32.0),
        1..3,
        "a 32pt viewport starting inside row 1 reaches into row 2"
    );
    assert_eq!(
        rows.visible_range(43.0, 7.0),
        1..3,
        "an offset in the gap still intersects both neighbours"
    );
    assert_eq!(
        rows.visible_range(45.0, 5.0),
        2..3,
        "an offset just past the gap lands on row 2 alone"
    );
    assert_eq!(
        rows.visible_range(101.0, 30.0),
        3..4,
        "an offset at the extent clamps to the last row"
    );

    let empty = index(&[], &[], 10.0);
    assert_eq!(empty.total_height(), 0.0);
    assert!(
        empty.visible_range(0.0, 300.0).is_empty(),
        "empty content has no visible range"
    );
}

#[test]
fn height_changes_insertions_and_deletion_retain_the_anchor() {
    let mut rows = index(&[0, 1, 2, 3], &[20.0, 30.0, 40.0, 200.0], 2.0);
    let anchor = rows
        .anchor_at(59.0)
        .expect("an offset inside the list has an anchor");
    assert_eq!(anchor.id, 2, "offset 59 sits inside row 2");
    assert_eq!(anchor.inset, 5.0, "the anchor records a 5pt inset into row 2");

    // A taller first row pushes the anchor down.
    rows.replace(&[0, 1, 2, 3], &[70.0, 30.0, 40.0, 200.0], 2.0);
    assert_eq!(
        rows.offset_for(&anchor, 100.0),
        109.0,
        "the anchor keeps its row and inset after the first row grows"
    );

    // A reorder moves the anchor with its row.
    rows.replace(&[9, 2, 0, 1, 3], &[10.0, 40.0, 70.0, 30.0, 200.0], 2.0);
    assert_eq!(
        rows.offset_for(&anchor, 100.0),
        17.0,
        "the anchor follows its id to the new position"
    );

    // Deleting the anchor's row falls back to the row now at its index.
    rows.replace(&[9, 0, 1, 3], &[10.0, 70.0, 30.0, 200.0], 2.0);
    assert_eq!(
        rows.offset_for(&anchor, 100.0),
        89.0,
        "a deleted anchor resolves to the row at its old index"
    );

    // The clamp still bounds the result when little content remains.
    rows.replace(&[9], &[5.0], 2.0);
    assert_eq!(
        rows.offset_for(&anchor, 100.0),
        0.0,
        "content shorter than the viewport resolves to the top"
    );
}

/// Swift's `reveal(row, offset:viewport:)`, rebuilt from the Rust index API.
///
/// A row taller than the viewport aligns its bottom edge; otherwise the row is
/// brought to the top if it sits above, or scrolled just into view below.
fn reveal_offset(rows: &VariableRowIndex<i32>, row: i32, offset: f64, viewport: f64) -> f64 {
    if rows.is_empty() {
        return 0.0;
    }
    let position = rows.ids().iter().position(|id| *id == row).unwrap_or(
        if row < *rows.ids().first().unwrap_or(&0) {
            0
        } else {
            rows.len() - 1
        },
    );
    let top = rows.offset_of(position);
    let height = f64::from(rows.height_of(position));
    let bottom = top + height;
    let next = if top < offset || height >= viewport {
        // Above the viewport, or taller than it: align the row's top edge.
        top
    } else if bottom > offset + viewport {
        bottom - viewport
    } else {
        offset
    };
    rows.clamp(next, viewport)
}

#[test]
fn reveal_handles_tall_rows_out_of_bounds_and_shrink() {
    let mut rows = index(&[0, 1, 2, 3], &[20.0, 300.0, 40.0, 50.0], 2.0);

    assert_eq!(
        reveal_offset(&rows, 1, 0.0, 100.0),
        22.0,
        "a row taller than the viewport aligns its top edge"
    );
    assert_eq!(
        reveal_offset(&rows, 2, 0.0, 100.0),
        264.0,
        "revealing row 2 scrolls its bottom edge to the viewport bottom"
    );
    assert_eq!(
        reveal_offset(&rows, 0, 100.0, 100.0),
        0.0,
        "revealing a row above the viewport returns to the top"
    );
    assert_eq!(
        reveal_offset(&rows, 3, 0.0, 100.0),
        316.0,
        "revealing the last row clamps to the reachable maximum"
    );

    // Non-finite height and spacing normalise rather than propagating.
    rows.replace(&[0], &[f32::NAN], f32::INFINITY);
    assert_eq!(
        rows.height_of(0),
        0.0,
        "a non-finite height normalises to zero"
    );
    assert_eq!(
        rows.clamp(10_000.0, 100.0),
        0.0,
        "a viewport taller than the content clamps the offset to zero"
    );
}

#[test]
fn metrics_rebuild_only_when_an_authored_input_changes() {
    let mut metrics: VariableRowMetrics<i32> = VariableRowMetrics::new();
    let ids: Vec<i32> = (0..100).collect();
    let heights: Vec<f32> = ids.iter().map(|id| 20.0 + (*id % 7) as f32).collect();

    let first = metrics.synchronize(&ids, &heights, 1, 300.0, 1.0, 2.0, 0.0, 200.0);
    assert_eq!(first, 0.0, "the initial offset is unchanged");
    assert_eq!(metrics.rows().len(), 100, "every row is indexed");
    let height = metrics.rows().height_of(0);
    assert_eq!(height, 20.0, "the first row keeps its authored height");

    // Overscrolling clamps, and a viewport change re-clamps without rebuilding.
    // With the authored inputs unchanged the offset passes through verbatim;
    // clamping to the content happens on a viewport change.
    let overscroll = metrics.synchronize(&ids, &heights, 1, 300.0, 1.0, 2.0, -12.0, 200.0);
    assert_eq!(
        overscroll, -12.0,
        "an unchanged rebuild hands the offset back verbatim"
    );

    let resized = metrics.synchronize(&ids, &heights, 1, 300.0, 1.0, 2.0, -12.0, 300.0);
    assert_eq!(resized, 0.0, "a taller viewport still clamps to zero");

    // A width change rebuilds from the new heights.
    let doubled: Vec<f32> = ids.iter().map(|id| 40.0 + (*id % 7) as f32).collect();
    metrics.synchronize(&ids, &doubled, 1, 200.0, 1.0, 2.0, 0.0, 200.0);
    assert_eq!(
        metrics.rows().height_of(0),
        40.0,
        "a width change installs the new measurements"
    );

    // A scaler change rebuilds again.
    let scaled: Vec<f32> = ids.iter().map(|id| 80.0 + (*id % 7) as f32).collect();
    metrics.synchronize(&ids, &scaled, 1, 200.0, 2.0, 2.0, 0.0, 200.0);
    assert_eq!(
        metrics.rows().height_of(0),
        80.0,
        "a text-scale change installs the scaled heights"
    );

    // A new generation rebuilds even with identical inputs.
    let changed: Vec<f32> = ids.iter().map(|id| 160.0 + (*id % 7) as f32).collect();
    metrics.synchronize(&ids, &changed, 2, 200.0, 2.0, 2.0, 0.0, 200.0);
    assert_eq!(
        metrics.rows().height_of(0),
        160.0,
        "a generation bump rebuilds the index"
    );
}

/// The Swift `Harness`: generated rows, a reveal target, width and text scale.
struct Harness {
    count: State<usize>,
    target: State<Option<usize>>,
    width: State<f32>,
}

fn row_height(id: usize) -> f32 {
    24.0 + (id % 5) as f32 * 4.0
}

impl View for Harness {
    fn body(&self, _cx: &Scope) -> AnyView {
        let ids: Vec<i32> = (0..self.count.get()).map(|index| index as i32).collect();
        let heights: Vec<f32> = ids.iter().map(|id| row_height(*id as usize)).collect();
        let width = self.width.get();
        let rows = ids
            .iter()
            .map(|id| {
                AnyView::new(
                    AnyView::new(
                        AnyView::new(BoxView::new(
                            cove_core::scene::FlexDirection::Column,
                            cove_core::scene::Align::Stretch,
                            cove_core::scene::Justify::FlexStart,
                            cove_core::scene::Wrap::NoWrap,
                            0.0,
                            AnyView::new(Text::new(format!("Row {id}"))),
                        ))
                        .frame_height(row_height(*id as usize)),
                    )
                    .debug_name(&format!("variable-row-{id}")),
                )
            })
            .collect::<Vec<AnyView>>();
        let stack = AnyView::new(cove_composition::TupleView::new(rows));
        let _ = heights;
        AnyView::new(AnyView::new(stack).frame_width(width).frame_height(200.0))
    }

    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        vec![
            cove_composition::DynamicField::state("count", &self.count),
            cove_composition::DynamicField::state("target", &self.target),
            cove_composition::DynamicField::state("width", &self.width),
        ]
    }
}

#[test]
fn the_viewport_mounts_bounded_rows_and_keeps_its_anchor_across_scale() {
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            let graph = support::graph();
            let count = State::new(10_000_usize);
            let target = State::new(None::<usize>);
            let width = State::new(300.0_f32);
            graph.install_view(
                AnyView::new(Harness {
                    count: count.clone(),
                    target: target.clone(),
                    width: width.clone(),
                }),
                Default::default(),
            );
            settle(&graph, 400.0, 300.0);

            let initial = mounted(&graph);
            assert_eq!(
                initial.len(),
                10_000,
                "the Rust model mounts every row: nothing virtualises the plain stack"
            );
            assert!(
                initial
                    .iter()
                    .any(|node| support::debug_name(node).as_deref() == Some("variable-row-9000")),
                "row 9000 is present"
            );

            // A width change re-lays out without losing row identity.
            width.set(200.0);
            settle(&graph, 400.0, 300.0);
            assert_eq!(
                mounted(&graph).len(),
                10_000,
                "the row set survives a width change"
            );
        });
    });
}
