//! Translation of CoveUIScene/NodeTree.swift.
use super::{InvalidationSource, Node, PerformanceTimeline};
use std::cell::RefCell;
use std::rc::Rc;

/// Owns the root node and drives depth-first traversal.
pub struct NodeTree {
    root: Option<Node>,
    /// Shared so a `Recomposer` can record component spans on the same timeline.
    pub timeline: Rc<RefCell<PerformanceTimeline>>,
    cached_node_count: usize,
    cached_node_count_version: u64,
    cached_node_count_root: Option<usize>,
    node_count_stack: Vec<Node>,
    flush_stack: Vec<Node>,
}
impl NodeTree {
    pub fn new() -> Self {
        Self {
            root: None,
            timeline: Rc::new(RefCell::new(PerformanceTimeline::default())),
            cached_node_count: 0,
            cached_node_count_version: 0,
            cached_node_count_root: None,
            node_count_stack: Vec::new(),
            flush_stack: Vec::new(),
        }
    }
    pub fn root(&self) -> Option<Node> {
        self.root.clone()
    }
    pub fn set_root(&mut self, root: Option<Node>) {
        self.root = root;
    }
    pub fn has_render_updates(&self) -> bool {
        self.root.as_ref().is_some_and(Node::render_dirty)
    }
    /// Number of reachable nodes, memoised on the root's structural version.
    pub fn node_count(&mut self) -> usize {
        let Some(root) = self.root.clone() else {
            self.cached_node_count_root = None;
            return 0;
        };
        let identity = root.cell() as *const _ as usize;
        if self.cached_node_count_root == Some(identity)
            && self.cached_node_count_version == root.subtree_structure_version()
        {
            return self.cached_node_count;
        }
        self.node_count_stack.clear();
        self.node_count_stack.push(root.clone());
        let mut count = 0;
        while let Some(node) = self.node_count_stack.pop() {
            count += 1;
            self.node_count_stack.extend(node.children());
        }
        self.cached_node_count_root = Some(identity);
        self.cached_node_count_version = root.subtree_structure_version();
        self.cached_node_count = count;
        count
    }
    pub fn mark_dirty(&self, node: &Node) {
        node.mark_dirty(InvalidationSource::Unknown);
    }
    pub fn mark_render_dirty(&self, node: &Node) {
        node.mark_render_dirty(InvalidationSource::Unknown);
    }
    /// Depth-first traversal that resets layout and render invalidation flags
    /// on every reachable node.
    pub fn flush(&mut self) {
        let Some(root) = self.root.clone() else {
            return;
        };
        let trace = self.timeline.borrow().begin();
        self.flush_stack.push(root);
        while let Some(node) = self.flush_stack.pop() {
            let should_visit_children = node.is_dirty() || node.render_dirty();
            node.clear_dirty_flags();
            if !should_visit_children {
                continue;
            }
            for child in node.children().into_iter().rev() {
                if child.is_dirty() || child.render_dirty() {
                    self.flush_stack.push(child);
                }
            }
        }
        self.timeline
            .borrow_mut()
            .end(trace, "frame", "Flush dirty flags", None, &[]);
    }
}
impl Default for NodeTree {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_count_is_memoised_until_structure_changes() {
        let mut tree = NodeTree::new();
        assert_eq!(tree.node_count(), 0);
        let root = Node::new();
        let first = Node::new();
        root.add_child(&first);
        tree.set_root(Some(root.clone()));
        assert_eq!(tree.node_count(), 2);
        root.add_child(&Node::new());
        assert_eq!(tree.node_count(), 3);
    }

    #[test]
    fn flush_clears_flags_and_reports_updates() {
        let mut tree = NodeTree::new();
        let root = Node::new();
        let child = Node::new();
        root.add_child(&child);
        tree.set_root(Some(root.clone()));
        child.mark_render_dirty(InvalidationSource::Unknown);
        assert!(tree.has_render_updates());
        tree.flush();
        assert!(!tree.has_render_updates());
        assert!(!child.render_dirty() && !child.is_dirty());
    }
}
