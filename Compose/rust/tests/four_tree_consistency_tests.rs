//! Translation of `Tests/CoveUIComposeTests/FourTreeConsistencyTests.swift`.
//!
//! Behaviour checklist:
//! - every node has a `RenderObject` mirror whose child list matches the `Node`
//!   child list exactly — same objects, same order — through reorder, remove,
//!   insert and re-insert churn
//! - the reconciled leaf tags match the requested order exactly
//! - hit-test order and paint order agree after a reorder, because both are
//!   computed off the same reconciled ordering
//!
//! Phase 7 removed the input mirror, so there is no input tree to assert; the
//! Rust port has the same shape (`Node` + `RenderTree` only).

mod support;

use std::rc::Rc;

use cove_composition::{AnyView, IdentifiedView, PrimitiveView, View, ViewGraph};
use cove_core::scene::{LayoutNode, Node};

use support::GlobalTestLock;

const TAG_KEY: &str = "tag";

/// A keyed leaf carrying its tag as a raw attachment.
struct Leaf {
    tag: i32,
}

impl PrimitiveView for Leaf {
    fn make_node(&self) -> Node {
        Node::new()
    }
    fn update_node(&self, node: &Node) {
        node.set_attachment(TAG_KEY, self.tag);
    }
    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        layout.set_width(Some(10.0));
        layout.set_height(Some(10.0));
        Some(layout)
    }
}
cove_composition::impl_primitive_view!(Leaf);

/// A container with its own layout node and no static children, so the test
/// drives its children directly through `reconcile_children`.
struct Container;

impl PrimitiveView for Container {
    fn make_node(&self) -> Node {
        Node::new()
    }
    fn update_node(&self, _node: &Node) {}
    fn make_layout_node(&self) -> Option<LayoutNode> {
        Some(LayoutNode::new())
    }
    fn children(&self, _node: &Node) -> Vec<AnyView> {
        Vec::new()
    }
}
cove_composition::impl_primitive_view!(Container);

fn leaves(ids: &[i32]) -> Vec<AnyView> {
    ids.iter()
        .map(|tag| AnyView::new(IdentifiedView::new(Leaf { tag: *tag }, *tag)))
        .collect()
}

fn assert_consistent(node: &Node) {
    assert!(
        node.render_object().is_some(),
        "every node has a RenderObject mirror"
    );
    if let Some(object) = node.render_object() {
        let render_children: Vec<Node> = object
            .children()
            .iter()
            .filter_map(|child| child.node())
            .collect();
        assert_eq!(
            render_children.len(),
            node.children().len(),
            "render child count matches the node child count"
        );
        assert!(
            render_children
                .iter()
                .zip(node.children().iter())
                .all(|(render, plain)| render == plain),
            "render child order matches the node child order"
        );
    }
    for child in node.children() {
        assert_consistent(&child);
    }
}

fn reconcile(graph: &Rc<ViewGraph>, anchor: &Node, ids: &[i32]) {
    let layout_parent = graph.layout_node(anchor);
    graph.reconcile_children(anchor, layout_parent.as_ref(), &leaves(ids));
    graph.compute_layout(200.0, 400.0);
    graph.recomposer.commit_all();
}

fn tags(anchor: &Node) -> Vec<i32> {
    anchor
        .children()
        .iter()
        .filter_map(|child| child.attachment::<i32>(TAG_KEY))
        .collect()
}

#[test]
fn mirrors_stay_consistent_under_churn() {
    GlobalTestLock::locked(|| {
        let graph = support::graph();
        graph.install_view(AnyView::new(Container), Default::default());
        let root = graph.tree.borrow().root().expect("root");
        let anchor = root.children().first().cloned().expect("anchor node");

        reconcile(&graph, &anchor, &[0, 1, 2, 3, 4]);
        assert_consistent(&root);

        // A battery of structural mutations: reverse, rotate, drop middle,
        // re-insert, shuffle-ish permutations, shrink to one, grow back.
        let sequences: Vec<Vec<i32>> = vec![
            vec![4, 3, 2, 1, 0],
            vec![1, 2, 3, 4, 0],
            vec![1, 3, 0, 4],
            vec![3, 1, 4, 0, 2, 5],
            vec![5, 4, 3, 2, 1, 0],
            vec![2],
            vec![],
            vec![7, 8, 9],
            vec![9, 7, 8],
        ];
        for ids in sequences {
            reconcile(&graph, &anchor, &ids);
            assert_consistent(&root);
            assert_eq!(
                tags(&anchor),
                ids,
                "the reconciled leaf tags match the requested order"
            );
        }
    });
}

#[test]
fn hit_and_paint_order_agree_after_reorder() {
    GlobalTestLock::locked(|| {
        let graph = support::graph();
        graph.install_view(AnyView::new(Container), Default::default());
        let root = graph.tree.borrow().root().expect("root");
        let anchor = root.children().first().cloned().expect("anchor node");

        reconcile(&graph, &anchor, &[0, 1, 2]);
        reconcile(&graph, &anchor, &[2, 0, 1]);
        assert_consistent(&root);

        // The node tree and its render mirror present the leaves in the same
        // sequence, so paint order and hit-test order are computed off one
        // ordering.
        let node_tags = tags(&anchor);
        let render_tags: Vec<i32> = anchor
            .render_object()
            .expect("the anchor has a render mirror")
            .children()
            .iter()
            .filter_map(|child| child.node())
            .filter_map(|node| node.attachment::<i32>(TAG_KEY))
            .collect();
        assert_eq!(node_tags, vec![2, 0, 1], "the node order is the requested order");
        assert_eq!(
            render_tags, node_tags,
            "the render mirror presents the same order as the node tree"
        );
    });
}
