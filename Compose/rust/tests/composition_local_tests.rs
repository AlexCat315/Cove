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
//!
//! Name mapping: `CompositionLocal(defaultValue:)` is
//! `CompositionLocal::constant(..)` (or `::new(|| ..)` for a lazy default),
//! `compositionValue(of:)` is `composition_value(..)`, and
//! `setCompositionValue` is `set_composition_value(..)`.

mod support;

use std::rc::Rc;

use cove_composition::{AnyView, ProvideCompositionLocalModifier, ViewExt};
use cove_core::scene::{CompositionLocal, Node};

use support::{DebugNode, GlobalTestLock};

#[test]
fn node_falls_back_to_default() {
    GlobalTestLock::locked(|| {
        let local = CompositionLocal::constant(42i32);
        let node = Node::new();
        assert_eq!(
            node.composition_value(&local),
            42,
            "without a provider the declaration's default applies"
        );
    });
}

#[test]
fn node_stores_value() {
    GlobalTestLock::locked(|| {
        let local = CompositionLocal::constant("default".to_string());
        let node = Node::new();
        node.set_composition_value(&local, "hello".to_string());
        assert_eq!(
            node.composition_value(&local),
            "hello",
            "a per-node value shadows the default"
        );
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
        assert_eq!(leaf.composition_value(&local), 7, "the leaf sees the root");
        assert_eq!(mid.composition_value(&local), 7, "the middle sees the root");
        assert_eq!(root.composition_value(&local), 7, "the root sees itself");
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
        assert_eq!(
            leaf.composition_value(&local),
            2,
            "the nearest ancestor's provider wins"
        );
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
        assert_eq!(node.composition_value(&a), 11, "declaration a keeps its value");
        assert_eq!(node.composition_value(&b), 22, "declaration b keeps its value");
    });
}

#[test]
fn modifier_writes_onto_wrapper() {
    GlobalTestLock::locked(|| {
        let local = Rc::new(CompositionLocal::constant(0i32));
        let graph = support::graph();
        graph.install_view(
            AnyView::new(
                AnyView::new(DebugNode::new("leaf")).with_modifier(
                    ProvideCompositionLocalModifier::new(local.clone(), 99),
                ),
            ),
            Default::default(),
        );

        // root -> wrapper node carrying the provider.
        let root = graph.tree.borrow().root().expect("root node");
        let wrapper = root.children().first().cloned().expect("wrapper node");
        assert_eq!(
            wrapper.composition_value(&local),
            99,
            "the provider value lives on the wrapper node"
        );
    });
}

#[test]
fn descendants_resolve_provider() {
    GlobalTestLock::locked(|| {
        let local = Rc::new(CompositionLocal::constant("default".to_string()));
        let graph = support::graph();
        graph.install_view(
            AnyView::new(
                AnyView::new(DebugNode::new("x")).with_modifier(
                    ProvideCompositionLocalModifier::new(local.clone(), "themed".to_string()),
                ),
            ),
            Default::default(),
        );

        let root = graph.tree.borrow().root().expect("root node");
        let deepest = support::deepest_node(&root);
        assert_eq!(
            deepest.composition_value(&local),
            "themed",
            "the deepest descendant reaches the provider through the parent walk"
        );
    });
}
