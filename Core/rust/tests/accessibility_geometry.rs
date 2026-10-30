//! Translation of `Tests/CoveUIRuntimeTests/AccessibilityGeometryTests.swift`.
//!
//! Behaviour checklist: accessibility traversal reports each element's frame in
//! window space, matching `Node::absolute_frame` through nested scales and
//! content offsets, whether the snapshot starts at the root or at an interior
//! scroll container.

use cove_core::scene::{
    AccessibilityRole, AccessibilitySemantics, AccessibilityTree, Node, NodeTransform,
};
use cove_core::{Point, Rect};

fn node(x: f32, y: f32, width: f32, height: f32) -> Node {
    let node = Node::new();
    node.set_frame(Rect::new(x, y, width, height));
    node
}

fn assert_frame_matches(actual: Rect, expected: Rect, context: &str) {
    let close = |a: f32, b: f32| (a - b).abs() < 0.001;
    assert!(
        close(actual.x, expected.x)
            && close(actual.y, expected.y)
            && close(actual.width, expected.width)
            && close(actual.height, expected.height),
        "{context}: accessibility frame {actual:?} must match absolute_frame {expected:?}"
    );
}

#[test]
fn nested_placement() {
    let root = node(15.0, 20.0, 800.0, 600.0);
    root.set_transform(NodeTransform::new(0.9, Point { x: 0.0, y: 0.0 }));
    root.set_content_offset(Point { x: 3.0, y: 5.0 });

    let scroll = node(30.0, 40.0, 400.0, 300.0);
    scroll.set_transform(NodeTransform::new(0.8, Point { x: 0.0, y: 0.0 }));
    scroll.set_content_offset(Point { x: 7.5, y: 12.5 });
    root.add_child(&scroll);

    let label = node(25.0, 35.0, 100.0, 30.0);
    label.set_accessibility(Some(AccessibilitySemantics {
        role: AccessibilityRole::StaticText,
        label: "Nested".into(),
        ..Default::default()
    }));
    scroll.add_child(&label);

    // Both the root and the interior scroll container must resolve the same
    // window-space frame: the clip origin must not shift the reported rect.
    for (name, subtree) in [("root", &root), ("scroll", &scroll)] {
        let elements = AccessibilityTree::snapshot_in(subtree, Some(root.absolute_frame()), true);
        assert_eq!(
            elements.len(),
            1,
            "{name}: only the labelled node is exposed, saw {}",
            elements.len()
        );
        assert_frame_matches(elements[0].frame, label.absolute_frame(), name);
    }
}
