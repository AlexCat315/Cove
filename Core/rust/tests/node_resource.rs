//! Translation of `Tests/CoveUIRuntimeTests/NodeResourceTests.swift`.
//!
//! Phase 2 acceptance: external registrations are released by the node
//! lifecycle. A resource attached to a node must `unmount` whenever the node
//! leaves the tree — including a node deep inside a removed subtree — with no
//! help from any modifier or teardown bookkeeping.
//!
//! Behaviour checklist:
//! - `add_resource` mounts immediately
//! - `remove_child` unmounts the removed node's resource
//! - `remove_child` unmounts resources deep inside the removed subtree
//! - `remove_from_parent` releases resources
//! - removing a non-child does not unmount
//! - `first_resource` finds the attached resource by type

use std::cell::Cell;
use std::rc::Rc;

use cove_core::scene::{Node, NodeResource};

#[derive(Default)]
struct ProbeResource {
    mounts: Cell<usize>,
    unmounts: Cell<usize>,
}

impl NodeResource for ProbeResource {
    fn mount(&self, _node: &Node) {
        self.mounts.set(self.mounts.get() + 1);
    }
    fn unmount(&self, _node: &Node) {
        self.unmounts.set(self.unmounts.get() + 1);
    }
    fn as_any_rc(self: Rc<Self>) -> Rc<dyn std::any::Any> {
        self
    }
}

fn probe() -> (Rc<ProbeResource>, Rc<dyn NodeResource>) {
    let probe = Rc::new(ProbeResource::default());
    (probe.clone(), probe as Rc<dyn NodeResource>)
}

#[test]
fn add_resource_mounts() {
    let node = Node::new();
    let (probe, resource) = probe();
    node.add_resource(resource);
    assert_eq!(probe.mounts.get(), 1);
    assert_eq!(probe.unmounts.get(), 0);
}

#[test]
fn remove_child_unmounts() {
    let parent = Node::new();
    let child = Node::new();
    let (probe, resource) = probe();
    child.add_resource(resource);
    parent.add_child(&child);

    parent.remove_child(&child);
    assert_eq!(probe.unmounts.get(), 1);
}

#[test]
fn remove_child_unmounts_subtree() {
    let root = Node::new();
    let mid = Node::new();
    let leaf = Node::new();
    let (mid_probe, mid_resource) = probe();
    let (leaf_probe, leaf_resource) = probe();
    mid.add_resource(mid_resource);
    leaf.add_resource(leaf_resource);
    root.add_child(&mid);
    mid.add_child(&leaf);

    // Removing the mid subtree releases both mid's and leaf's resources.
    root.remove_child(&mid);
    assert_eq!(mid_probe.unmounts.get(), 1);
    assert_eq!(leaf_probe.unmounts.get(), 1);
}

#[test]
fn remove_from_parent_releases_resources() {
    let parent = Node::new();
    let child = Node::new();
    let (probe, resource) = probe();
    child.add_resource(resource);
    parent.add_child(&child);

    child.remove_from_parent();
    assert_eq!(probe.unmounts.get(), 1);
}

#[test]
fn removing_non_child_is_no_op() {
    let parent = Node::new();
    let stranger = Node::new();
    let (probe, resource) = probe();
    stranger.add_resource(resource);

    // A node that was never a child keeps its resources.
    parent.remove_child(&stranger);
    assert_eq!(probe.unmounts.get(), 0);
}

#[test]
fn first_resource_lookup() {
    let node = Node::new();
    let (probe, resource) = probe();
    node.add_resource(resource);
    let found = node.first_resource::<ProbeResource>().expect("resource is attached");
    assert!(
        Rc::ptr_eq(&found, &probe),
        "first_resource returns the same instance that was attached"
    );
}
