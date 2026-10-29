//! Translation of CoveUIScene/LayoutPass.swift.
use super::{LayoutNode, Node};

/// Applies layout results back to the `Node` tree in a single depth-first pass.
pub struct LayoutPass;
impl LayoutPass {
    /// Calculate layout and write results to every `Node.frame`.
    ///
    /// Both trees must have the same structure (same child count and order at
    /// every level).
    pub fn run(root_layout_node: &LayoutNode, root_node: &Node, available_width: Option<f32>, available_height: Option<f32>) {
        root_layout_node.calculate_layout(available_width, available_height, super::Direction::Ltr);
        apply(root_layout_node, root_node);
    }
}

fn apply(layout_node: &LayoutNode, node: &Node) {
    node.set_frame(layout_node.frame());
    let layout_children = layout_node.children();
    let node_children = node.children();
    for (layout_child, node_child) in layout_children.iter().zip(node_children.iter()) {
        apply(layout_child, node_child);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Rect;

    #[test]
    fn frames_are_written_down_the_matching_trees() {
        let root = Node::new();
        let child = Node::new();
        root.add_child(&child);
        let layout_root = LayoutNode::new();
        layout_root.set_flex_direction(super::super::FlexDirection::Column);
        layout_root.set_width(Some(200.0));
        let layout_child = LayoutNode::new();
        layout_child.set_height(Some(40.0));
        layout_root.add_child(&layout_child);
        LayoutPass::run(&layout_root, &root, Some(200.0), Some(400.0));
        assert_eq!(root.frame(), Rect::new(0.0, 0.0, 200.0, 40.0));
        assert_eq!(child.frame(), Rect::new(0.0, 0.0, 200.0, 40.0));
    }
}
