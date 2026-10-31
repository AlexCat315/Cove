//! Translation of CoveUIScene/NodeResource.swift.
use super::Node;
use std::any::Any;
use std::rc::Rc;

/// An external registration owned by a node's lifetime.
///
/// Anything a node registers with the outside world — a portal/overlay entry,
/// pointer capture, focus — is a `NodeResource` attached to that node. When the
/// node leaves the tree for ANY reason, `Node::remove_child` walks the departing
/// subtree and calls `unmount` on every resource, so leaking an entry is not
/// expressible.
pub trait NodeResource {
    /// Called when the resource is attached to a node (`Node::add_resource`).
    fn mount(&self, node: &Node);
    /// Called when the owning node leaves the tree, for ANY reason. Must release
    /// everything the resource acquired and be idempotent.
    fn unmount(&self, node: &Node);
    /// Enables `Node::first_resource` to recover the concrete type.
    fn as_any_rc(self: Rc<Self>) -> Rc<dyn Any>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    struct Recorder {
        mounts: Cell<u32>,
        unmounts: Cell<u32>,
    }
    impl NodeResource for Recorder {
        fn mount(&self, _node: &Node) {
            self.mounts.set(self.mounts.get() + 1);
        }
        fn unmount(&self, _node: &Node) {
            self.unmounts.set(self.unmounts.get() + 1);
        }
        fn as_any_rc(self: Rc<Self>) -> Rc<dyn Any> {
            self
        }
    }

    #[test]
    fn resources_mount_and_release_with_the_node() {
        let parent = Node::new();
        let child = Node::new();
        parent.add_child(&child);
        let recorder = Rc::new(Recorder {
            mounts: Cell::new(0),
            unmounts: Cell::new(0),
        });
        child.add_resource(recorder.clone());
        assert_eq!(recorder.mounts.get(), 1);
        assert!(child.first_resource::<Recorder>().is_some());
        parent.remove_child(&child);
        assert_eq!(recorder.unmounts.get(), 1);
    }
}
