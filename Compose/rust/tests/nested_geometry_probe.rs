//! Regression: `Node::frame` is parent-relative (as in the Swift `LayoutPass`),
//! so `absolute_frame` must compose ancestor origins exactly once.

mod support;

use cove_composition::{AnyView, TupleView};
use cove_core::scene::{Align, FlexDirection, Justify, Node, Wrap};
use cove_ui_compose::BoxView;
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use support::GlobalTestLock;

fn boxed(width: f32, height: f32) -> AnyView {
    AnyView::new(
        BoxView::new(
            FlexDirection::Column,
            Align::Stretch,
            Justify::FlexStart,
            Wrap::NoWrap,
            0.0,
            AnyView::new(cove_composition::EmptyView),
        )
        .frame_width(width)
        .frame_height(height),
    )
}

fn collect(node: &Node, out: &mut Vec<Node>) {
    out.push(node.clone());
    for child in node.children() {
        collect(&child, out);
    }
}

#[test]
fn nested_absolute_frames_do_not_double_count_parent_origin() {
    GlobalTestLock::with_scheduler(|| {
        let graph = support::graph();
        // 300-wide row: a 100-wide spacer, then a 200-wide row holding one 40-wide leaf.
        let outer = AnyView::new(
            BoxView::new(
                FlexDirection::Row,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                boxed(40.0, 20.0),
            )
            .frame_width(200.0)
            .frame_height(100.0),
        );
        let root = AnyView::new(BoxView::new(
            FlexDirection::Row,
            Align::Stretch,
            Justify::FlexStart,
            Wrap::NoWrap,
            0.0,
            AnyView::new(TupleView::new(vec![boxed(100.0, 20.0), outer])),
        ));
        graph.install_view(root, Default::default());
        graph.compute_layout(300.0, 100.0);

        let root_node = graph.tree.borrow().root().expect("root node");
        let mut nodes = Vec::new();
        collect(&root_node, &mut nodes);

        // The leaf is the only node 40 wide.
        let leaf = nodes
            .iter()
            .find(|node| (node.frame().width - 40.0).abs() < 1.0)
            .expect("leaf node");
        assert!(
            (leaf.frame().x - 0.0).abs() < 1.0,
            "the leaf's frame is parent-relative, so it starts at x=0; got {}",
            leaf.frame().x
        );
        assert!(
            (leaf.absolute_frame().x - 100.0).abs() < 1.0,
            "the leaf sits after the 100pt spacer; absolute x should be 100, got {}",
            leaf.absolute_frame().x
        );

        // The outer row itself starts at 100.
        let outer_node = nodes
            .iter()
            .find(|node| (node.frame().width - 200.0).abs() < 1.0)
            .expect("outer node");
        assert!(
            (outer_node.absolute_frame().x - 100.0).abs() < 1.0,
            "the outer row starts after the spacer; got {}",
            outer_node.absolute_frame().x
        );
    });
}
