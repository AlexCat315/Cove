//! Translation of `Tests/CoveUIComposeTests/MasonryTests.swift`.
//!
//! Behaviour checklist:
//! - shortest-column placement, column spacing and item spacing drive the
//!   waterfall
//! - a width below the two-column budget collapses to one column
//! - RTL mirrors the columns while preserving shortest-column order
//! - `maximumColumns` caps the column count on a wide surface
//!
//! The Rust `Masonry` wrapper takes the same four parameters plus an explicit
//! alignment (Swift defaults to `TopLeading`), and its content is an erased
//! `AnyView` rather than a `@ViewBuilder` result, so the cells are built as a
//! `TupleView` here. No assertion is relaxed.

mod support;

use cove_composition::layout::LayoutDirectionExt;
use cove_composition::{AnyView, LayoutDebugExt, TupleView};
use cove_core::scene::{Align, Direction, FlexDirection, Justify, Node, Wrap};
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::primitives::layout::masonry::Masonry;
use cove_ui_compose::{Alignment, BoxView};

use support::GlobalTestLock;

fn cell(width: f32, height: f32, name: &str) -> AnyView {
    AnyView::new(
        AnyView::new(BoxView::new(
            FlexDirection::Column,
            Align::Stretch,
            Justify::FlexStart,
            Wrap::NoWrap,
            0.0,
            AnyView::new(cove_composition::EmptyView),
        ))
        .frame_size(width, height)
        .debug_name(name),
    )
}

/// A `Masonry` sized to `width`, the Swift `Masonry { ... }.frame(width:)` shape.
fn masonry(children: Vec<AnyView>, width: f32) -> AnyView {
    AnyView::new(
        AnyView::new(Masonry::new(
            100.0,
            2,
            10.0,
            8.0,
            Alignment::TopLeading,
            AnyView::new(TupleView::new(children)),
        ))
        .frame_width(width),
    )
}

fn cells(root: &Node) -> Vec<(String, Node)> {
    fn walk(node: &Node, into: &mut Vec<(String, Node)>) {
        if let Some(name) = support::debug_name(node)
            && name.starts_with("m-")
        {
            into.push((name, node.clone()));
        }
        for child in node.children() {
            walk(&child, into);
        }
    }
    let mut found = Vec::new();
    walk(root, &mut found);
    found
}

fn frame_of(root: &Node, name: &str) -> cove_core::Rect {
    cells(root)
        .into_iter()
        .find(|(candidate, _)| candidate == name)
        .map(|(_, node)| node.frame())
        .unwrap_or_else(|| panic!("cell {name} materialises"))
}

const WIDTH_330: f32 = 330.0;
const WIDTH_200: f32 = 200.0;
const WIDTH_1000: f32 = 1000.0;

#[test]
fn shortest_column_placement_drives_the_waterfall() {
    GlobalTestLock::locked(|| {
        let graph = support::graph();
        graph.install_view(
            masonry(
                vec![
                    cell(80.0, 40.0, "m-0"),
                    cell(80.0, 80.0, "m-1"),
                    cell(80.0, 60.0, "m-2"),
                    cell(80.0, 20.0, "m-3"),
                ],
                WIDTH_330,
            ),
            Default::default(),
        );
        graph.compute_layout(WIDTH_330, 300.0);
        let root = graph.tree.borrow().root().expect("root");

        assert_eq!(
            frame_of(&root, "m-0"),
            cove_core::Rect::new(0.0, 0.0, 80.0, 40.0),
            "the first cell lands in the first column"
        );
        assert_eq!(
            frame_of(&root, "m-1"),
            cove_core::Rect::new(170.0, 0.0, 80.0, 80.0),
            "the second cell takes the second column, one spacing past the first"
        );
        assert_eq!(
            frame_of(&root, "m-2"),
            cove_core::Rect::new(0.0, 48.0, 80.0, 60.0),
            "the third cell returns to the shortest column, one item spacing below"
        );
        assert_eq!(
            frame_of(&root, "m-3"),
            cove_core::Rect::new(170.0, 88.0, 80.0, 20.0),
            "the fourth cell stacks on the now-shorter second column"
        );
    });
}

#[test]
fn a_narrow_width_collapses_to_one_column() {
    GlobalTestLock::locked(|| {
        let graph = support::graph();
        let children = (0..4)
            .map(|index| cell(80.0, 40.0, &format!("m-{index}")))
            .collect();
        graph.install_view(
            masonry(children, WIDTH_200),
            Default::default(),
        );
        graph.compute_layout(WIDTH_200, 300.0);
        let root = graph.tree.borrow().root().expect("root");

        for index in 0..4usize {
            let expected = cove_core::Rect::new(0.0, index as f32 * 48.0, 80.0, 40.0);
            assert_eq!(
                frame_of(&root, &format!("m-{index}")),
                expected,
                "cell {index} stacks in the single column"
            );
        }
    });
}

#[test]
fn rtl_mirrors_the_columns() {
    GlobalTestLock::locked(|| {
        let graph = support::graph();
        graph.install_view(
            AnyView::new(
                masonry(
                    vec![cell(80.0, 40.0, "m-0"), cell(80.0, 80.0, "m-1")],
                    WIDTH_330,
                )
                .layout_direction(Direction::Rtl),
            ),
            Default::default(),
        );
        graph.compute_layout(WIDTH_330, 300.0);
        let root = graph.tree.borrow().root().expect("root");

        assert_eq!(
            frame_of(&root, "m-0"),
            cove_core::Rect::new(250.0, 0.0, 80.0, 40.0),
            "the first cell mirrors to the trailing edge"
        );
        assert_eq!(
            frame_of(&root, "m-1"),
            cove_core::Rect::new(80.0, 0.0, 80.0, 80.0),
            "the second cell keeps shortest-column order and mirrors with it"
        );
    });
}

#[test]
fn maximum_columns_caps_the_surface() {
    GlobalTestLock::locked(|| {
        let graph = support::graph();
        graph.install_view(
            masonry(
                vec![
                    cell(80.0, 40.0, "m-0"),
                    cell(80.0, 40.0, "m-1"),
                    cell(80.0, 40.0, "m-2"),
                ],
                WIDTH_1000,
            ),
            Default::default(),
        );
        graph.compute_layout(WIDTH_1000, 300.0);
        let root = graph.tree.borrow().root().expect("root");

        assert_eq!(frame_of(&root, "m-0").x, 0.0, "the first cell stays leading");
        assert_eq!(
            frame_of(&root, "m-1").x,
            505.0,
            "the second column sits one 495pt column plus the 10pt spacing in"
        );
        assert_eq!(
            frame_of(&root, "m-2").x, 0.0,
            "maximumColumns caps the layout at two, so the third cell wraps"
        );
        assert_eq!(frame_of(&root, "m-2").y, 48.0, "the wrapped cell stacks below");
    });
}
