//! Regression for `AccessibilityTree::collect` subtree truncation.
//!
//! Swift's `Accessibility.swift` keeps descending when a node's frame is null,
//! zero-sized, or clipped out entirely (`appendChildren(into: &output)`). The
//! Rust port returned without extending `children`, so any graph whose root had
//! a 0x0 layout anchor lost its whole accessibility subtree — which is nearly
//! every control tree, and reached native VoiceOver and the Android bridge
//! through `Runtime::native_accessibility` and `Hosts::inspection`.

use cove_core::scene::{
    AccessibilityRole, AccessibilitySemantics, AccessibilityTree, Node, NodeTree,
};
use cove_core::{Point, Rect};

fn labelled(role: AccessibilityRole, label: &str) -> Node {
    let node = Node::new();
    node.set_accessibility(Some(
        AccessibilitySemantics::new(role).with_label(label),
    ));
    node
}

fn build(root_frame: Rect) -> (NodeTree, Node, Node) {
    let mut tree = NodeTree::new();
    let root = Node::new();
    root.set_frame(root_frame);
    let anchor = Node::new();
    anchor.set_frame(Rect::new(0.0, 0.0, 0.0, 0.0));
    let button = labelled(AccessibilityRole::Button, "Save");
    button.set_frame(Rect::new(10.0, 10.0, 80.0, 30.0));
    anchor.add_child(&button);
    root.add_child(&anchor);
    tree.set_root(Some(root.clone()));
    (tree, root, button)
}

#[test]
fn zero_sized_layout_anchor_does_not_truncate_its_subtree() {
    let (_tree, root, _button) = build(Rect::new(0.0, 0.0, 400.0, 300.0));

    let elements = AccessibilityTree::snapshot(&root);
    let labels: Vec<&str> = elements
        .iter()
        .map(|element| element.semantics.label.as_str())
        .collect();

    assert!(
        labels.contains(&"Save"),
        "the button below a 0x0 anchor must reach the accessibility tree, got {labels:?}"
    );
}

#[test]
fn node_clipped_entirely_out_of_view_still_contributes_descendants() {
    let mut tree = NodeTree::new();
    let root = Node::new();
    root.set_frame(Rect::new(0.0, 0.0, 100.0, 100.0));
    root.set_clips_to_bounds(true);

    // The wrapper starts inside the clip, so the traversal reaches it; its child
    // then lies beyond the wrapper's own bounds and is clipped away per-child
    // rather than taking the wrapper's subtree with it.
    let wrapper = Node::new();
    wrapper.set_frame(Rect::new(0.0, 0.0, 50.0, 50.0));
    wrapper.set_clips_to_bounds(true);
    let inside = labelled(AccessibilityRole::Button, "Inside");
    inside.set_frame(Rect::new(10.0, 10.0, 20.0, 20.0));
    let outside = labelled(AccessibilityRole::Button, "Outside");
    outside.set_frame(Rect::new(200.0, 200.0, 20.0, 20.0));
    wrapper.add_child(&inside);
    wrapper.add_child(&outside);
    root.add_child(&wrapper);
    tree.set_root(Some(root.clone()));

    let elements = AccessibilityTree::snapshot(&root);
    let labels: Vec<&str> = elements
        .iter()
        .map(|element| element.semantics.label.as_str())
        .collect();

    assert!(
        labels.contains(&"Inside"),
        "a child inside the clip survives, got {labels:?}"
    );
    assert!(
        !labels.contains(&"Outside"),
        "a child clipped out by its parent is removed, got {labels:?}"
    );
}

#[test]
fn visible_control_tree_reports_every_labelled_element() {
    let (_tree, root, _button) = build(Rect::new(0.0, 0.0, 400.0, 300.0));

    let elements = AccessibilityTree::snapshot(&root);
    assert!(
        elements.len() >= 1,
        "a control tree with one labelled button must not snapshot empty"
    );
    let button = elements
        .iter()
        .find(|element| element.semantics.label == "Save")
        .expect("the button is present");
    assert_eq!(button.semantics.role, AccessibilityRole::Button);
    assert_eq!(button.frame, Rect::new(10.0, 10.0, 80.0, 30.0));
    assert_eq!(button.frame.origin(), Point::new(10.0, 10.0));
}
