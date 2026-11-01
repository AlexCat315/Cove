//! Translation of `Tests/CoveUIRuntimeTests/NodeTreeTests.swift`.
//!
//! Behaviour checklist:
//! - `add_child` links parent ↔ child and `remove_child` unlinks both ways
//! - `remove_from_parent` uses the parent reference
//! - `mark_dirty` propagates to every ancestor and sets render dirtiness
//! - `mark_render_dirty` propagates without setting layout dirtiness
//! - `NodeTree::mark_dirty` matches `Node::mark_dirty`
//! - `flush` resets every dirty flag depth-first and preserves structure
//! - `flush` on an empty tree does not panic
//! - `node_count` tracks membership through the structural-version cache

use cove_core::scene::{InvalidationSource, Node, NodeTree};

#[test]
fn add_child() {
    let parent = Node::new();
    let child = Node::new();
    parent.add_child(&child);

    assert_eq!(parent.children().len(), 1);
    let recorded = child.parent().expect("the child records its parent");
    assert_eq!(recorded.id(), parent.id());
}

#[test]
fn remove_child() {
    let parent = Node::new();
    let child = Node::new();
    parent.add_child(&child);
    parent.remove_child(&child);

    assert!(parent.children().is_empty());
    assert!(child.parent().is_none());
}

#[test]
fn remove_from_parent() {
    let parent = Node::new();
    let child = Node::new();
    parent.add_child(&child);
    child.remove_from_parent();

    assert!(parent.children().is_empty());
    assert!(child.parent().is_none());
}

#[test]
fn mark_dirty_propagates() {
    let root = Node::new();
    let mid = Node::new();
    let leaf = Node::new();
    root.add_child(&mid);
    mid.add_child(&leaf);

    leaf.mark_dirty(InvalidationSource::Unknown);

    assert!(leaf.is_dirty());
    assert!(mid.is_dirty());
    assert!(root.is_dirty());
    assert!(leaf.render_dirty());
    assert!(mid.render_dirty());
    assert!(root.render_dirty());
}

#[test]
fn mark_render_dirty_propagates() {
    let root = Node::new();
    let mid = Node::new();
    let leaf = Node::new();
    root.add_child(&mid);
    mid.add_child(&leaf);
    // Clear the build-time flags so only the new propagation is observed.
    let mut tree = NodeTree::new();
    tree.set_root(Some(root.clone()));
    tree.flush();
    assert!(!root.render_dirty() && !mid.render_dirty() && !leaf.render_dirty());

    leaf.mark_render_dirty(InvalidationSource::Unknown);

    assert!(!leaf.is_dirty(), "a render-only change leaves layout clean");
    assert!(!mid.is_dirty());
    assert!(!root.is_dirty());
    assert!(leaf.render_dirty());
    assert!(mid.render_dirty());
    assert!(root.render_dirty());
}

#[test]
fn tree_mark() {
    let mut tree = NodeTree::new();
    let root = Node::new();
    let child = Node::new();
    root.add_child(&child);
    tree.set_root(Some(root.clone()));
    tree.flush();
    assert!(!child.is_dirty() && !root.is_dirty());

    tree.mark_dirty(&child);

    assert!(child.is_dirty());
    assert!(root.is_dirty());
}

#[test]
fn flush() {
    let mut tree = NodeTree::new();
    let root = Node::new();
    let child = Node::new();
    let grandchild = Node::new();
    root.add_child(&child);
    child.add_child(&grandchild);
    tree.set_root(Some(root.clone()));

    grandchild.mark_dirty(InvalidationSource::Unknown);
    assert!(grandchild.is_dirty());
    tree.flush();

    assert!(!root.is_dirty());
    assert!(!child.is_dirty());
    assert!(!grandchild.is_dirty());
    assert!(!root.render_dirty());
    assert!(!child.render_dirty());
    assert!(!grandchild.render_dirty());
}

#[test]
fn flush_empty_tree() {
    let mut tree = NodeTree::new();
    tree.flush();
    assert!(tree.root().is_none());
}

#[test]
fn node_count_tracks_membership() {
    let mut tree = NodeTree::new();
    let root = Node::new();
    let mid = Node::new();
    let leaf = Node::new();
    root.add_child(&mid);
    mid.add_child(&leaf);
    tree.set_root(Some(root));
    assert_eq!(tree.node_count(), 3);

    // The next read is served from the cache, so this asserts the deep
    // insertion still invalidated it through the root's version.
    leaf.add_child(&Node::new());
    assert_eq!(tree.node_count(), 4);

    mid.remove_child(&leaf);
    assert_eq!(tree.node_count(), 2);

    // A different root must not reuse the old count.
    let mut other = NodeTree::new();
    other.set_root(Some(Node::new()));
    assert_eq!(other.node_count(), 1);
}

#[test]
fn flush_preserves_structure() {
    let mut tree = NodeTree::new();
    let root = Node::new();
    let a = Node::new();
    let b = Node::new();
    root.add_child(&a);
    root.add_child(&b);
    tree.set_root(Some(root.clone()));

    tree.flush();

    assert_eq!(root.children().len(), 2);
}
