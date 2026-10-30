//! Translation of Android/CoveAndroidProbe/Layout.swift.

use super::runner::{require, ProbeContext, ProbeFailure};
use cove_core::scene::{FlexDirection, InvalidationSource, LayoutNode, LayoutPass, Node, NodeTree};
use cove_core::Rect;

impl ProbeContext {
    /// Verifies the shared scene graph, Yoga layout and the invalidation/reset
    /// contract in one pass.
    pub fn layout(&self) -> Result<String, ProbeFailure> {
        let root = LayoutNode::new();
        let first = LayoutNode::new();
        let second = LayoutNode::new();
        root.set_flex_direction(FlexDirection::Row);
        first.set_width(Some(20.0));
        first.set_height(Some(12.0));
        second.set_flex_grow(1.0);
        second.set_height(Some(12.0));
        root.add_child(&first);
        root.add_child(&second);

        let tree = Node::new();
        let a = Node::new();
        let b = Node::new();
        tree.add_child(&a);
        tree.add_child(&b);
        LayoutPass::run(&root, &tree, Some(64.0), Some(64.0));

        let (a_frame, b_frame): (Rect, Rect) = (a.frame(), b.frame());
        require(
            a_frame.width == 20.0 && b_frame.x == 20.0 && b_frame.width == 44.0,
            "Yoga layout mismatch",
        )?;

        let mut nodes = NodeTree::new();
        nodes.set_root(Some(tree));
        nodes.flush();
        require(!nodes.has_render_updates(), "scene invalidation did not reset")?;

        b.mark_render_dirty(InvalidationSource::Unknown);
        require(nodes.has_render_updates(), "scene invalidation did not propagate")?;
        Ok("shared CoveUI scene graph, Yoga layout and invalidation/reset verified".to_owned())
    }
}
