//! Translation of `Tests/CoveUIComposeTests/CompositionLocalTests.swift`.
//!
//! Behaviour checklist:
//! - a node without a provider falls back to the declaration's default value
//! - `set_composition_value` stores per-node values
//! - lookup walks up the parent chain
//! - the nearest ancestor wins over a more distant one
//! - distinct declarations are isolated from each other
//! - the `.composition_local` modifier writes onto the wrapper node
//! - descendants resolve a provider by walking up

mod support;

use cove_composition::{AnyView, ViewExt};
use cove_core::scene::{CompositionLocal, Node};

use support::{DebugNode, GlobalTestLock};

#[test]
fn node_falls_back_to_default() {
    GlobalTestLock::locked(|| {
        let local = CompositionLocal::constant(42i32);
        let node = Node::new();
        assert_eq!(node.composition_value(&local), 42);
    });
}

#[test]
fn node_stores_value() {
    GlobalTestLock::locked(|| {
        let local = CompositionLocal::constant("default".to_string());
        let node = Node::new();
        node.set_composition_value(&local, "hello".to_string());
        assert_eq!(node.composition_value(&local), "hello");
    });
}

#[test]
fn lookup_walks_parents() {
    GlobalTestLock::locked(|| {
        let local = CompositionLocal::constant(-1i32);
        let root = Node::new();
        let mid = Node::new();
        let leaf = Node::new();
        root.add_child(&mid);
        mid.add_child(&leaf);
        root.set_composition_value(&local, 7);
        assert_eq!(leaf.composition_value(&local), 7);
        assert_eq!(mid.composition_value(&local), 7);
        assert_eq!(root.composition_value(&local), 7);
    });
}

#[test]
fn nearest_ancestor_wins() {
    GlobalTestLock::locked(|| {
        let local = CompositionLocal::constant(0i32);
        let root = Node::new();
        let mid = Node::new();
        let leaf = Node::new();
        root.add_child(&mid);
        mid.add_child(&leaf);
        root.set_composition_value(&local, 1);
        mid.set_composition_value(&local, 2);
        assert_eq!(leaf.composition_value(&local), 2);
    });
}

#[test]
fn distinct_locals_are_isolated() {
    GlobalTestLock::locked(|| {
        let a = CompositionLocal::constant(0i32);
        let b = CompositionLocal::constant(0i32);
        let node = Node::new();
        node.set_composition_value(&a, 11);
        node.set_composition_value(&b, 22);
        assert_eq!(node.composition_value(&a), 11);
        assert_eq!(node.composition_value(&b), 22);
    });
}

#[test]
fn modifier_writes_onto_wrapper() {
    GlobalTestLock::locked(|| {
        let local = std::rc::Rc::new(CompositionLocal::constant(0i32));
        let graph = support::graph();
        graph.install_view(
            AnyView::new(AnyView::new(helpers::boxed(DebugNode::new("leaf")))
                .with_modifier(cove_composition::ProvideCompositionLocalModifier::new(
                    local.clone(),
                    99,
                ))),
            Default::default(),
        );

        // root → wrapper node. The provider value lives on the wrapper.
        let root = graph.tree.borrow().root().expect("root node");
        let wrapper = root.children().first().cloned().expect("wrapper node");
        assert_eq!(wrapper.composition_value(&local), 99);
    });
}

#[test]
fn descendants_resolve_provider() {
    GlobalTestLock::locked(|| {
        let local = std::rc::Rc::new(CompositionLocal::constant("default".to_string()));
        let graph = support::graph();
        graph.install_view(
            AnyView::new(AnyView::new(helpers::boxed(DebugNode::new("x")))
                .with_modifier(cove_composition::ProvideCompositionLocalModifier::new(
                    local.clone(),
                    "themed".to_string(),
                ))),
            Default::default(),
        );

        let root = graph.tree.borrow().root().expect("root node");
        let deepest = support::deepest_node(&root);
        assert_eq!(deepest.composition_value(&local), "themed");
    });
}

/// Keeps an inner view materialised inside a layout box, mirroring the Swift
/// `Row { ... }` wrapper these tests use to give the tree a real parent.
mod helpers {
    use cove_composition::AnyView;
    use cove_core::scene::{Align, FlexDirection, Justify, Wrap};
    use cove_ui_compose::BoxView;

    pub fn boxed(content: impl cove_composition::View + 'static) -> BoxView {
        BoxView::new(
            FlexDirection::Column,
            Align::Stretch,
            Justify::FlexStart,
            Wrap::NoWrap,
            0.0,
            AnyView::new(content),
        )
    }
}
