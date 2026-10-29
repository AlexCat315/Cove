//! Translation of Tests/CoveUIComposeTests/CompositionLocalTests.swift.
use cove_composition::{AnyView, PrimitiveView, View, ViewExt, ViewGraph, ViewKind};
use cove_core::scene::{ComponentSourceLocation, CompositionLocal, Node, NodeTree, Recomposer};
use std::rc::Rc;

#[derive(Clone)]
struct DebugNode {
    label: String,
}
impl View for DebugNode {
    fn kind(&self) -> ViewKind<'_> {
        ViewKind::Primitive(self)
    }
    fn tag(&self) -> &'static str {
        "DebugNode"
    }
}
impl PrimitiveView for DebugNode {
    fn make_node(&self) -> Node {
        Node::new()
    }
    fn update_node(&self, _node: &Node) {}
}

fn install(root: impl View + 'static) -> Rc<std::cell::RefCell<NodeTree>> {
    let tree = Rc::new(std::cell::RefCell::new(NodeTree::new()));
    let graph = Rc::new(ViewGraph::new(tree.clone(), Rc::new(Recomposer::new())));
    graph.install(root, ComponentSourceLocation::new("composition_local.rs", "Compose/tests/composition_local.rs", 1));
    tree
}

#[test]
fn node_falls_back_to_default() {
    let local = Rc::new(CompositionLocal::constant(42));
    let node = Node::new();
    assert_eq!(node.composition_value(&local), 42);
}

#[test]
fn node_stores_value() {
    let local = Rc::new(CompositionLocal::constant("default".to_string()));
    let node = Node::new();
    node.set_composition_value(&local, "hello".to_string());
    assert_eq!(node.composition_value(&local), "hello");
}

#[test]
fn lookup_walks_parents() {
    let local = Rc::new(CompositionLocal::constant(-1));
    let root = Node::new();
    let mid = Node::new();
    let leaf = Node::new();
    root.add_child(&mid);
    mid.add_child(&leaf);
    root.set_composition_value(&local, 7);
    assert_eq!(leaf.composition_value(&local), 7);
    assert_eq!(mid.composition_value(&local), 7);
    assert_eq!(root.composition_value(&local), 7);
}

#[test]
fn nearest_ancestor_wins() {
    let local = Rc::new(CompositionLocal::constant(0));
    let root = Node::new();
    let mid = Node::new();
    let leaf = Node::new();
    root.add_child(&mid);
    mid.add_child(&leaf);
    root.set_composition_value(&local, 1);
    mid.set_composition_value(&local, 2);
    assert_eq!(leaf.composition_value(&local), 2);
}

#[test]
fn distinct_locals_are_isolated() {
    let a = Rc::new(CompositionLocal::constant(0));
    let b = Rc::new(CompositionLocal::constant(0));
    let node = Node::new();
    node.set_composition_value(&a, 11);
    node.set_composition_value(&b, 22);
    assert_eq!(node.composition_value(&a), 11);
    assert_eq!(node.composition_value(&b), 22);
}

#[test]
fn modifier_writes_onto_wrapper() {
    let local = Rc::new(CompositionLocal::constant(0));
    let tree = install(DebugNode { label: "leaf".into() }.composition_local(local.clone(), 99));
    let wrapper = tree.borrow().root().unwrap().children().first().cloned().unwrap();
    assert_eq!(wrapper.composition_value(&local), 99);
}

struct ParentView {
    inner: DebugNode,
}
impl View for ParentView {
    fn body(&self) -> AnyView {
        AnyView::new(self.inner.clone())
    }
}

#[test]
fn descendants_resolve_provider() {
    let local = Rc::new(CompositionLocal::constant("default".to_string()));
    let tree = install(
        ParentView { inner: DebugNode { label: "x".into() } }.composition_local(local.clone(), "themed".to_string()),
    );
    let mut cursor = tree.borrow().root().unwrap();
    while let Some(next) = cursor.children().first().cloned() {
        cursor = next;
    }
    assert_eq!(cursor.composition_value(&local), "themed");
}
