//! Translation of `Tests/CoveUIComposeTests/RenderTreeMirrorTests.swift`.
//!
//! Behaviour checklist:
//! - installing a root mirrors the whole node tree, sets back-pointers from
//!   `Node` to `RenderObject` and back, counts the mirrored objects and marks
//!   the root as a layer root unconditionally
//! - a plain primitive without clip, opacity or shadow is not a layer root
//! - `clips_to_bounds` and sub-1.0 `opacity` each promote a node to layer root
//! - reconciling a reordered keyed child list keeps the same `RenderObject`s and
//!   reorders the parent's mirror children to match
//! - tearing down a removed child drops its mirror and shrinks the parent's
//!   mirror children
//! - `layer_roots()` enumerates the root plus the promoted descendants
//! - the flat draw order is complete (one entry per mirrored object, no
//!   duplicates), visits every parent before its children, emits siblings in
//!   `zIndex` order, and is rebuilt after a structural reconcile
//!
//! Named differences from the Swift suite:
//! - Swift reaches the materialised leaves through fixed child indices
//!   (`tree.root!.children.first!.children.first!`). The Rust graph inserts a
//!   single synthetic root above a directly installed primitive, so the suite
//!   locates leaves by their recorded label attachment and walks to the parent
//!   instead; the asserted classification, identity and ordering are unchanged.

use cove_composition::Scope;
mod support;

use std::collections::HashMap;

use cove_composition::{AnyView, IdentifiedView, PrimitiveView, TupleView, View, ViewGraph};
use cove_core::scene::{ElementId, LayoutNode, Node};

use support::GlobalTestLock;

const LABEL_KEY: &str = "__render_tree_label";

/// A primitive whose node configuration is supplied by the case, mirroring the
/// Swift `_LayerNode` configurator.
struct LayerNode {
    label: &'static str,
    configure: fn(&Node),
}

impl LayerNode {
    fn new(label: &'static str, configure: fn(&Node)) -> Self {
        Self { label, configure }
    }
}

impl PrimitiveView for LayerNode {
    fn make_node(&self) -> Node {
        let node = Node::new();
        self.apply(&node);
        node
    }
    fn update_node(&self, node: &Node) {
        self.apply(node);
    }
    fn make_layout_node(&self) -> Option<LayoutNode> {
        None
    }
}

impl LayerNode {
    fn apply(&self, node: &Node) {
        node.set_attachment(LABEL_KEY, self.label.to_string());
        (self.configure)(node);
    }
}
cove_composition::impl_primitive_view!(LayerNode);

fn plain(label: &'static str) -> AnyView {
    AnyView::new(LayerNode::new(label, |_| {}))
}

fn clipped(label: &'static str) -> AnyView {
    AnyView::new(LayerNode::new(label, |node| node.set_clips_to_bounds(true)))
}

fn translucent(label: &'static str) -> AnyView {
    AnyView::new(LayerNode::new(label, |node| node.set_opacity(0.5)))
}

fn identified(label: &'static str, key: &'static str) -> AnyView {
    AnyView::new(IdentifiedView::new(LayerNode::new(label, |_| {}), key))
}

fn stacked(views: Vec<AnyView>) -> AnyView {
    AnyView::new(TupleView::new(views))
}

fn root(graph: &ViewGraph) -> Node {
    graph.tree.borrow().root().expect("the graph has a root")
}

fn nodes(below: &Node) -> Vec<Node> {
    let mut collected = vec![below.clone()];
    let mut index = 0;
    while index < collected.len() {
        collected.extend(collected[index].children());
        index += 1;
    }
    collected
}

fn labelled(graph: &ViewGraph, label: &str) -> Node {
    nodes(&root(graph))
        .into_iter()
        .find(|node| node.attachment::<String>(LABEL_KEY).as_deref() == Some(label))
        .unwrap_or_else(|| panic!("a node labelled {label} is mounted"))
}

fn anchor_of(graph: &ViewGraph, label: &str) -> Node {
    labelled(graph, label)
        .parent()
        .expect("the leaf has a materialised parent")
}

fn reconcile(graph: &std::rc::Rc<ViewGraph>, anchor: &Node, new_views: &[AnyView]) {
    let layout_parent = graph.layout_node(anchor);
    graph.reconcile_children(anchor, layout_parent.as_ref(), new_views);
}

struct Single {
    content: fn() -> AnyView,
}

impl View for Single {
    fn body(&self, _cx: &Scope) -> AnyView {
        (self.content)()
    }
    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

struct Pair {
    content: fn() -> AnyView,
}

impl View for Pair {
    fn body(&self, _cx: &Scope) -> AnyView {
        (self.content)()
    }
    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

#[test]
fn install_mirrors_the_tree_sets_back_pointers_and_counts_the_root() {
    GlobalTestLock::locked(|| {
        let graph = support::graph();
        graph.install(LayerNode::new("root", |_| {}), Default::default());

        let tree_root = root(&graph);
        let render_root = graph
            .render_tree()
            .root()
            .expect("installing a root builds a mirror root");
        assert_eq!(
            render_root.node().as_ref(),
            Some(&tree_root),
            "the mirror root is bound to the tree root"
        );
        assert_eq!(
            tree_root.render_object().as_ref(),
            Some(&render_root),
            "the tree root points back at the mirror root"
        );
        assert!(
            graph.render_tree().object_count() >= 1,
            "the mirror counts at least the root, got {}",
            graph.render_tree().object_count()
        );
        assert!(
            render_root.is_layer_root(),
            "the mirror root is always a layer root"
        );
    });
}

#[test]
fn a_plain_leaf_is_not_a_layer_root() {
    GlobalTestLock::locked(|| {
        let graph = support::graph();
        graph.install(
            Single {
                content: || plain("leaf"),
            },
            Default::default(),
        );

        let leaf = labelled(&graph, "leaf");
        let object = graph
            .render_tree()
            .render_object(&leaf)
            .expect("the leaf is mirrored");
        assert!(!object.is_layer_root(), "a plain leaf begins no layer");
    });
}

#[test]
fn clipping_promotes_a_leaf_to_a_layer_root() {
    GlobalTestLock::locked(|| {
        let graph = support::graph();
        graph.install(
            Single {
                content: || clipped("leaf"),
            },
            Default::default(),
        );

        let leaf = labelled(&graph, "leaf");
        let object = graph
            .render_tree()
            .render_object(&leaf)
            .expect("the leaf is mirrored");
        assert!(
            object.is_layer_root(),
            "clips_to_bounds promotes the leaf to a layer root"
        );
    });
}

#[test]
fn sub_unit_opacity_promotes_a_leaf_to_a_layer_root() {
    GlobalTestLock::locked(|| {
        let graph = support::graph();
        graph.install(
            Single {
                content: || translucent("leaf"),
            },
            Default::default(),
        );

        let leaf = labelled(&graph, "leaf");
        let object = graph
            .render_tree()
            .render_object(&leaf)
            .expect("the leaf is mirrored");
        assert!(
            object.is_layer_root(),
            "opacity below 1.0 promotes the leaf to a layer root"
        );
    });
}

#[test]
fn reconcile_keeps_render_objects_for_reused_children() {
    GlobalTestLock::locked(|| {
        let graph = support::graph();
        graph.install(
            Pair {
                content: || stacked(vec![identified("a", "a"), identified("b", "b")]),
            },
            Default::default(),
        );
        let original_a = labelled(&graph, "a");
        let original_b = labelled(&graph, "b");
        let anchor = anchor_of(&graph, "a");
        let render_a = graph
            .render_tree()
            .render_object(&original_a)
            .expect("a is mirrored");
        let render_b = graph
            .render_tree()
            .render_object(&original_b)
            .expect("b is mirrored");

        reconcile(
            &graph,
            &anchor,
            &[stacked(vec![identified("b", "b"), identified("a", "a")])],
        );

        assert_eq!(
            graph.render_tree().render_object(&original_a),
            Some(render_a),
            "reused child a keeps its RenderObject"
        );
        assert_eq!(
            graph.render_tree().render_object(&original_b),
            Some(render_b),
            "reused child b keeps its RenderObject"
        );
        let mirror = graph
            .render_tree()
            .render_object(&anchor)
            .expect("the anchor is mirrored");
        assert_eq!(mirror.children().len(), 2, "the mirror keeps both children");
        assert_eq!(
            mirror.children()[0].node().as_ref(),
            Some(&original_b),
            "the mirror lists b first after the reorder"
        );
        assert_eq!(
            mirror.children()[1].node().as_ref(),
            Some(&original_a),
            "the mirror lists a second after the reorder"
        );
    });
}

#[test]
fn tearing_down_a_node_drops_its_mirror() {
    GlobalTestLock::locked(|| {
        let graph = support::graph();
        graph.install(
            Pair {
                content: || stacked(vec![identified("a", "a"), identified("b", "b")]),
            },
            Default::default(),
        );
        let anchor = anchor_of(&graph, "a");
        let b_node = labelled(&graph, "b");
        assert!(
            graph.render_tree().render_object(&b_node).is_some(),
            "b is mirrored before the reconcile"
        );

        reconcile(&graph, &anchor, &[stacked(vec![identified("a", "a")])]);

        assert!(
            graph.render_tree().render_object(&b_node).is_none(),
            "the torn-down child has no mirror"
        );
        let mirror = graph
            .render_tree()
            .render_object(&anchor)
            .expect("the anchor is mirrored");
        assert_eq!(mirror.children().len(), 1, "the anchor keeps one child");
    });
}

#[test]
fn layer_inventory_enumerates_the_root_and_clipped_descendants() {
    GlobalTestLock::locked(|| {
        let graph = support::graph();
        graph.install(
            Pair {
                content: || {
                    stacked(vec![
                        plain("plain"),
                        clipped("clip"),
                        translucent("opacity"),
                    ])
                },
            },
            Default::default(),
        );

        let layers = graph.render_tree().layer_roots();
        assert_eq!(
            layers.len(),
            3,
            "the root plus the clipped and translucent children form three layers, got {}",
            layers.len()
        );
        assert_eq!(
            graph.render_tree().root().as_ref(),
            layers.first(),
            "the inventory starts at the mirror root"
        );
    });
}

#[test]
fn flat_draw_order_is_preorder_and_complete() {
    GlobalTestLock::locked(|| {
        let graph = support::graph();
        graph.install(
            Pair {
                content: || stacked(vec![identified("a", "a"), identified("b", "b")]),
            },
            Default::default(),
        );

        let flat = graph.render_tree().flat_draw_order();
        assert_eq!(
            flat.len(),
            graph.render_tree().object_count(),
            "the flat table has one entry per mirrored object"
        );
        let mut index_of: HashMap<ElementId, usize> = HashMap::new();
        for (index, object) in flat.iter().enumerate() {
            assert!(
                index_of.insert(object.element_id(), index).is_none(),
                "object {} appears once in the flat table",
                object.element_id()
            );
        }
        for (index, object) in flat.iter().enumerate() {
            if let Some(parent) = object.parent()
                && let Some(parent_index) = index_of.get(&parent.element_id())
            {
                assert!(
                    *parent_index < index,
                    "parent {} at {parent_index} precedes child {} at {index}",
                    parent.element_id(),
                    object.element_id()
                );
            }
        }
    });
}

#[test]
fn flat_draw_order_emits_siblings_in_z_index_order() {
    GlobalTestLock::locked(|| {
        let graph = support::graph();
        graph.install(
            Pair {
                content: || {
                    stacked(vec![
                        AnyView::new(LayerNode::new("high", |node| node.set_z_index(5.0))),
                        AnyView::new(LayerNode::new("low", |node| node.set_z_index(-1.0))),
                    ])
                },
            },
            Default::default(),
        );

        let high_node = labelled(&graph, "high");
        let low_node = labelled(&graph, "low");
        let flat = graph.render_tree().flat_draw_order();
        let high_index = flat
            .iter()
            .position(|object| object.node().as_ref() == Some(&high_node));
        let low_index = flat
            .iter()
            .position(|object| object.node().as_ref() == Some(&low_node));
        let high_index = high_index.expect("the high-z leaf is in the flat order");
        let low_index = low_index.expect("the low-z leaf is in the flat order");
        assert!(
            low_index < high_index,
            "the lower zIndex paints first even though it is declared second \
             (low {low_index}, high {high_index})"
        );
    });
}

#[test]
fn flat_draw_order_rebuilds_after_a_structural_reconcile() {
    GlobalTestLock::locked(|| {
        let graph = support::graph();
        graph.install(
            Pair {
                content: || stacked(vec![identified("a", "a"), identified("b", "b")]),
            },
            Default::default(),
        );
        let before = graph.render_tree().flat_draw_order().len();
        let anchor = anchor_of(&graph, "a");

        reconcile(&graph, &anchor, &[stacked(vec![identified("a", "a")])]);

        let after = graph.render_tree().flat_draw_order().len();
        assert!(
            after < before,
            "a torn-down subtree leaves the memoised table (before {before}, after {after})"
        );
    });
}
