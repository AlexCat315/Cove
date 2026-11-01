//! Translation of CoveUIScene/Input/HitTester.swift.
use crate::scene::{Node, placement_step};
use crate::{Point, Rect};

/// Result of a hit test — the deepest hit-testable node containing the point,
/// plus the chain from root → node for capture/bubble traversal.
#[derive(Clone, Debug)]
pub struct HitResult {
    /// Deepest hit-testable node under the point.
    pub node: Node,
    /// Path from root to `node`, inclusive at both ends.
    pub path: Vec<Node>,
    /// Point in `node`'s local coordinate space.
    pub local_point: Point,
}

/// Hit-tests a `Node` tree against a window-local point.
///
/// Frames are parent-local. Points are mapped into a node's space by the
/// *inverse* of the same placement the renderers use, so a scaled node is hit
/// where it is drawn rather than where layout left it. Children are visited in
/// reverse paint order, and `clipsToBounds` rejects hits outside the frame.
pub struct HitTester;
impl HitTester {
    pub fn hit_test(root: &Node, point: Point) -> Option<HitResult> {
        let mut path = Vec::new();
        walk(root, point, &mut path)
    }
}

fn walk(node: &Node, point_in_parent: Point, path: &mut Vec<Node>) -> Option<HitResult> {
    if !node.is_layout_visible() || !node.allows_hit_testing() || !node.is_interaction_enabled() {
        return None;
    }
    // A node collapsed to nothing by a transition has no area to hit.
    if node.transform().scale == 0.0 {
        return None;
    }
    let frame = node.frame();
    let own = placement_step(frame, node.transform(), Point::default());
    let local = own.source(point_in_parent);
    let local_bounds = Rect::new(0.0, 0.0, frame.width, frame.height);
    if node.clips_to_bounds() && !local_bounds.contains(local) {
        return None;
    }
    path.push(node.clone());
    let content = placement_step(frame, node.transform(), node.content_offset());
    let child_point = content.source(point_in_parent);
    let children = if node.children_may_need_z_sort() {
        node.render_ordered_children()
    } else {
        node.children()
    };
    for child in children.iter().rev() {
        if let Some(hit) = walk(child, child_point, path) {
            return Some(hit);
        }
    }
    if node.is_hit_testable() && local_bounds.contains(local) {
        return Some(HitResult {
            node: node.clone(),
            path: path.clone(),
            local_point: local,
        });
    }
    path.pop();
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn window() -> (Node, Node, Node) {
        let root = Node::new();
        root.set_frame(Rect::new(0.0, 0.0, 200.0, 100.0));
        let left = Node::new();
        left.set_frame(Rect::new(0.0, 0.0, 100.0, 100.0));
        let right = Node::new();
        right.set_frame(Rect::new(100.0, 0.0, 100.0, 100.0));
        root.add_child(&left);
        root.add_child(&right);
        (root, left, right)
    }

    #[test]
    fn deepest_child_wins_and_path_is_root_first() {
        let (root, left, right) = window();
        let hit = HitTester::hit_test(&root, Point { x: 150.0, y: 50.0 }).unwrap();
        assert_eq!(hit.node, right);
        assert_eq!(hit.path.len(), 2);
        assert_eq!(hit.path[0], root);
        let hit = HitTester::hit_test(&root, Point { x: 10.0, y: 10.0 }).unwrap();
        assert_eq!(hit.node, left);
        assert!(HitTester::hit_test(&root, Point { x: 400.0, y: 10.0 }).is_none());
    }

    #[test]
    fn clipping_and_scaling_are_honoured() {
        let (root, left, _) = window();
        left.set_clips_to_bounds(true);
        let child = Node::new();
        child.set_frame(Rect::new(-50.0, 0.0, 200.0, 40.0));
        left.add_child(&child);
        // Outside the clipping parent: the whole subtree is rejected.
        assert!(
            HitTester::hit_test(&root, Point { x: 150.0, y: 10.0 })
                .unwrap()
                .node
                != child
        );
        // Inside: the overflowing child still answers inside the clip.
        assert_eq!(
            HitTester::hit_test(&root, Point { x: 50.0, y: 10.0 })
                .unwrap()
                .node,
            child
        );

        left.set_transform(crate::scene::NodeTransform::anchored(2.0, 0.0, 0.0));
        let hit = HitTester::hit_test(&root, Point { x: 50.0, y: 10.0 }).unwrap();
        // The window point maps into `left` at (25, 5), and the child sits at
        // x = -50 inside `left`, so its local point is (75, 5). The expectation
        // must account for the child's own frame origin, not just the scale.
        assert_eq!(hit.local_point, Point { x: 75.0, y: 5.0 });
    }
}
