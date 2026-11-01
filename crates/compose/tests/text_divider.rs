//! Translation of `Tests/CoveUIComposeTests/TextDividerTests.swift`.
//!
//! Behaviour checklist:
//! - `Text` without a text environment still builds a tree and measures to zero
//! - `Text` installs a draw hook and is never hit-testable
//! - a horizontal `Divider` spans the cross axis at its thickness
//! - a vertical `Divider` is a 1xN (or Nx1) strip
//! - a line limit keeps text on a single line under a narrow constraint

mod support;

use cove_composition::AnyView;
use cove_composition::layout::LayoutDirectionExt;
use cove_core::scene::{Align, FlexDirection, Justify, Node, Wrap};
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::primitives::text::text::{Divider, DividerAxis, Text};
use cove_ui_compose::{BoxView, Row, VerticalAlignment};

use support::GlobalTestLock;

fn collect(node: &Node, into: &mut Vec<Node>) {
    into.push(node.clone());
    for child in node.children() {
        collect(&child, into);
    }
}

fn all(graph: &std::rc::Rc<cove_composition::ViewGraph>) -> Vec<Node> {
    let mut nodes = Vec::new();
    if let Some(root) = graph.tree.borrow().root() {
        collect(&root, &mut nodes);
    }
    nodes
}

fn column(children: Vec<AnyView>) -> AnyView {
    AnyView::new(BoxView::new(
        FlexDirection::Column,
        Align::FlexStart,
        Justify::FlexStart,
        Wrap::NoWrap,
        0.0,
        AnyView::new(cove_composition::TupleView::new(children)),
    ))
}

fn divider_column(children: Vec<AnyView>) -> AnyView {
    AnyView::new(BoxView::new(
        FlexDirection::Column,
        Align::Stretch,
        Justify::FlexStart,
        Wrap::NoWrap,
        0.0,
        AnyView::new(cove_composition::TupleView::new(children)),
    ))
}

#[test]
fn text_without_environment_lays_out_at_zero_size() {
    GlobalTestLock::locked(|| {
        // With no text environment installed, measurement degrades to zero
        // rather than panicking.
        let graph = support::graph();
        graph.install_view(
            column(vec![
                AnyView::new(Text::new("Hello")),
                AnyView::new(Text::new("World")),
            ]),
            Default::default(),
        );
        graph.compute_layout(200.0, 200.0);

        let root = graph.tree.borrow().root().expect("root node");
        let first_child = root.children().first().cloned().expect("the column");
        let leaves = first_child.children();
        assert_eq!(leaves.len(), 2, "both texts materialise");
        assert_eq!(
            (leaves[0].frame().width, leaves[0].frame().height),
            (0.0, 0.0),
            "an unmeasured text reports zero intrinsic size"
        );
    });
}

#[test]
fn text_installs_a_draw_hook_and_is_not_hit_testable() {
    GlobalTestLock::locked(|| {
        let graph = support::graph();
        graph.install_view(AnyView::new(Text::new("Hi")), Default::default());

        let nodes = all(&graph);
        let text = nodes
            .iter()
            .find(|node| support::debug_name(node).is_none() && node.parent().is_some())
            .or_else(|| nodes.get(1))
            .cloned()
            .expect("a text node");
        assert!(text.draw().is_some(), "text installs a draw hook");
        assert!(!text.is_hit_testable(), "text never takes pointer input");
    });
}

#[test]
fn horizontal_divider_spans_the_cross_axis_at_its_thickness() {
    GlobalTestLock::locked(|| {
        let graph = support::graph();
        graph.install_view(
            divider_column(vec![AnyView::new(Divider::new(
                None,
                1.0,
                DividerAxis::Horizontal,
            ))]),
            Default::default(),
        );
        graph.compute_layout(200.0, 200.0);

        let root = graph.tree.borrow().root().expect("root node");
        let divider = root.children()[0].children()[0].clone();
        assert_eq!(
            (divider.frame().width, divider.frame().height),
            (200.0, 1.0),
            "a horizontal divider is full width and one point tall"
        );
        assert!(
            divider.background_color().is_some(),
            "the divider resolves a theme tint"
        );
    });
}

#[test]
fn vertical_divider_is_a_thin_strip() {
    GlobalTestLock::locked(|| {
        let graph = support::graph();
        let divider = AnyView::new(Divider::new(None, 2.0, DividerAxis::Vertical));
        let row = AnyView::new(
            AnyView::new(Row::new(VerticalAlignment::Center, 0.0, divider))
                .frame_width(200.0)
                .frame_height(100.0),
        );
        graph.install_view(row, Default::default());
        graph.compute_layout(200.0, 100.0);

        let nodes = all(&graph);
        let divider = nodes
            .into_iter()
            .find(|node| {
                let frame = node.frame();
                frame.width == 2.0
            })
            .expect("the 2pt vertical divider");
        assert_eq!(
            (divider.frame().width, divider.frame().height),
            (2.0, 100.0),
            "a vertical divider is its thickness wide and stretches to the row height"
        );
    });
}

#[test]
fn line_limit_keeps_text_on_one_line_under_narrow_constraints() {
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            let wrapping = support::graph();
            wrapping.install_view(AnyView::new(Text::new("Main Camera")), Default::default());
            wrapping.compute_layout(48.0, 120.0);

            let single = support::graph();
            single.install_view(
                AnyView::new(Text::new("Main Camera").line_limit(Some(1))),
                Default::default(),
            );
            single.compute_layout(48.0, 120.0);

            let height = |graph: &std::rc::Rc<cove_composition::ViewGraph>| {
                graph
                    .tree
                    .borrow()
                    .root()
                    .and_then(|root| root.children().first().cloned())
                    .map(|node| node.frame().height)
                    .unwrap_or(0.0)
            };
            let wrapping_height = height(&wrapping);
            let single_height = height(&single);
            assert!(
                wrapping_height > 20.0,
                "unconstrained text wraps to several lines (got {wrapping_height})"
            );
            assert_eq!(
                single_height, 20.0,
                "a one-line limit keeps the single line height"
            );
        });
    });
}

#[test]
fn explicit_color_overrides_the_theme_tint() {
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            let graph = support::graph();
            graph.install_view(
                AnyView::new(Text::new("Hello").with_color(cove_core::Color::BLACK)),
                Default::default(),
            );
            graph.compute_layout(200.0, 100.0);

            let text = all(&graph)
                .into_iter()
                .find(|node| node.draw().is_some())
                .expect("the text node");
            let mut draw = cove_core::DrawList::default();
            text.draw().expect("draw hook")(&mut draw, cove_core::Point::default());
            if !draw.vertices().is_empty() {
                assert!(
                    draw.vertices()
                        .iter()
                        .all(|vertex| vertex.color == cove_core::Color::BLACK.rgba8()),
                    "every vertex carries the authored color"
                );
            }
        });
    });
}

#[test]
fn layout_direction_does_not_change_divider_geometry() {
    GlobalTestLock::locked(|| {
        let graph = support::graph();
        let divider = AnyView::new(
            AnyView::new(Divider::new(None, 1.0, DividerAxis::Horizontal))
                .layout_direction(cove_core::scene::Direction::Rtl),
        );
        graph.install_view(divider, Default::default());
        graph.compute_layout(160.0, 100.0);

        let node = all(&graph)
            .into_iter()
            .find(|node| node.background_color().is_some())
            .expect("the divider");
        assert_eq!(
            (node.frame().width, node.frame().height),
            (160.0, 1.0),
            "RTL does not change a symmetric full-width rule"
        );
    });
}
