//! Translation of `Tests/CoveUIRuntimeTests/ZIndexTests.swift`.
//!
//! Behaviour checklist:
//! - hit testing prefers the highest sibling `z_index`, whatever the child order

use cove_core::scene::{HitTester, Node};
use cove_core::{Point, Rect};

#[test]
fn hit_testing_prefers_highest_z_index() {
    let root = Node::new();
    root.set_frame(Rect::new(0.0, 0.0, 100.0, 100.0));

    let back = Node::new();
    back.set_frame(Rect::new(0.0, 0.0, 100.0, 100.0));
    back.set_is_hit_testable(true);

    let front = Node::new();
    front.set_frame(Rect::new(0.0, 0.0, 100.0, 100.0));
    front.set_is_hit_testable(true);
    front.set_z_index(10.0);

    // The high-z child is added first, so only z ordering can put it on top.
    root.add_child(&front);
    root.add_child(&back);

    let hit = HitTester::hit_test(&root, Point { x: 50.0, y: 50.0 }).expect("a node is hit");
    assert_eq!(hit.node.id(), front.id(), "the highest sibling z_index wins");
}
